//! ffmpeg : préparation des rushs, rendu des plans, assemblage (filtres, son, incrustations,
//! sous-titres). Les modèles (Whisper, netteté IA) passent par les capteurs Python.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use futures_util::stream::{self, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

use crate::carte;
use crate::decoupe::{self, Plan, Programme, IPS};
use crate::etat::EtatPartage;
use crate::modele::{maintenant, InfosSource, Projet, Sequence, Sortie};
use crate::reglages::{nombre, texte, vrai};

const PLANS_EN_PARALLELE: usize = 6;
/// Étiquetage couleur Rec.709 écrit dans le flux (les options -color_* seules ne suffisent pas avec x264).
const BT709: &str = "setparams=color_primaries=bt709:color_trc=bt709:colorspace=bt709";
/// Latence fixe du filtre de réduction du bruit afftdn, en échantillons à 48 kHz.
const LATENCE_AFFTDN: u32 = 1200;
const ZOOM_TRANSITION: f64 = 1.4;
/// Une passe de rendu. L'aperçu reste léger (30 i/s, 8 bits). Le rendu final garde la cadence et les 10 bits du
/// fichier de travail jusqu'au seul encodage avec perte : sur épisode 0, la chaîne 8 bits à 30 i/s et ses quatre
/// compressions donnaient une image moins propre que le rush iPhone (constat du 2026-10-26).
#[derive(Clone, Copy, Debug)]
pub struct Passe {
    pub ips: f64,
    pub dix_bits: bool,
    /// Sortie HDR HLG (BT.2100), comme le rush iPhone : sans conversion en SDR.
    pub hdr: bool,
}

/// Étiquetage couleur HLG BT.2020 de la sortie HDR.
const BT2020_HLG: &str = "setparams=color_primaries=bt2020:color_trc=arib-std-b67:colorspace=bt2020nc";
/// Table SDR → HLG des images incrustées (couleur::table_sdr_vers_hlg), écrite au premier rendu HDR.
static TABLE_HLG: std::sync::OnceLock<String> = std::sync::OnceLock::new();

impl Passe {
    pub const APERCU: Passe = Passe { ips: 30.0, dix_bits: false, hdr: false };

    fn setparams(&self) -> &'static str {
        if self.hdr { BT2020_HLG } else { BT709 }
    }

    /// Niveau du blanc pour lutyuv : le blanc de référence HLG (75 % du signal) en HDR, le blanc en SDR.
    fn blanc(&self) -> &'static str {
        if self.hdr { "minval+(maxval-minval)*0.75" } else { "maxval" }
    }

    /// Fin de chaîne d'un élément incrusté en RVBA (logo, image, gif, vidéo) avant l'overlay : passage en YUV avec
    /// la matrice de la sortie (sans elle, swscale prend BT.601 et décale les couleurs), et en HDR, couleurs SDR
    /// portées en HLG par la table BT.2408.
    fn vers_sortie_rgba(&self) -> String {
        let (fmt, matrice) = (if self.dix_bits { "yuva420p10le" } else { "yuva420p" }, if self.hdr { "bt2020" } else { "bt709" });
        let table = match (self.hdr, TABLE_HLG.get()) {
            (true, Some(t)) => format!("format=rgba,lut3d=file='{t}':interp=tetrahedral,"),
            _ => String::new(),
        };
        format!("{table}scale=out_color_matrix={matrice}:out_range=tv,format={fmt}")
    }

    /// Même chose pour une image opaque (vidéo ou image du générique) : rien en SDR, la table HLG en HDR.
    fn vers_sortie_opaque(&self) -> String {
        match (self.hdr, TABLE_HLG.get()) {
            (true, Some(t)) => format!("format=rgb48le,lut3d=file='{t}':interp=tetrahedral,scale=out_color_matrix=bt2020:out_range=tv,format={},",
                                       self.format()),
            _ => String::new(),
        }
    }

    /// Échantillons de son par image, à 48 kHz.
    fn echantillons_par_image(&self) -> u64 {
        (48_000.0 / self.ips).round() as u64
    }

    fn format(&self) -> &'static str {
        if self.dix_bits { "yuv420p10le" } else { "yuv420p" }
    }

    /// Option de format des incrustations (overlay), qui sinon repasse l'image en 8 bits.
    fn format_overlay(&self) -> &'static str {
        if self.dix_bits { ":format=yuv420p10" } else { "" }
    }
}

/// Fondu depuis ou vers le blanc entre [entree] et [sortie], sans quitter la profondeur de l'image (le fondu en
/// couleur de ffmpeg repasse en RVB 8 bits). `k` : part du blanc, expression du temps T de l'image.
fn fondu_blanc(entree: &str, sortie: &str, etiquette: &str, k: &str, passe: Passe) -> String {
    let blanc = passe.blanc();
    format!(
        "[{entree}]split[{etiquette}a][{etiquette}b];[{etiquette}b]lutyuv=y={blanc}:u=(maxval+minval)/2:v=(maxval+minval)/2[{etiquette}w];\
         [{etiquette}a][{etiquette}w]blend=all_expr='A+(B-A)*({k})'[{sortie}]"
    )
}

/// Cadence du fichier de travail d'un rush : 60 i/s pour un rush à 50 i/s ou plus, 30 sinon.
fn cadence_travail(ips_source: f64) -> u32 {
    if ips_source >= 45.0 { 60 } else { 30 }
}
/// Part de l'image reconstruite par l'IA, le reste venant de l'image d'origine agrandie : seule, l'IA
/// donne une peau « peinte » ; mélangée, elle garde le grain naturel (comparé le 2026-10-18).
const MELANGE_IA: f64 = 0.6;

/// Lance ffmpeg ; la progression (0–100) suit le temps encodé rapporté à `duree`.
async fn ffmpeg(etat: &EtatPartage, projet: &str, etape: &str, args: &[String], duree: f64) -> Result<()> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-hide_banner", "-v", "error", "-y", "-progress", "pipe:1", "-nostats"]).args(args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    let mut enfant = cmd.spawn().context("ffmpeg introuvable")?;
    let mut erreurs = enfant.stderr.take().unwrap();
    let lecture_erreurs = tokio::spawn(async move {
        let mut s = String::new();
        let _ = erreurs.read_to_string(&mut s).await;
        s
    });
    let mut lignes = BufReader::new(enfant.stdout.take().unwrap()).lines();
    let mut dernier = Instant::now();
    while let Some(l) = lignes.next_line().await? {
        if let Some(v) = l.strip_prefix("out_time_us=") {
            if dernier.elapsed().as_millis() > 500 && duree > 0.0 {
                let t = v.trim().parse::<f64>().unwrap_or(0.0) / 1e6;
                etat.signaler(projet, etape, (100.0 * t / duree).clamp(0.0, 99.0));
                dernier = Instant::now();
            }
        }
    }
    let statut = enfant.wait().await?;
    let erreurs = lecture_erreurs.await.unwrap_or_default();
    if !statut.success() {
        // Le détail complet va au journal ; la personne reçoit un message court (une ligne d'erreur ffmpeg peut
        // recopier tout le graphe de filtres).
        tracing::warn!("ffmpeg en échec ({etape}) : {}", erreurs.lines().rev().take(4).collect::<Vec<_>>().join(" | "));
        let court = |l: &str| if l.chars().count() > 140 { format!("{}…", l.chars().take(140).collect::<String>()) } else { l.to_string() };
        bail!("ffmpeg en échec ({etape}) : {}", erreurs.lines().rev().take(2).map(court).collect::<Vec<_>>().join(" | "));
    }
    Ok(())
}

async fn ffmpeg_simple(args: Vec<String>) -> Result<()> {
    let sortie = Command::new("ffmpeg")
        .args(["-hide_banner", "-v", "error", "-y"])
        .args(&args)
        .stdin(Stdio::null())
        .output()
        .await
        .context("ffmpeg introuvable")?;
    if !sortie.status.success() {
        bail!("ffmpeg en échec : {}", String::from_utf8_lossy(&sortie.stderr).lines().rev().take(4).collect::<Vec<_>>().join(" | "));
    }
    Ok(())
}

/// NVENC répond-il ? (il faut le paquet libnvidia-encode du pilote).
pub async fn nvenc_disponible() -> bool {
    Command::new("ffmpeg")
        .args(["-hide_banner", "-v", "error", "-f", "lavfi", "-i", "nullsrc=s=256x256:d=0.2", "-c:v", "h264_nvenc", "-f", "null", "-"])
        .output()
        .await
        .map(|s| s.status.success())
        .unwrap_or(false)
}

/// Options d'encodage vidéo : NVENC si on l'a, sinon x264.
fn encodeur(nvenc: bool, usage: &str) -> Vec<String> {
    let v: &[&str] = match (nvenc, usage) {
        (true, "plan_apercu") => &["-c:v", "h264_nvenc", "-preset", "p2", "-rc", "vbr", "-cq", "26", "-b:v", "0", "-pix_fmt", "yuv420p"],
        // Plans du rendu final : intermédiaires quasi sans perte, en 10 bits, sur le processeur (encodés en parallèle).
        (_, "plan") => &["-c:v", "libx264", "-profile:v", "high10", "-preset", "veryfast", "-crf", "6", "-pix_fmt", "yuv420p10le"],
        (true, "assemblage_apercu") => &["-c:v", "h264_nvenc", "-preset", "p4", "-rc", "vbr", "-cq", "24", "-b:v", "0", "-pix_fmt", "yuv420p"],
        // Le seul encodage avec perte du rendu final.
        (true, "final_hevc") => &["-c:v", "hevc_nvenc", "-profile:v", "main10", "-pix_fmt", "p010le", "-preset", "p7", "-tune", "hq",
                                  "-rc", "vbr", "-cq", "16", "-b:v", "0", "-spatial-aq", "1", "-temporal-aq", "1", "-tag:v", "hvc1"],
        (false, "final_hevc") => &["-c:v", "libx265", "-profile:v", "main10", "-pix_fmt", "yuv420p10le", "-preset", "medium", "-crf", "16",
                                   "-tag:v", "hvc1", "-x265-params", "log-level=error"],
        (true, _) => &["-c:v", "h264_nvenc", "-preset", "p7", "-tune", "hq", "-rc", "vbr", "-cq", "16", "-b:v", "0",
                       "-spatial-aq", "1", "-temporal-aq", "1", "-profile:v", "high", "-pix_fmt", "yuv420p"],
        (false, "plan_apercu") => &["-c:v", "libx264", "-preset", "ultrafast", "-crf", "23", "-pix_fmt", "yuv420p"],
        (false, "assemblage_apercu") => &["-c:v", "libx264", "-preset", "veryfast", "-crf", "23", "-pix_fmt", "yuv420p"],
        (false, _) => &["-c:v", "libx264", "-preset", "slow", "-crf", "16", "-pix_fmt", "yuv420p"],
    };
    v.iter().map(|x| x.to_string()).collect()
}

pub async fn ffprobe(fichier: &Path) -> Result<Value> {
    let sortie = Command::new("ffprobe")
        .args(["-v", "error", "-show_streams", "-show_format", "-of", "json"])
        .arg(fichier)
        .output()
        .await?;
    if !sortie.status.success() {
        bail!("fichier illisible : {}", String::from_utf8_lossy(&sortie.stderr).trim());
    }
    Ok(serde_json::from_slice(&sortie.stdout)?)
}

fn ratio(s: &str) -> f64 {
    match s.split_once('/') {
        Some((a, b)) => a.parse::<f64>().unwrap_or(0.0) / b.parse::<f64>().unwrap_or(1.0).max(1e-9),
        None => s.parse().unwrap_or(0.0),
    }
}

/// Lit un rush : dimensions affichées (rotation comprise), durée, cadence, HDR.
pub async fn sonder_rush(fichier: &Path) -> Result<InfosSource> {
    let d = ffprobe(fichier).await?;
    let v = d["streams"]
        .as_array()
        .and_then(|s| s.iter().find(|s| s["codec_type"] == "video"))
        .ok_or_else(|| anyhow!("ce fichier ne contient pas de vidéo"))?;
    let (mut l, mut h) = (v["width"].as_u64().unwrap_or(0) as u32, v["height"].as_u64().unwrap_or(0) as u32);
    let rotation = v["side_data_list"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|s| s["rotation"].as_f64())
        .or_else(|| v["tags"]["rotate"].as_str().and_then(|r| r.parse().ok()))
        .unwrap_or(0.0);
    if (rotation.abs() - 90.0).abs() < 1.0 || (rotation.abs() - 270.0).abs() < 1.0 {
        std::mem::swap(&mut l, &mut h);
    }
    let hdr = match v["color_transfer"].as_str() {
        Some("arib-std-b67") => Some("HLG".to_string()),
        Some("smpte2084") => Some("PQ".to_string()),
        _ => None,
    };
    Ok(InfosSource {
        largeur: l,
        hauteur: h,
        duree: d["format"]["duration"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
        ips: ratio(v["avg_frame_rate"].as_str().unwrap_or("30/1")),
        hdr,
        codec: v["codec_name"].as_str().unwrap_or("?").to_string(),
    })
}

/// Fichier de travail : rotation appliquée, HDR converti en SDR Rec.709 en virgule flottante, 10 bits, cadence du
/// rush (60 ou 30 i/s), son PCM. Conversion « mobius » : elle garde le contraste des tons moyens et le détail des
/// fenêtres, là où « hable » délavait toute l'image (comparé sur épisode 0, 2026-10-26).
pub async fn preparer(etat: &EtatPartage, projet: &str, source: &Path, infos: &InfosSource, mezz: &Path) -> Result<()> {
    preparer_en(etat, projet, source, infos, mezz, false).await
}

/// `garder_hlg` : fichier de travail HDR (mezz_hlg.mkv) d'un rush HLG, sans conversion en SDR, pour la sortie HDR.
async fn preparer_en(etat: &EtatPartage, projet: &str, source: &Path, infos: &InfosSource, mezz: &Path, garder_hlg: bool) -> Result<()> {
    let d = ffprobe(source).await?;
    let a_son = d["streams"].as_array().into_iter().flatten().any(|s| s["codec_type"] == "audio");
    let mut vf = Vec::new();
    let hlg = garder_hlg && infos.hdr.as_deref() == Some("HLG");
    match infos.hdr.as_deref() {
        _ if hlg => {}
        Some(hdr) => {
            let (tin, npl) = if hdr == "HLG" { ("arib-std-b67", 203) } else { ("smpte2084", 100) };
            vf.push(format!(
                "zscale=tin={tin}:min=bt2020nc:pin=bt2020:rin=tv:t=linear:npl={npl},format=gbrpf32le,zscale=p=bt709,\
                 tonemap=tonemap=mobius:desat=0,zscale=t=bt709:m=bt709:r=tv"
            ));
        }
        None => {}
    }
    let ips = cadence_travail(infos.ips);
    vf.push(format!("fps={ips}"));
    vf.push("format=yuv420p10le".into());
    vf.push(if hlg { BT2020_HLG } else { BT709 }.into());
    let (matrice, primaires, transfert) = if hlg { ("bt2020nc", "bt2020", "arib-std-b67") } else { ("bt709", "bt709", "bt709") };
    let mut args: Vec<String> = vec!["-i".into(), source.to_string_lossy().into()];
    if !a_son {
        args.extend(["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"].map(String::from));
    }
    args.extend(["-map".into(), "0:v:0".into(), "-map".into(), if a_son { "0:a:0".into() } else { "1:a".into() }]);
    args.extend([
        "-vf".into(), vf.join(","),
        "-c:v".into(), "libx264".into(), "-profile:v".into(), "high10".into(), "-preset".into(), "veryfast".into(),
        "-crf".into(), "10".into(), "-g".into(), format!("{ips}"), "-pix_fmt".into(), "yuv420p10le".into(),
        "-colorspace".into(), matrice.into(), "-color_primaries".into(), primaires.into(), "-color_trc".into(), transfert.into(),
        "-c:a".into(), "pcm_s16le".into(), "-ar".into(), "48000".into(), "-ac".into(), "2".into(),
    ]);
    if !a_son {
        args.push("-shortest".into());
    }
    args.push(mezz.to_string_lossy().into());
    ffmpeg(etat, projet, "Préparation du rush", &args, infos.duree).await
}

/// Version légère du rush pour le navigateur (vidéo d'origine dans l'interface).
pub async fn proxy(etat: &EtatPartage, projet: &str, mezz: &Path, sortie: &Path, duree: f64) -> Result<()> {
    let tmp = sortie.with_extension("tmp.mp4");
    let args: Vec<String> = vec![
        "-i".into(), mezz.to_string_lossy().into(),
        "-vf".into(), "fps=30,scale='if(gt(iw,ih),-2,540)':'if(gt(iw,ih),540,-2)'".into(),
        "-c:v".into(), "libx264".into(), "-preset".into(), "veryfast".into(), "-crf".into(), "26".into(),
        "-g".into(), "30".into(), "-pix_fmt".into(), "yuv420p".into(),
        "-c:a".into(), "aac".into(), "-b:a".into(), "96k".into(), "-movflags".into(), "+faststart".into(),
        tmp.to_string_lossy().into(),
    ];
    ffmpeg(etat, projet, "Version d'origine pour le lecteur", &args, duree).await?;
    tokio::fs::rename(&tmp, sortie).await?;
    Ok(())
}

/// Genre d'un média de la médiathèque : image, gif, video ou son.
pub async fn sonder_media(fichier: &Path, nom: &str) -> Result<(String, Option<f64>, bool)> {
    let d = ffprobe(fichier).await?;
    let flux = d["streams"].as_array().cloned().unwrap_or_default();
    let video = flux.iter().find(|s| s["codec_type"] == "video");
    let a_son = flux.iter().any(|s| s["codec_type"] == "audio");
    let duree = d["format"]["duration"].as_str().and_then(|s| s.parse::<f64>().ok());
    let nom = nom.to_lowercase();
    let genre = match video {
        None if a_son => "son",
        None => bail!("ni image, ni vidéo, ni son"),
        Some(v) => {
            let codec = v["codec_name"].as_str().unwrap_or("");
            if codec == "gif" || nom.ends_with(".gif") {
                "gif"
            } else if ["png", "mjpeg", "webp", "bmp", "tiff"].contains(&codec) && duree.map(|d| d < 0.2).unwrap_or(true) {
                "image"
            } else if v["disposition"]["attached_pic"] == 1 {
                // Pochette d'un fichier audio.
                "son"
            } else {
                "video"
            }
        }
    };
    Ok((genre.to_string(), duree, a_son))
}

/// Lance un capteur Python ; ses lignes {"etape","pct"} deviennent la progression.
pub async fn capteur(etat: &EtatPartage, projet: &str, script: &str, args: &[String]) -> Result<()> {
    let python = match script {
        "rehausse.py" => &etat.cfg.python_rehaussement,
        "restaure.py" => &etat.cfg.python_voicefixer,
        "separe.py" | "qualite_son.py" | "egaliseur.py" => &etat.cfg.python_son,
        "seche_ia.py" => &etat.cfg.python_clearvoice,
        "debruite.py" => &etat.cfg.python_deepfilter,
        _ => &etat.cfg.python,
    };
    let mut enfant = Command::new(python)
        .arg(etat.cfg.racine.join("capteurs").join(script))
        .args(args)
        .env("HF_HUB_OFFLINE", "1")
        // Moins de mémoire perdue en fragments quand la taille des lots varie (netteté IA).
        .env("PYTORCH_CUDA_ALLOC_CONF", "expandable_segments:True")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("Python des capteurs introuvable")?;
    let mut erreurs = enfant.stderr.take().unwrap();
    let lecture_erreurs = tokio::spawn(async move {
        let mut s = String::new();
        let _ = erreurs.read_to_string(&mut s).await;
        s
    });
    let mut lignes = BufReader::new(enfant.stdout.take().unwrap()).lines();
    let mut dernier = Instant::now();
    while let Some(l) = lignes.next_line().await? {
        let Ok(v) = serde_json::from_str::<Value>(&l) else { continue };
        let libelle = match v["etape"].as_str().unwrap_or("") {
            "son" => "Lecture du son",
            "transcription" => "Transcription de la parole",
            "visage" => "Repérage du visage",
            "nettete" => "Netteté IA",
            "rehaussement" => "Retrait de l'écho et du bruit (IA)",
            "restauration" => "Reconstruction du timbre (IA)",
            "verification" => "Vérification des mots de la voix reconstruite",
            "separation" => "Retrait de la réverbération (IA, séparation)",
            "sechage" => "Voix séchée : pièce et bruit retirés (IA)",
            "debruitage" => "Bruit de fond retiré (IA)",
            "egalisation" => "Égaliseur de timbre",
            "qualite" => "Mesure de la qualité du son",
            _ => continue,
        };
        if dernier.elapsed().as_millis() > 400 {
            etat.signaler(projet, libelle, v["pct"].as_f64().unwrap_or(0.0));
            dernier = Instant::now();
        }
    }
    let statut = enfant.wait().await?;
    let erreurs = lecture_erreurs.await.unwrap_or_default();
    if !statut.success() {
        let utile: Vec<&str> = erreurs.lines().filter(|l| !l.contains("WARN")).collect();
        // L'exception Python d'abord (sa dernière ligne « Type: message ») : les messages de ffmpeg qui suivent
        // (tube cassé quand le script s'arrête) n'en sont que la conséquence.
        let exception = utile.iter().rev().find(|l| !l.starts_with(' ') && !l.starts_with('[') && l.split(':').next().is_some_and(|t| t.ends_with("Error") || t.ends_with("Exception")));
        let fin = utile.iter().rev().take(3).rev().cloned().collect::<Vec<_>>().join(" | ");
        match exception {
            Some(e) => bail!("{script} en échec : {e}"),
            None => bail!("{script} en échec : {fin}"),
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- plans

struct Zone {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

fn zone_entiere(p: &Plan, source: (u32, u32)) -> Zone {
    let pair = |v: f64| ((v.floor() as u32) / 2 * 2).max(2);
    let w = pair(p.zone.w).min(source.0 / 2 * 2);
    let h = pair(p.zone.h).min(source.1 / 2 * 2);
    let x = (p.zone.x.floor() as u32).min(source.0 - w);
    let y = (p.zone.y.floor() as u32).min(source.1 - h);
    Zone { x, y, w, h }
}

/// Largeur utile de la version nette d'un plan : de quoi remplir la sortie au cadre le plus serré.
fn largeur_nette(p: &Plan, largeur_zone: u32, largeur_sortie: u32) -> u32 {
    let z_trans = if p.transition.as_deref() == Some("zoom") { ZOOM_TRANSITION } else { 1.0 };
    let utile = (largeur_sortie as f64 * p.z_fin * z_trans).ceil() as u32;
    (utile.min(largeur_zone * 4) / 2 * 2).max(2)
}

/// Filtre vidéo d'un plan, à partir de l'image de la zone (ou de sa version nette).
///
/// Le zoom agrandit l'image à chaque trame (lanczos) puis la pose, décalée, sur un cadre à la taille de la sortie :
/// zoompan, qu'on employait avant, ne travaille qu'en 8 bits.
fn filtre_plan(p: &Plan, sortie: (u32, u32), nette: bool, passe: Passe) -> String {
    let (wo, ho) = sortie;
    let ips = passe.ips;
    let n = (p.duree * ips).round().max(1.0);
    let z_trans = if p.transition.as_deref() == Some("zoom") { ZOOM_TRANSITION } else { 1.0 };
    let mut f: Vec<String> = vec![format!("fps={ips}")];
    if p.z_fin <= 1.0 + 1e-6 && z_trans == 1.0 {
        f.push(format!("scale={wo}:{ho}:flags=lanczos"));
    } else {
        let zm = p.z_fin * z_trans;
        let (cx, cy) = p.centre_serre;
        let nt = 0.25 * ips;
        // Une poussée répartie sur plusieurs plans avance régulièrement ; seule, elle ralentit en fin de plan.
        let avance = if p.lineaire { format!("min(n/{n},1)") } else { format!("(1-pow(1-min(n/{n},1),2))") };
        let z = format!("(1+{:.5}*{avance})*(1+{:.3}*pow(max(0,1-n/{nt}),2))", p.z_fin - 1.0, z_trans - 1.0);
        // Part du recentrage vers le visage : nulle à l'image entière, pleine au cadre le plus serré.
        let progres = |taille: &str, cadre: u32| {
            if zm > 1.0 + 1e-6 { format!("min(1,max(0,({taille}/{cadre}-1)/{:.5}))", zm - 1.0) } else { "0".into() }
        };
        let (pw, ph) = (progres("overlay_w", wo), progres("overlay_h", ho));
        f.push(format!(
            "scale=w='ceil({wo}*{z}/2)*2':h='ceil({ho}*{z}/2)*2':eval=frame:flags=lanczos[zz];\
             color=c=black:s={wo}x{ho}:r={ips}[zc];\
             [zc][zz]overlay=x='-clip((0.5+({cx:.5}-0.5)*{pw})*overlay_w-{wo}/2,0,overlay_w-{wo})':\
             y='-clip((0.5+({cy:.5}-0.5)*{ph})*overlay_h-{ho}/2,0,overlay_h-{ho})':eval=frame:shortest=1{}",
            passe.format_overlay()
        ));
    }
    // Zoom sans IA : un peu de netteté adaptative (CAS, luminance) compense la douceur de l'agrandissement, sans
    // halo autour des contours.
    if !nette && p.agrandissement > 1.2 {
        f.push(format!("cas=strength={:.2}:planes=1", (0.15 * (p.agrandissement - 1.0)).min(0.35)));
    }
    let echelle = wo as f64 / 1080.0;
    let flou = |taille: f64| ((taille * echelle).round() as u32).max(2);
    let mut chaine = f.join(",");
    let ajouter = |c: &mut String, filtre: String| {
        c.push(',');
        c.push_str(&filtre);
    };
    match p.transition.as_deref() {
        Some("flash") => {
            chaine.push_str("[fl0];");
            chaine.push_str(&fondu_blanc("fl0", "fl1", "flb", "clip(1-T/0.15,0,1)", passe));
            chaine.push_str(";[fl1]null");
        }
        Some("fondu") => ajouter(&mut chaine, "fade=t=in:st=0:d=0.25".into()),
        Some("glitch") => ajouter(&mut chaine, "rgbashift=rh=-14:bh=14:enable='lt(t,0.2)',noise=alls=35:allf=t:enable='lt(t,0.2)'".into()),
        Some("glisse") => ajouter(&mut chaine, format!(
            "avgblur=sizeX={}:sizeY=1:enable='lt(t,0.05)',avgblur=sizeX={}:sizeY=1:enable='between(t,0.05,0.1)',\
             avgblur=sizeX={}:sizeY=1:enable='between(t,0.1,0.15)'",
            flou(60.0), flou(30.0), flou(12.0)
        )),
        _ => {}
    }
    match p.sortie.as_deref() {
        Some("fondu") => ajouter(&mut chaine, format!("fade=t=out:st={:.3}:d=0.12", (p.duree - 0.12).max(0.0))),
        Some("glisse") => ajouter(&mut chaine, format!("avgblur=sizeX={}:sizeY=1:enable='gte(t,{:.3})'", flou(40.0), (p.duree - 0.08).max(0.0))),
        _ => {}
    }
    ajouter(&mut chaine, "setsar=1".into());
    ajouter(&mut chaine, format!("format={}", passe.format()));
    chaine
}

#[allow(clippy::too_many_arguments)]
async fn rendre_plan(p: &Plan, mezz: &Path, nette: Option<&Path>, voix: Option<&Path>, source: (u32, u32),
                     sortie: (u32, u32), fichier: &Path, passe: Passe, nvenc: bool) -> Result<()> {
    let zone = zone_entiere(p, source);
    let n = (p.duree * passe.ips).round().max(1.0) as u32;
    let mezz_hdr = mezz.with_file_name("mezz_hlg.mkv");
    let mezz = if passe.hdr { mezz_hdr.as_path() } else { mezz };
    // Image et son de durée exacte : n images et n × 1 600 échantillons (à 30 i/s). Avec « -frames:v », ffmpeg coupait
    // le son au dernier bloc entier : jusqu'à 20 ms perdus par plan, près d'une seconde d'avance de la voix
    // sur 200 plans (mesuré le 2026-10-18).
    let echantillons = n as u64 * passe.echantillons_par_image();
    let mut args: Vec<String> = Vec::new();
    let vf = match nette {
        Some(f) => {
            args.extend(["-t".into(), format!("{:.3}", p.duree), "-i".into(), f.to_string_lossy().into()]);
            filtre_plan(p, sortie, true, passe)
        }
        None => format!("crop={}:{}:{}:{},{}", zone.w, zone.h, zone.x, zone.y, filtre_plan(p, sortie, false, passe)),
    };
    args.extend([
        "-ss".into(), format!("{:.3}", p.t0), "-t".into(), format!("{:.3}", p.duree), "-i".into(), mezz.to_string_lossy().into(),
    ]);
    let (v, mut a) = if nette.is_some() { ("0:v", "1:a") } else { ("0:v", "0:a") };
    // Voix sans écho : même découpe, dans le son du rush traité (calé à l'échantillon sur le rush).
    if let Some(f) = voix {
        args.extend(["-ss".into(), format!("{:.3}", p.t0), "-t".into(), format!("{:.3}", p.duree), "-i".into(), f.to_string_lossy().into()]);
        a = if nette.is_some() { "2:a" } else { "1:a" };
    }
    args.extend([
        "-map".into(), v.into(), "-map".into(), a.into(),
        "-vf".into(), format!("{vf},tpad=stop_mode=clone:stop_duration=1,trim=end_frame={n},setpts=PTS-STARTPTS"),
        "-af".into(), format!("aresample=48000,apad=whole_len={echantillons},atrim=end_sample={echantillons},\
                               afade=t=in:d=0.01,afade=t=out:st={:.3}:d=0.01", (p.duree - 0.01).max(0.0)),
    ]);
    args.extend(encodeur(nvenc, if passe.dix_bits { "plan" } else { "plan_apercu" }));
    args.extend([
        "-c:a".into(), "pcm_s16le".into(), "-ac".into(), "2".into(),
        fichier.to_string_lossy().into(),
    ]);
    ffmpeg_simple(args).await
}

// ---------------------------------------------------------------- sous-titres

fn temps_ass(t: f64) -> String {
    let t = t.max(0.0);
    format!("{}:{:02}:{:05.2}", (t / 3600.0) as u32, ((t % 3600.0) / 60.0) as u32, t % 60.0)
}

fn sans_ponctuation(m: &str) -> String {
    m.trim_end_matches(['.', ',', '!', '?', ';', ':']).to_string()
}

/// Un sous-titre à l'écran : de `debut` à `fin` (temps du montage), ses mots (sans ponctuation) et leur emphase.
pub struct GroupeSt {
    pub debut: f64,
    pub fin: f64,
    pub mots: Vec<(String, bool)>,
}

/// Les sous-titres du montage, groupe par groupe : le rendu les écrit en ASS, l'aperçu en direct les reçoit tels
/// quels (programme_json) et ne fait que les dessiner. Vide si les sous-titres sont éteints.
pub fn groupes_sous_titres(prog: &Programme, r: &crate::reglages::Reglages) -> Vec<GroupeSt> {
    if !vrai(r, "sous_titres") {
        return vec![];
    }
    let dynamique = texte(r, "style_sous_titres") == "dynamique";
    let vertical = prog.hauteur > prog.largeur;
    // Recolle « c » + « 'est ».
    let mut mots: Vec<decoupe::MotSortie> = Vec::new();
    for m in &prog.mots {
        match mots.last_mut() {
            Some(prec) if m.mot.starts_with(['\'', '-']) && m.t0 - prec.t1 < 0.3 => {
                prec.mot.push_str(&m.mot);
                prec.t1 = m.t1;
                prec.fort |= m.fort;
            }
            _ => mots.push(m.clone()),
        }
    }
    let (max_mots, max_car) = if dynamique { (3, 16) } else if vertical { (6, 30) } else { (8, 42) };
    let mut groupes: Vec<Vec<decoupe::MotSortie>> = Vec::new();
    let mut courant: Vec<decoupe::MotSortie> = Vec::new();
    for m in mots {
        if let Some(dernier) = courant.last() {
            let longueur: usize = courant.iter().map(|x| x.mot.chars().count() + 1).sum::<usize>() + m.mot.chars().count();
            if courant.len() >= max_mots || longueur > max_car || m.t0 - dernier.t1 > 0.3
                || dernier.mot.ends_with(['.', '?', '!', ','])
            {
                groupes.push(std::mem::take(&mut courant));
            }
        }
        courant.push(m);
    }
    if !courant.is_empty() {
        groupes.push(courant);
    }
    (0..groupes.len())
        .map(|i| {
            let g = &groupes[i];
            let fin_naturelle = g[g.len() - 1].t1 + 0.4;
            GroupeSt {
                debut: g[0].t0,
                fin: groupes.get(i + 1).map(|s| s[0].t0.min(fin_naturelle)).unwrap_or(fin_naturelle),
                mots: g.iter().map(|m| (sans_ponctuation(&m.mot), m.fort)).collect(),
            }
        })
        .collect()
}

/// Les sous-titres ASS du rendu final, pour les essais du direct.
#[cfg(test)]
pub fn sous_titres_pour_essai(prog: &Programme, p: &Projet) -> String {
    sous_titres(prog, p, false)
}

fn sous_titres(prog: &Programme, p: &Projet, apercu: bool) -> String {
    let r = &p.reglages;
    let (wo, ho) = (prog.largeur as f64, prog.hauteur as f64);
    let vertical = ho > wo;
    let dynamique = texte(r, "style_sous_titres") == "dynamique";
    let milieu = texte(r, "position_sous_titres") == "milieu";
    let taille = if dynamique { if vertical { 0.054 * ho } else { 0.075 * ho } } else if vertical { 0.034 * ho } else { 0.05 * ho };
    let marge = if milieu { 0.0 } else if vertical { 0.21 * ho } else { 0.09 * ho };
    let alignement = if milieu { 5 } else { 2 };
    let (police, contour, ombre) = if dynamique { ("Anton", 0.0036 * ho.max(wo), 3.0) } else { ("DejaVu Sans", 0.002 * ho.max(wo), 1.0) };
    let titre_taille = if vertical { 0.048 * ho } else { 0.07 * ho };
    let mut l = vec![
        "[Script Info]".to_string(),
        "ScriptType: v4.00+".into(),
        format!("PlayResX: {}", prog.largeur),
        format!("PlayResY: {}", prog.hauteur),
        "WrapStyle: 0".into(),
        String::new(),
        "[V4+ Styles]".into(),
        "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, \
         StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding"
            .into(),
        format!("Style: Mot,{police},{taille:.0},&H00FFFFFF,&H00FFFFFF,&H00000000,&H64000000,{},0,0,0,100,100,1,0,1,{contour:.1},{ombre},{alignement},{m:.0},{m:.0},{marge:.0},1",
                if dynamique { 0 } else { -1 }, m = 0.05 * wo),
        format!("Style: Titre,Anton,{titre_taille:.0},&H00000000,&H00000000,&H0000E5FF,&H0000E5FF,0,0,0,0,100,100,2,0,3,{:.0},0,8,60,60,{:.0},1",
                0.011 * ho.max(wo), 0.12 * ho),
        format!("Style: Texte,Anton,{:.0},&H00FFFFFF,&H00FFFFFF,&H00000000,&H78000000,0,0,0,0,100,100,1,0,1,{:.1},2,5,40,40,40,1",
                0.06 * ho, 0.0035 * ho.max(wo)),
        format!("Style: TexteBoite,Anton,{:.0},&H00FFFFFF,&H00FFFFFF,&H50000000,&H50000000,0,0,0,0,100,100,1,0,3,{:.1},0,5,40,40,40,1",
                0.06 * ho, 0.012 * ho.max(wo)),
        String::new(),
        "[Events]".into(),
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text".into(),
    ];
    // L'aperçu sort sans textes : sous-titres, titres de parties, textes posés et habillage sont dessinés en direct.
    if apercu {
        return l.join("\n") + "\n";
    }
    for (t, titre) in &prog.titres {
        l.push(format!("Dialogue: 1,{},{},Titre,,0,0,0,,{{\\fad(120,200)}}{}", temps_ass(t + 0.1), temps_ass(t + 1.9),
                       titre.to_uppercase()));
    }
    textes(prog, &mut l);
    habillage_textes(p, prog, &mut l);
    if !vrai(r, "sous_titres") {
        return l.join("\n") + "\n";
    }
    for g in groupes_sous_titres(prog, r) {
        let texte_groupe = g
            .mots
            .iter()
            .map(|(mot, fort)| {
                if dynamique {
                    let mot = mot.to_uppercase();
                    if *fort { format!("{{\\c&H0000E5FF&\\fscx115\\fscy115}}{mot}{{\\r}}") } else { mot }
                } else {
                    mot.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let anim = if dynamique { "{\\fscx85\\fscy85\\t(0,90,\\fscx100\\fscy100)}" } else { "" };
        l.push(format!("Dialogue: 0,{},{},Mot,,0,0,0,,{anim}{texte_groupe}", temps_ass(g.debut), temps_ass(g.fin)));
    }
    l.join("\n") + "\n"
}

/// Position par défaut d'une incrustation (centre 0–1), selon sa position nommée.
fn centre_nomme(position: &str, texte: bool) -> (f64, f64) {
    match position {
        "haut_gauche" => (0.27, 0.16),
        "haut_droite" => (0.73, 0.16),
        "bas_gauche" => (0.27, 0.72),
        "bas_droite" => (0.73, 0.72),
        "centre" | "plein" => (0.5, 0.5),
        "haut" => (0.5, 0.18),
        "bas" => (0.5, 0.7),
        _ if texte => (0.5, 0.22),
        _ => (0.5, 0.5),
    }
}

fn echapper_ass(t: &str) -> String {
    t.replace('\\', "/").replace('{', "(").replace('}', ")").replace('\n', "\\N")
}

/// Textes d'habillage : posés sur toute la durée, dans un coin ou sur un bord, avec leur transparence.
fn habillage_textes(p: &Projet, prog: &Programme, l: &mut Vec<String>) {
    let (wo, ho) = (prog.largeur as f64, prog.hauteur as f64);
    let marge = (0.04 * wo).round();
    for hab in p.habillage.iter().filter(|x| x.genre == "texte" && !x.texte.trim().is_empty()) {
        let (an, x, y) = match hab.position.as_str() {
            "haut_gauche" => (7, marge, marge),
            "bas_gauche" => (1, marge, ho - marge),
            "bas_droite" => (3, wo - marge, ho - marge),
            "haut" => (8, wo / 2.0, marge),
            "bas" => (2, wo / 2.0, ho - marge),
            _ => (9, wo - marge, marge),
        };
        let couleur = match hab.couleur.as_str() {
            "noir" => "&H00000000&",
            "jaune" => "&H0000E5FF&",
            _ => "&H00FFFFFF&",
        };
        let contour = if hab.couleur == "noir" { "&H00FFFFFF&" } else { "&H00000000&" };
        let alpha = ((1.0 - hab.opacite.clamp(0.1, 1.0)) * 255.0).round() as u8;
        let taille = (hab.taille * wo).max(12.0);
        l.push(format!(
            "Dialogue: 3,{},{},Texte,,0,0,0,,{{\\an{an}\\pos({x:.0},{y:.0})\\fs{taille:.0}\\c{couleur}\\3c{contour}\\bord{:.1}\\shad1\\alpha&H{alpha:02X}&}}{}",
            temps_ass(0.0), temps_ass(prog.duree), (taille * 0.06).max(1.0), echapper_ass(hab.texte.trim())
        ));
    }
}

/// Textes du module Texte : position, couleur, fond, effet d'entrée.
fn textes(prog: &Programme, l: &mut Vec<String>) {
    let (wo, ho) = (prog.largeur as f64, prog.hauteur as f64);
    for inc in prog.incrustations.iter().filter(|i| !i.texte.trim().is_empty()) {
        let (cx, cy) = match (inc.x, inc.y) {
            (Some(x), Some(y)) => (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)),
            _ => centre_nomme(&inc.position, true),
        };
        let (x, y) = (cx * wo, cy * ho);
        let taille = ho * (0.02 + 0.10 * inc.taille.clamp(0.1, 1.0));
        let boite = inc.fond == "boite" || inc.fond == "bandeau";
        let (couleur, cadre) = match (inc.couleur.as_str(), boite) {
            ("jaune", true) => ("&H0000E5FF&", "&H50000000&"),
            ("jaune", false) => ("&H0000E5FF&", "&H00000000&"),
            ("noir", _) => ("&H00000000&", "&H00FFFFFF&"),
            (_, true) => ("&H00FFFFFF&", "&H50000000&"),
            _ => ("&H00FFFFFF&", "&H00000000&"),
        };
        let marge = if inc.fond == "bandeau" { format!("\\bord{:.0}", 0.03 * ho) } else { String::new() };
        let duree_ms = ((inc.fin - inc.debut) * 1000.0).max(200.0);
        let mut tags = format!("\\an5\\fs{taille:.0}\\c{couleur}\\3c{cadre}\\4c{cadre}{marge}");
        match inc.entree.as_str() {
            "splash" => tags.push_str("\\pos(X,Y)\\fscx30\\fscy30\\t(0,160,\\fscx115\\fscy115)\\t(160,280,\\fscx100\\fscy100)\\fad(0,200)"),
            "glisse_gauche" => tags.push_str(&format!("\\move({:.0},{y:.0},{x:.0},{y:.0},0,300)\\fad(0,200)", -0.4 * wo)),
            "glisse_droite" => tags.push_str(&format!("\\move({:.0},{y:.0},{x:.0},{y:.0},0,300)\\fad(0,200)", 1.4 * wo)),
            "glisse_haut" => tags.push_str(&format!("\\move({x:.0},{:.0},{x:.0},{y:.0},0,300)\\fad(0,200)", -0.15 * ho)),
            "glisse_bas" => tags.push_str(&format!("\\move({x:.0},{:.0},{x:.0},{y:.0},0,300)\\fad(0,200)", 1.15 * ho)),
            "zoom" => tags.push_str("\\pos(X,Y)\\fscx10\\fscy10\\t(0,250,\\fscx100\\fscy100)\\fad(0,200)"),
            "aucune" | "machine" => tags.push_str("\\pos(X,Y)"),
            _ => tags.push_str("\\pos(X,Y)\\fad(250,250)"),
        }
        let tags = tags.replace("\\pos(X,Y)", &format!("\\pos({x:.0},{y:.0})"));
        let corps = if inc.entree == "machine" {
            // Machine à écrire : chaque lettre apparaît à son tour.
            let lettres: Vec<char> = inc.texte.trim().chars().collect();
            let pas = (0.6 * duree_ms / lettres.len().max(1) as f64).min(70.0);
            lettres
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let a = (i as f64 * pas) as u64;
                    let lettre = echapper_ass(&c.to_string());
                    format!("{{\\alpha&HFF&\\t({a},{},\\alpha&H00&)}}{lettre}", a + 1)
                })
                .collect::<String>()
        } else {
            echapper_ass(inc.texte.trim())
        };
        let style = if boite { "TexteBoite" } else { "Texte" };
        l.push(format!("Dialogue: 2,{},{},{style},,0,0,0,,{{{tags}}}{corps}", temps_ass(inc.debut), temps_ass(inc.fin)));
    }
}

// ---------------------------------------------------------------- habillage et générique

/// Coin ou bord d'un élément d'habillage, en expressions d'overlay (W, H : image ; w, h : élément).
/// Logo de l'habillage posé sur [base] → [sortie] : son halo lumineux (copie blanche floutée), son ombre portée
/// (copie noire floutée, décalée vers le bas à droite), puis le logo. Les trois sont agrandis d'une bordure
/// transparente où le flou s'étale ; la position est corrigée de cette bordure.
#[allow(clippy::too_many_arguments)]
fn graphe_logo(entree: &str, base: &str, sortie: &str, j: usize, largeur: u32, hab: &crate::modele::Habillage, marge: u32,
               passe: Passe) -> Vec<String> {
    let (ips, fmt) = (passe.ips, passe.format_overlay());
    let conv = passe.vers_sortie_rgba();
    let (x, y) = position_habillage(&hab.position, marge);
    let l = largeur as f64;
    let flou_ombre = (0.025 * l).max(1.0);
    let decalage = (0.03 * l).round().max(1.0);
    // Le réglage du halo est son opacité (0 à 1) ; son étendue varie peu (0,021 à 0,039 × largeur du logo). Avant,
    // il ne réglait que l'étendue : même à 1 %, la lueur était presque opaque et noyait un logo fin sur fond clair.
    let flou_halo = (0.03 * l * (0.7 + 0.6 * hab.halo)).max(1.0);
    let teinte = if hab.halo_couleur == "noir" { 0 } else { 255 };
    let bord = (3.0 * flou_ombre.max(flou_halo) + decalage).ceil() as u32 / 2 * 2 + 2;
    // Une expression écrite pour le logo seul (w, h) vaut pour le logo agrandi de sa bordure (w-2b, h-2b), moins b.
    let corriger = |e: &str| format!("({})-{bord}", e.replace('w', &format!("(w-{})", 2 * bord)).replace('h', &format!("(h-{})", 2 * bord)));
    let (x, y) = (corriger(&x), corriger(&y));
    let mut fc = vec![format!(
        "[{entree}]fps={ips},scale={largeur}:-2,format=rgba,colorchannelmixer=aa={:.2},pad=iw+{}:ih+{}:{bord}:{bord}:color=black@0[hl{j}]",
        hab.opacite, 2 * bord, 2 * bord
    )];
    let mut sur = base.to_string();
    let couches: Vec<(&str, bool)> = vec![("halo", hab.halo > 0.0), ("ombre", hab.ombre > 0.0)];
    let nb = 1 + couches.iter().filter(|(_, oui)| *oui).count();
    if nb > 1 {
        fc.push(format!("[hl{j}]split={nb}[hlo{j}]{}", couches.iter().filter(|(_, o)| *o).map(|(n, _)| format!("[h{n}{j}]")).collect::<String>()));
    } else {
        fc.push(format!("[hl{j}]null[hlo{j}]"));
    }
    if hab.halo > 0.0 {
        // Le flou étale et dilue l'opacité : on la relève pour que le halo se voie.
        fc.push(format!(
            "[hhalo{j}]lutrgb=r={teinte}:g={teinte}:b={teinte},gblur=sigma={flou_halo:.1},lutrgb=a='min(255,val*{:.3})',{conv}[hhb{j}]",
            3.0 * hab.halo
        ));
        fc.push(format!("[{sur}][hhb{j}]overlay=x='{x}':y='{y}':eof_action=pass{fmt}[hvh{j}]"));
        sur = format!("hvh{j}");
    }
    if hab.ombre > 0.0 {
        fc.push(format!(
            "[hombre{j}]lutrgb=r=0:g=0:b=0,gblur=sigma={flou_ombre:.1},lutrgb=a='val*{:.2}',{conv}[hob{j}]",
            0.9 * hab.ombre
        ));
        fc.push(format!("[{sur}][hob{j}]overlay=x='{x}+{decalage}':y='{y}+{decalage}':eof_action=pass{fmt}[hvo{j}]"));
        sur = format!("hvo{j}");
    }
    fc.push(format!("[hlo{j}]{conv}[hlc{j}]"));
    fc.push(format!("[{sur}][hlc{j}]overlay=x='{x}':y='{y}':eof_action=pass{fmt}[{sortie}]"));
    fc
}

fn position_habillage(position: &str, marge: u32) -> (String, String) {
    match position {
        "haut_gauche" => (format!("{marge}"), format!("{marge}")),
        "bas_gauche" => (format!("{marge}"), format!("H-h-{marge}")),
        "bas_droite" => (format!("W-w-{marge}"), format!("H-h-{marge}")),
        "haut" => ("(W-w)/2".into(), format!("{marge}")),
        "bas" => ("(W-w)/2".into(), format!("H-h-{marge}")),
        _ => (format!("W-w-{marge}"), format!("{marge}")),
    }
}

/// Durées des séquences du générique (début, fin), 0 s'il n'y en a pas.
pub fn durees_generique(p: &Projet) -> (f64, f64) {
    let duree = |s: &Option<Sequence>| -> f64 {
        let Some(s) = s else { return 0.0 };
        if s.genre == "carton" {
            return s.duree;
        }
        let d = p.medias.iter().find(|m| m.id == s.media).and_then(|m| m.duree).unwrap_or(0.0);
        if s.duree > 0.0 { d.min(s.duree) } else { d }
    };
    let arrondi = |d: f64| (d * IPS as f64).floor() / IPS as f64;
    (arrondi(duree(&p.generique.debut)), arrondi(duree(&p.generique.fin)))
}

/// Coupe un titre en lignes d'au plus `max` caractères, aux espaces.
fn couper_lignes(texte: &str, max: usize) -> Vec<String> {
    let mut lignes: Vec<String> = vec![String::new()];
    for mot in texte.split_whitespace() {
        let l = lignes.last_mut().unwrap();
        if !l.is_empty() && l.chars().count() + 1 + mot.chars().count() > max {
            lignes.push(mot.to_string());
        } else {
            if !l.is_empty() {
                l.push(' ');
            }
            l.push_str(mot);
        }
    }
    lignes
}

/// Une séquence du générique, au format du montage, dans le graphe : [gv{k}] (image) et [ga{k}] (son).
/// Vidéo : cadrée sur fond flouté si son format diffère. Carton : titre et sous-titre centrés sur l'image
/// choisie, ou sur un fond sombre ; silencieux.
#[allow(clippy::too_many_arguments)]
fn sequence_generique(etat: &EtatPartage, p: &Projet, s: &Sequence, d: f64, k: usize, (wo, ho): (u32, u32),
                      args: &mut Vec<String>, fc: &mut Vec<String>, entree: &mut usize, tmp: &Path, passe: Passe) -> Result<()> {
    let ips = passe.ips;
    let medias = etat.dossier_projet(&p.id).join("medias");
    let media = p.medias.iter().find(|m| m.id == s.media);
    let fondu = format!("fade=t=in:st=0:d=0.35,fade=t=out:st={:.3}:d=0.35", (d - 0.35).max(0.0));
    let afondu = format!("afade=t=in:st=0:d=0.3,afade=t=out:st={:.3}:d=0.35", (d - 0.35).max(0.0));
    let finir = format!("trim=0:{d:.3},setpts=PTS-STARTPTS,fps={ips},setsar=1,format={},{},{fondu}", passe.format(), passe.setparams());
    let opaque = passe.vers_sortie_opaque();
    let silence = |args: &mut Vec<String>, fc: &mut Vec<String>, entree: &mut usize| {
        args.extend(["-f".into(), "lavfi".into(), "-t".into(), format!("{d:.3}"), "-i".into(), "anullsrc=r=48000:cl=stereo".into()]);
        fc.push(format!("[{}:a]aresample=48000,asetpts=PTS-STARTPTS[ga{k}]", *entree));
        *entree += 1;
    };
    if s.genre == "video" {
        let m = media.ok_or_else(|| anyhow!("la vidéo du générique n'est plus dans la médiathèque"))?;
        args.extend(["-t".into(), format!("{d:.3}"), "-i".into(), medias.join(&m.fichier).to_string_lossy().into()]);
        let i = *entree;
        *entree += 1;
        fc.push(format!(
            "[{i}:v]fps={ips},split[gs{k}a][gs{k}b];\
             [gs{k}b]scale={wo}:{ho}:force_original_aspect_ratio=increase,crop={wo}:{ho},gblur=sigma=30[gs{k}c];\
             [gs{k}a]scale={wo}:{ho}:force_original_aspect_ratio=decrease[gs{k}d];\
             [gs{k}c][gs{k}d]overlay=(W-w)/2:(H-h)/2,{opaque}{finir}[gv{k}]"
        ));
        if m.a_son {
            fc.push(format!(
                "[{i}:a]aresample=48000,aformat=channel_layouts=stereo,apad,atrim=0:{d:.3},asetpts=PTS-STARTPTS,\
                 loudnorm=I=-14:TP=-1.5:LRA=11,aresample=48000,{afondu}[ga{k}]"
            ));
        } else {
            silence(args, fc, entree);
        }
        return Ok(());
    }
    // Carton.
    let i = *entree;
    match media {
        Some(m) => {
            if m.genre == "gif" {
                args.extend(["-ignore_loop", "0"].map(String::from));
            } else {
                args.extend(["-loop", "1", "-framerate", "30"].map(String::from));
            }
            args.extend(["-t".into(), format!("{d:.3}"), "-i".into(), medias.join(&m.fichier).to_string_lossy().into()]);
            let assombrir = if s.titre.is_empty() && s.sous_titre.is_empty() { "" } else { ",eq=brightness=-0.08" };
            fc.push(format!("[{i}:v]scale={wo}:{ho}:force_original_aspect_ratio=increase,crop={wo}:{ho}{assombrir},{}null[gc{k}]", opaque));
        }
        None => {
            args.extend(["-f".into(), "lavfi".into(), "-t".into(), format!("{d:.3}"), "-i".into(), format!("color=c={}:s={wo}x{ho}:r={ips}", crate::couleur::hex_ffmpeg([0x10, 0x10, 0x14], passe.hdr))]);
            fc.push(format!("[{i}:v]null[gc{k}]"));
        }
    }
    *entree += 1;
    let vertical = wo < ho;
    let police = etat.cfg.racine.join("modeles").join("polices").join("Anton-Regular.ttf");
    let police_sous = ["/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"].iter().map(Path::new).find(|f| f.exists())
        .map(Path::to_path_buf).unwrap_or_else(|| police.clone());
    // Une ligne par dessin : un saut de ligne dans le texte s'affiche comme un carré avec cette police.
    let mut textes = Vec::new();
    let deux = !s.titre.is_empty() && !s.sous_titre.is_empty();
    let mut poser = |nom: &str, lignes: Vec<String>, taille: f64, police: &Path, couleur: &str, ombre: u32, y_bloc: &dyn Fn(f64) -> String|
                     -> Result<()> {
        let hauteur_ligne = (taille * 1.22).round();
        let bloc = hauteur_ligne * lignes.len() as f64;
        for (i, ligne) in lignes.iter().enumerate() {
            let f = tmp.join(format!("generique{k}_{nom}{i}.txt"));
            std::fs::write(&f, ligne)?;
            textes.push(format!(
                "drawtext=fontfile='{}':textfile='{}':fontsize={taille}:fontcolor={couleur}:x=(w-text_w)/2:y={}+{}:\
                 shadowcolor=black@0.45:shadowx={ombre}:shadowy={}",
                police.to_string_lossy(), f.to_string_lossy(), y_bloc(bloc), hauteur_ligne * i as f64, ombre + 1
            ));
        }
        Ok(())
    };
    if !s.titre.is_empty() {
        let taille = (wo as f64 * if vertical { 0.085 } else { 0.055 }).round();
        // Seul : centré ; avec un sous-titre : posé juste au-dessus du milieu.
        let y = move |bloc: f64| if deux { format!("h/2-h*0.012-{bloc}") } else { format!("(h-{bloc})/2") };
        poser("titre", couper_lignes(&s.titre, if vertical { 16 } else { 28 }), taille, &police, "white", 2, &y)?;
    }
    if !s.sous_titre.is_empty() {
        let taille = (wo as f64 * if vertical { 0.038 } else { 0.024 }).round();
        let y = move |bloc: f64| if deux { "h/2+h*0.012".to_string() } else { format!("(h-{bloc})/2") };
        poser("sous_titre", couper_lignes(&s.sous_titre, if vertical { 30 } else { 50 }), taille, &police_sous, "white@0.9", 1, &y)?;
    }
    let dessin = if textes.is_empty() { "null".to_string() } else { textes.join(",") };
    fc.push(format!("[gc{k}]{dessin},{finir}[gv{k}]"));
    silence(args, fc, entree);
    Ok(())
}

// ---------------------------------------------------------------- assemblage

fn fenetres(intervalles: &[(f64, f64)]) -> String {
    intervalles.iter().map(|(a, b)| format!("between(t,{a:.3},{b:.3})")).collect::<Vec<_>>().join("+")
}

async fn bruitage(etat: &EtatPartage) -> Result<PathBuf> {
    let f = etat.cfg.donnees.join("sons").join("whoosh.wav");
    if !f.exists() {
        tokio::fs::create_dir_all(f.parent().unwrap()).await?;
        ffmpeg_simple(vec![
            "-f".into(), "lavfi".into(), "-i".into(), "anoisesrc=d=0.6:c=pink:a=0.6".into(),
            "-af".into(), "highpass=f=300,lowpass=f=5000,afade=t=in:st=0:d=0.35:curve=exp,afade=t=out:st=0.35:d=0.25,volume=0.7".into(),
            "-ar".into(), "48000".into(), "-ac".into(), "2".into(), f.to_string_lossy().into(),
        ])
        .await?;
    }
    Ok(f)
}

/// Chaîne de la voix, de l'entrée `entree` à la sortie `sortie` du graphe de filtres : niveau, bruit, écho,
/// timbre, sifflantes, acoustique de lieu, calage. Le rendu final et l'écoute d'essai la jouent en entier
/// (`entiere`). L'aperçu n'en garde que la base (niveau, bruit, calage) : le navigateur joue le reste en direct,
/// à partir des mêmes filtres (`filtres_direct`).
fn graphe_voix(r: &crate::reglages::Reglages, gain: f64, entree: &str, sortie: &str, ir: Option<&(PathBuf, f64)>, entiere: bool,
               timbre: &[String]) -> Vec<String> {
    let mut a = filtres_base(r, gain);
    if entiere {
        a.extend(filtres_direct(r, timbre));
    }
    let ir = ir.filter(|_| entiere);
    // Calage demandé par la personne : positif = son retardé, négatif = son avancé.
    let calage_ms = nombre(r, "calage_son").round() as i64;
    let calage = if calage_ms > 0 {
        format!(",adelay=delays={calage_ms}:all=1")
    } else if calage_ms < 0 {
        let n = (-calage_ms) * 48;
        format!(",atrim=start_sample={n},asetpts=PTS-STARTPTS,apad=pad_len={n}")
    } else {
        String::new()
    };
    let chaine = a.join(",");
    match ir {
        // Acoustique d'un lieu : la voix sèche, plus sa convolution par la réponse du lieu, dosée.
        Some((fichier, part)) => vec![
            format!("[{entree}]{chaine},asplit=2[vsec][vmou]"),
            format!("amovie=filename={},aresample=48000,aformat=channel_layouts=stereo[vir]", fichier.to_string_lossy()),
            // irnorm=2 : réponse normalisée en énergie ; elle ressort alors 9 dB au-dessus de la voix (mesuré le
            // 2026-10-26), que le dosage du lieu ramène sous elle.
            "[vmou][vir]afir=irnorm=2[vrev]".into(),
            format!("[vsec][vrev]amix=inputs=2:weights='1 {part:.3}':normalize=0{calage}[{sortie}]"),
        ],
        None => vec![format!("[{entree}]{chaine}{calage}[{sortie}]")],
    }
}

/// Base de la voix, inscrite dans l'aperçu comme dans le rendu : niveau connu, réduction du bruit.
fn filtres_base(r: &crate::reglages::Reglages, gain: f64) -> Vec<String> {
    // asetpts : le son suit son propre compte d'échantillons, sans hériter des arrondis de la concaténation.
    let mut a: Vec<String> = vec!["aresample=48000".into(), "asetpts=N/SR/TB".into()];
    // La voix est d'abord ramenée à un niveau connu (−23 LUFS) : les seuils du compresseur et de la porte sont
    // réglés pour ce niveau. Sans cela, un rush enregistré bas (−43 LUFS) passait sous tous les seuils.
    a.push(format!("volume={gain:.1}dB"));
    // afftdn retarde le son d'exactement 1 200 échantillons (mesuré au témoin flash + bip) : on les retire.
    let compensation = format!(",atrim=start_sample={LATENCE_AFFTDN},asetpts=PTS-STARTPTS,apad=pad_len={LATENCE_AFFTDN}");
    match texte(r, "debruitage").as_str() {
        "leger" => a.push(format!("afftdn=nr=10:nf=-40:tn=1{compensation}")),
        "fort" => a.push(format!("afftdn=nr=22:nf=-35:tn=1{compensation}")),
        _ => {}
    }
    a
}

/// Réglages du son joués en direct par le navigateur sur l'aperçu (ils ne changent pas l'aperçu rendu).
pub const REGLAGES_SON_DIRECT: &[&str] = &["reverberation", "voix", "timbre_voix", "harmoniques", "deesseur", "espace",
                                           "reference_timbre", "reference_media", "intensite_timbre"];

/// Partie de la chaîne de la voix jouée en direct sur l'aperçu : porte de traîne, timbre, sifflantes. Le navigateur
/// (web/son-dsp.js) exécute ces mêmes filtres ffmpeg ; il n'en connaît que ceux-ci : highpass, lowpass,
/// equalizer, bass, treble, agate, acompressor, deesser, aexciter.
pub fn filtres_direct(r: &crate::reglages::Reglages, timbre: &[String]) -> Vec<String> {
    let mut a = Vec::new();
    // Écho de la pièce : une porte douce baisse la traîne entre les phrases, après avoir retiré le grave boueux.
    match texte(r, "reverberation").as_str() {
        "attenuer" => a.push("highpass=f=90,equalizer=f=300:t=q:w=1:g=-3,agate=threshold=0.015:ratio=3:range=0.2:attack=3:release=120:knee=2".into()),
        "forte" => a.push("highpass=f=100,equalizer=f=300:t=q:w=1:g=-4,equalizer=f=600:t=q:w=1.2:g=-2,\
                           agate=threshold=0.02:ratio=5:range=0.1:attack=2:release=90:knee=1.5".into()),
        _ => {}
    }
    // Égaliseur de correspondance (référence de timbre) : avant le timbre fixe, qui ne fait plus que la finition.
    a.extend(timbre.iter().cloned());
    if vrai(r, "voix") {
        a.push(match texte(r, "timbre_voix").as_str() {
            // Studio chaleureux : passe-haut, boue retirée, chaleur, présence et air mesurés (l'égaliseur de référence
            // fait le gros), niveleur lent puis compresseur de crêtes, sifflantes, harmoniques douces (rondeur).
            "studio_chaud" => "highpass=f=75,equalizer=f=350:t=q:w=1.2:g=-2,bass=g=2.5:f=170:w=0.8,\
                               equalizer=f=3200:t=q:w=1.3:g=1.5,treble=g=1:f=11000:w=0.7,\
                               acompressor=threshold=-24dB:ratio=2:attack=40:release=400:knee=6,\
                               acompressor=threshold=-16dB:ratio=3.5:attack=8:release=120:knee=3:makeup=3,\
                               deesser=i=0.35,aexciter=amount=0.4:drive=5:freq=3000:blend=2",
            // Chaud et profond : du corps dans le bas (150 Hz), moins de nasal (400 Hz), une présence douce, des
            // aigus adoucis, une compression lente qui ne pompe pas.
            "chaud" => "highpass=f=70,bass=g=3.5:f=150:w=0.7,equalizer=f=420:t=q:w=1.3:g=-2,\
                        equalizer=f=2800:t=q:w=1.4:g=1.5,treble=g=-3:f=7500:w=0.6,deesser=i=0.3,\
                        acompressor=threshold=-22dB:ratio=2.5:attack=20:release=250:knee=4:makeup=2.5",
            "naturel" => "highpass=f=70,acompressor=threshold=-22dB:ratio=2:attack=15:release=200:makeup=1.5",
            // Voix de studio : présence, air, compression rapide, très régulière (la normalisation finale borne les
            // crêtes ; un limiteur retardait le son de 5 ms).
            "podcast" => "highpass=f=80,equalizer=f=200:t=q:w=1:g=-1.5,equalizer=f=3000:t=q:w=1.5:g=2.5,\
                          equalizer=f=10000:t=q:w=1:g=1.5,deesser=i=0.3,\
                          acompressor=threshold=-26dB:ratio=4:attack=5:release=120:knee=3:makeup=3",
            "graves" => "highpass=f=60,bass=g=6:f=120:w=0.8,acompressor=threshold=-22dB:ratio=2.5:attack=15:release=200:makeup=2",
            "aigus_doux" => "highpass=f=70,treble=g=-6:f=6000:w=0.6,deesser=i=0.4,\
                             acompressor=threshold=-22dB:ratio=2.5:attack=15:release=200:makeup=2",
            "radio" => "highpass=f=200,lowpass=f=5000,equalizer=f=1500:t=q:w=1:g=3,\
                        acompressor=threshold=-26dB:ratio=6:attack=3:release=80:makeup=4",
            "telephone" => "highpass=f=300,lowpass=f=3400,equalizer=f=1800:t=q:w=1:g=4,\
                            acompressor=threshold=-26dB:ratio=6:attack=3:release=80:makeup=3",
            // Proche : l'effet de proximité d'un micro tout près (grave chaud), présence, compression marquée.
            "proche" => "highpass=f=60,bass=g=4:f=180:w=0.9,equalizer=f=3000:t=q:w=1.4:g=1,treble=g=-2:f=8000:w=0.6,\
                         acompressor=threshold=-24dB:ratio=3:attack=10:release=180:knee=3:makeup=2.5",
            // Extérieur : coupe le grondement du vent et de la circulation, garde l'intelligibilité.
            "exterieur" => "highpass=f=130,equalizer=f=250:t=q:w=1:g=-2,equalizer=f=3000:t=q:w=1.4:g=2.5,\
                            acompressor=threshold=-24dB:ratio=3:attack=8:release=150:makeup=2.5",
            _ => "highpass=f=80,equalizer=f=250:t=q:w=1:g=-2,equalizer=f=3500:t=q:w=1.2:g=3,\
                  acompressor=threshold=-20dB:ratio=3:attack=10:release=150:makeup=2",
        }
        .to_string());
    }
    // Harmoniques (excitateur) : présence et air ; mesuré sur la voix débruitée d'épisode 0 : +0,5 / +1,6 / +3,8 dB
    // entre 3 et 6 kHz, +2 / +5 / +9 dB au-dessus de 12 kHz, médium inchangé.
    let dose = match texte(r, "harmoniques").as_str() {
        "leger" => 0.5,
        "moyen" => 1.0,
        "fort" => 2.0,
        _ => 0.0,
    };
    if dose > 0.0 {
        a.push(format!("aexciter=amount={dose}:drive=6:freq=3500:blend=3"));
    }
    if vrai(r, "deesseur") {
        a.push("deesser=i=0.4".into());
    }
    // Une entrée de la liste peut tenir plusieurs filtres : on rend un filtre par entrée.
    a.iter().flat_map(|f| f.split(',').map(str::to_string).collect::<Vec<_>>()).collect()
}

/// Gain final du son en direct (dB), pour l'aperçu et les réglages actuels : ffmpeg joue sur tout l'aperçu la
/// partie directe de la chaîne et le fond, et mesure la sonie ; le gain amène le mélange à −14 LUFS, comme la
/// normalisation du rendu final. Gardé par réglages, à côté de l'aperçu.
pub async fn niveau_direct(etat: &EtatPartage, p: &Projet) -> Result<f64> {
    let a = p.apercu.as_ref().ok_or_else(|| anyhow!("pas encore d'aperçu"))?;
    let sorties = etat.dossier_projet(&p.id).join("sorties");
    let timbre = filtres_timbre(etat, &p.id, &p.reglages).await;
    let filtres = filtres_direct(&p.reglages, &timbre);
    let espace = texte(&p.reglages, "espace");
    let cle = format!("{}|{espace}", filtres.join(","));
    let memo = sorties.join(format!("{}.niveaux.json", a.fichier.trim_end_matches(".mp4")));
    let mut connus: serde_json::Map<String, Value> = tokio::fs::read(&memo).await.ok()
        .and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    if let Some(g) = connus.get(&cle).and_then(|v| v.as_f64()) {
        return Ok(g);
    }
    let ir = reponse_espace(etat, &espace).await?;
    let mut fc = graphe_direct(&filtres, "0:a", "vd", ir.as_ref());
    let mut args = vec!["-hide_banner".to_string(), "-nostats".into(), "-i".into(), sorties.join(&a.fichier).to_string_lossy().into()];
    let fond = a.rapport["son_direct"]["fond"].as_str().map(|f| sorties.join(f)).filter(|f| f.exists());
    if let Some(f) = &fond {
        args.extend(["-i".into(), f.to_string_lossy().into()]);
        fc.push("[vd][1:a]amix=inputs=2:duration=first:normalize=0[m]".into());
    } else {
        fc.push("[vd]anull[m]".into());
    }
    fc.push("[m]ebur128=framelog=quiet[o]".into());
    args.extend(["-filter_complex".into(), fc.join(";"), "-map".into(), "[o]".into(), "-f".into(), "null".into(), "-".into()]);
    let sortie = Command::new("ffmpeg").args(&args).stdin(Stdio::null()).output().await.context("ffmpeg introuvable")?;
    let texte_err = String::from_utf8_lossy(&sortie.stderr);
    let i: f64 = texte_err.lines().rev().find_map(|l| l.trim().strip_prefix("I:")?.trim().strip_suffix("LUFS")?.trim().parse().ok())
        .ok_or_else(|| anyhow!("mesure du niveau impossible : {}", texte_err.lines().rev().take(3).collect::<Vec<_>>().join(" | ")))?;
    let gain = ((-14.0 - i) * 10.0).round() / 10.0;
    connus.insert(cle, json!(gain));
    let _ = tokio::fs::write(&memo, serde_json::to_vec(&connus)?).await;
    Ok(gain)
}

/// La partie directe de la chaîne de la voix, avec le lieu, dans un graphe (mesure du niveau).
fn graphe_direct(filtres: &[String], entree: &str, sortie: &str, ir: Option<&(PathBuf, f64)>) -> Vec<String> {
    let chaine = if filtres.is_empty() { "anull".to_string() } else { filtres.join(",") };
    match ir {
        Some((fichier, part)) => vec![
            format!("[{entree}]aformat=channel_layouts=stereo,{chaine},asplit=2[dsec][dmou]"),
            format!("amovie=filename={},aresample=48000,aformat=channel_layouts=stereo[dir]", fichier.to_string_lossy()),
            "[dmou][dir]afir=irnorm=2[drev]".into(),
            format!("[dsec][drev]amix=inputs=2:weights='1 {part:.3}':normalize=0[{sortie}]"),
        ],
        None => vec![format!("[{entree}]aformat=channel_layouts=stereo,{chaine}[{sortie}]")],
    }
}

/// Fichier de la réponse d'un lieu, et son dosage (pour le son en direct du navigateur).
pub async fn lieu_direct(etat: &EtatPartage, nom: &str) -> Result<Option<(PathBuf, f64)>> {
    reponse_espace(etat, nom).await
}

/// Part de la voix débruitée par l'IA selon le réglage « Retirer l'écho de la voix » (None : pas de traitement).
fn part_ia(force: &str) -> Option<f64> {
    match force {
        "douce" => Some(0.6),
        "forte" => Some(1.0),
        _ => None,
    }
}

/// Voix séchée par IA (capteurs/seche_ia.py, MossFormer2 : pièce et bruit retirés par masque, aucun mot inventé),
/// sous bail de la carte graphique. A remplacé le débruiteur de Resemble (rehausse.py), qui ne retirait pas la
/// réverbération (banc du 2026-10-27 : RT60 apparent 360 → 130 ms contre 345 ms).
async fn secher(etat: &EtatPartage, projet: &str, entree: &Path, sortie: &Path, part: f64, duree_s: f64) -> Result<()> {
    let bail = etat
        .carte
        .prendre(etat, projet, "other:sechage", carte::VRAM_SECHAGE_MO, (duree_s / 20.0 + 90.0) as u64)
        .await?;
    let res = capteur(etat, projet, "seche_ia.py", &[entree.to_string_lossy().into(), sortie.to_string_lossy().into(), format!("{part}")]).await;
    etat.carte.rendre(bail).await;
    res
}

/// Timbre reconstruit par IA à partir d'une voix débruitée (restaure.py), puis vérifié mot à mot (verifie_voix.py) :
/// là où un mot a changé, la voix débruitée reste. Chaque étape sous son bail. `debut` : instant du rush où
/// commencent les fichiers (écoute d'essai).
async fn reconstruire(etat: &EtatPartage, projet: &str, debruitee: &Path, analyse: &Path, sortie: &Path, debut: f64, duree_s: f64,
                      bilan: &Path) -> Result<()> {
    let brute = sortie.with_extension("brute.wav");
    let bail = etat.carte.prendre(etat, projet, "other:restauration", carte::VRAM_RESTAURATION_MO, (duree_s / 40.0 + 60.0) as u64).await?;
    let res = capteur(etat, projet, "restaure.py", &[debruitee.to_string_lossy().into(), brute.to_string_lossy().into()]).await;
    etat.carte.rendre(bail).await;
    res?;
    let bail = etat.carte.prendre(etat, projet, "other:transcription", carte::VRAM_TRANSCRIPTION_MO, (duree_s / 6.0 + 120.0) as u64).await?;
    let res = capteur(etat, projet, "verifie_voix.py", &[brute.to_string_lossy().into(), debruitee.to_string_lossy().into(),
                                                         analyse.to_string_lossy().into(), sortie.to_string_lossy().into(), format!("{debut}"),
                                                         bilan.to_string_lossy().into()]).await;
    etat.carte.rendre(bail).await;
    let _ = tokio::fs::remove_file(&brute).await;
    res
}

/// Voix au timbre reconstruit d'un rush (sur la voix débruitée à fond), calculée une fois puis gardée, avec son
/// bilan (`voix_reconstruite.json` : voix gardée, part des mots identiques). Une voix d'avant ce bilan était
/// raccordée mot à mot à la voix débruitée : elle est refaite.
async fn voix_reconstruite(etat: &EtatPartage, projet: &str, rush: &str, duree_s: f64) -> Result<PathBuf> {
    let base = voix_sans_echo(etat, projet, rush, "forte", duree_s).await?.ok_or_else(|| anyhow!("voix débruitée indisponible"))?;
    let dossier = etat.dossier_travail(projet, rush);
    let f = dossier.join("voix_reconstruite.wav");
    let bilan = dossier.join("voix_reconstruite.json");
    if !f.exists() || !bilan.exists() {
        let provisoire = dossier.join("voix_reconstruite.en-cours.wav");
        reconstruire(etat, projet, &base, &dossier.join("analyse.json"), &provisoire, 0.0, duree_s, &bilan).await?;
        tokio::fs::rename(&provisoire, &f).await?;
    }
    Ok(f)
}

/// Bilan de la voix reconstruite des rushs du projet (voix gardée, accord des mots), pour le rapport.
fn bilans_voix(etat: &EtatPartage, p: &Projet) -> Value {
    json!(p.rushs.iter().filter_map(|r| {
        let t = std::fs::read_to_string(etat.dossier_travail(&p.id, &r.id).join("voix_reconstruite.json")).ok()?;
        serde_json::from_str::<Value>(&t).ok()
    }).collect::<Vec<_>>())
}

/// Son du rush sans la pièce ni le bruit (voix séchée par IA), calculé une fois par rush et par dosage.
async fn voix_sans_echo(etat: &EtatPartage, projet: &str, rush: &str, force: &str, duree_s: f64) -> Result<Option<PathBuf>> {
    let Some(part) = part_ia(force) else { return Ok(None) };
    let dossier = etat.dossier_travail(projet, rush);
    let f = dossier.join(format!("voix_seche_{force}.wav"));
    if !f.exists() {
        let provisoire = dossier.join(format!("voix_seche_{force}.en-cours.wav"));
        secher(etat, projet, &dossier.join("mezz.mkv"), &provisoire, part, duree_s).await?;
        tokio::fs::rename(&provisoire, &f).await?;
    }
    Ok(Some(f))
}

/// Fichier de la voix telle qu'elle entre dans la chaîne directe (séchée si le réglage le demande et si elle est
/// déjà calculée, sinon le rush), et sa clé pour les spectres.
fn source_timbre(etat: &EtatPartage, projet: &str, rush: &str, r: &crate::reglages::Reglages) -> (PathBuf, String) {
    let dossier = etat.dossier_travail(projet, rush);
    let force = texte(r, "dereverberation");
    let seche = dossier.join(format!("voix_seche_{force}.wav"));
    if part_ia(&force).is_some() && seche.exists() {
        (seche, format!("seche_{force}"))
    } else {
        (dossier.join("mezz.mkv"), "brut".into())
    }
}

/// Spectre moyen d'une voix (egaliseur.py spectre), gardé dans `sortie` ; `analyse` et `debut` isolent les mots.
async fn spectre_voix(etat: &EtatPartage, projet: &str, son: &Path, sortie: &Path, analyse: Option<&Path>, debut: f64) -> Result<()> {
    if sortie.exists() {
        return Ok(());
    }
    let mut args = vec![son.to_string_lossy().into_owned(), sortie.to_string_lossy().into_owned()];
    if let Some(a) = analyse {
        args.extend([a.to_string_lossy().into_owned(), format!("{debut}")]);
    }
    capteur(etat, projet, "egaliseur.py", &[&["spectre".to_string()][..], &args[..]].concat()).await
}

/// Référence « cette voix, sans la pièce » : spectre d'une minute de la voix reconstruite par VoiceFixer (générative :
/// on ne garde que son spectre, jamais son son). Calculée à la demande, gardée dans le dossier du rush.
pub async fn calculer_reference_cette_voix(etat: &EtatPartage, projet: &str) -> Result<()> {
    let p = etat.projet(projet).await?;
    let rush = p.rushs.first().map(|r| r.id.clone()).ok_or_else(|| anyhow!("aucun rush"))?;
    let a = etat.analyse(projet, &rush).await?;
    let dossier = etat.dossier_travail(projet, &rush);
    let sortie = dossier.join("spectre_reference_cette_voix.json");
    let debut = a.mots.first().map(|m| (m.t0 - 0.3).max(0.0)).unwrap_or(0.0);
    let duree = 60f64.min(a.duree - debut);
    let (source, _) = source_timbre(etat, projet, &rush, &p.reglages);
    let extrait = dossier.join("reference.extrait.wav");
    let seche = dossier.join("reference.seche.wav");
    let refaite = dossier.join("reference.reconstruite.wav");
    ffmpeg_simple(vec!["-ss".into(), format!("{debut:.3}"), "-t".into(), format!("{duree:.3}"), "-i".into(), source.to_string_lossy().into(),
                       "-vn".into(), "-ar".into(), "48000".into(), "-ac".into(), "2".into(), "-c:a".into(), "pcm_s16le".into(), extrait.to_string_lossy().into()]).await?;
    let res = async {
        secher(etat, projet, &extrait, &seche, 1.0, duree).await?;
        let bail = etat.carte.prendre(etat, projet, "other:restauration", carte::VRAM_RESTAURATION_MO, (duree / 40.0 + 60.0) as u64).await?;
        let r = capteur(etat, projet, "restaure.py", &[seche.to_string_lossy().into(), refaite.to_string_lossy().into()]).await;
        etat.carte.rendre(bail).await;
        r?;
        let _ = tokio::fs::remove_file(&sortie).await;
        spectre_voix(etat, projet, &refaite, &sortie, Some(&dossier.join("analyse.json")), debut).await
    }.await;
    for f in [&extrait, &seche, &refaite] {
        let _ = tokio::fs::remove_file(f).await;
    }
    // Les courbes calculées avec l'ancienne référence ne valent plus.
    if let Ok(mut lus) = tokio::fs::read_dir(&dossier).await {
        while let Ok(Some(e)) = lus.next_entry().await {
            if e.file_name().to_string_lossy().starts_with("timbre_") && e.file_name().to_string_lossy().contains("_cette_voix_") {
                let _ = tokio::fs::remove_file(e.path()).await;
            }
        }
    }
    res
}

/// La référence de timbre est-elle prête (sa courbe peut-elle se calculer) ?
pub fn reference_prete(etat: &EtatPartage, p: &Projet) -> bool {
    let Some(rush) = p.rushs.first() else { return false };
    match texte(&p.reglages, "reference_timbre").as_str() {
        "cette_voix" => etat.dossier_travail(&p.id, &rush.id).join("spectre_reference_cette_voix.json").exists(),
        "studio" => true,
        "media" => p.medias.iter().any(|m| m.id == texte(&p.reglages, "reference_media") && m.genre == "son"),
        _ => false,
    }
}

/// Filtres de l'égaliseur de correspondance (biquads, joués à l'identique en direct) pour les réglages `r` : la voix
/// du premier rush, telle qu'elle entre dans la chaîne, ramenée vers la référence choisie, à l'intensité voulue.
/// Vide si aucune référence, ou si elle n'est pas encore calculée.
pub async fn filtres_timbre(etat: &EtatPartage, projet: &str, r: &crate::reglages::Reglages) -> Vec<String> {
    match filtres_timbre_interne(etat, projet, r).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("égaliseur de timbre de {projet} : {e}");
            vec![]
        }
    }
}

async fn filtres_timbre_interne(etat: &EtatPartage, projet: &str, r: &crate::reglages::Reglages) -> Result<Vec<String>> {
    let reference = texte(r, "reference_timbre");
    let intensite = (nombre(r, "intensite_timbre") / 100.0).clamp(0.0, 1.0);
    if reference == "aucune" || intensite <= 0.0 {
        return Ok(vec![]);
    }
    let p = etat.projet(projet).await?;
    if !reference_prete(etat, &p) {
        return Ok(vec![]);
    }
    let rush = p.rushs.first().map(|x| x.id.clone()).ok_or_else(|| anyhow!("aucun rush"))?;
    let dossier = etat.dossier_travail(projet, &rush);
    let (source, cle_source) = source_timbre(etat, projet, &rush, r);
    let spectre_source = dossier.join(format!("spectre_{cle_source}.json"));
    spectre_voix(etat, projet, &source, &spectre_source, Some(&dossier.join("analyse.json")), 0.0).await?;
    let (spectre_ref, cle_ref) = match reference.as_str() {
        "cette_voix" => (dossier.join("spectre_reference_cette_voix.json"), "cette_voix".to_string()),
        "studio" => (etat.cfg.racine.join("bibliotheque").join("references_voix").join("studio.json"), "studio".to_string()),
        _ => {
            let id = texte(r, "reference_media");
            let m = p.medias.iter().find(|m| m.id == id).ok_or_else(|| anyhow!("son de référence introuvable"))?;
            let s = dossier.join(format!("spectre_media_{id}.json"));
            spectre_voix(etat, projet, &etat.dossier_projet(projet).join("medias").join(&m.fichier), &s, None, 0.0).await?;
            (s, format!("media_{id}"))
        }
    };
    let courbe = dossier.join(format!("timbre_{cle_source}_{cle_ref}_{}.json", (intensite * 100.0).round() as u32));
    if !courbe.exists() {
        capteur(etat, projet, "egaliseur.py", &["courbe".into(), spectre_source.to_string_lossy().into(), spectre_ref.to_string_lossy().into(),
                                                  format!("{intensite:.2}"), courbe.to_string_lossy().into()]).await?;
    }
    let v: Value = serde_json::from_str(&tokio::fs::read_to_string(&courbe).await?)?;
    Ok(v["filtres"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(String::from)).collect())
}

/// Écoute d'essai : `duree` secondes d'un rush à partir de `debut`, traitées avec les réglages de son donnés, en
/// MP3. Tout se fait sur le processeur ; le résultat est gardé (même extrait, mêmes réglages : même fichier).
pub async fn essai_son(etat: &EtatPartage, projet: &str, rush: &str, debut: f64, duree: f64, r: &crate::reglages::Reglages) -> Result<Vec<u8>> {
    let cles = ["debruitage", "dereverberation", "restauration", "reverberation", "voix", "timbre_voix", "harmoniques", "deesseur", "espace",
                "calage_son", "reference_timbre", "reference_media", "intensite_timbre"];
    let timbre = filtres_timbre(etat, projet, r).await;
    let empreinte = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!("{debut:.2}-{duree:.2}-{}|{}", cles.iter().map(|c| r.get(*c).map(|v| v.to_string()).unwrap_or_default()).collect::<Vec<_>>().join("|"),
                timbre.join(","))
            .hash(&mut h);
        format!("{:016x}", h.finish())
    };
    let dossier = etat.dossier_travail(projet, rush).join("essais");
    let mp3 = dossier.join(format!("{empreinte}.mp3"));
    if let Ok(b) = tokio::fs::read(&mp3).await {
        return Ok(b);
    }
    tokio::fs::create_dir_all(&dossier).await?;
    let extrait = dossier.join(format!("{empreinte}.wav"));
    ffmpeg_simple(vec!["-ss".into(), format!("{debut:.3}"), "-t".into(), format!("{duree:.3}"), "-i".into(),
                       etat.dossier_travail(projet, rush).join("mezz.mkv").to_string_lossy().into(),
                       "-vn".into(), "-ar".into(), "48000".into(), "-c:a".into(), "pcm_s16le".into(), extrait.to_string_lossy().into()])
        .await?;
    let mut source = extrait.clone();
    if vrai(r, "restauration") {
        let sans = dossier.join(format!("{empreinte}-sans-echo.wav"));
        secher(etat, projet, &extrait, &sans, 1.0, duree).await?;
        let refaite = dossier.join(format!("{empreinte}-reconstruite.wav"));
        reconstruire(etat, projet, &sans, &etat.dossier_travail(projet, rush).join("analyse.json"), &refaite, debut, duree,
                     &dossier.join(format!("{empreinte}-bilan.json"))).await?;
        let _ = tokio::fs::remove_file(&sans).await;
        source = refaite;
    } else if let Some(part) = part_ia(&texte(r, "dereverberation")) {
        let sans = dossier.join(format!("{empreinte}-sans-echo.wav"));
        secher(etat, projet, &extrait, &sans, part, duree).await?;
        source = sans;
    }
    let gain = sonie_rush(etat, projet, rush).await.map(|i| (-23.0 - i).clamp(-12.0, 30.0)).unwrap_or(0.0);
    let ir = reponse_espace(etat, &texte(r, "espace")).await?;
    let mut fc = graphe_voix(r, gain, "0:a", "voix", ir.as_ref(), true, &timbre);
    fc.push("[voix]loudnorm=I=-14:TP=-1.5:LRA=11,aresample=48000[a]".into());
    ffmpeg_simple(vec!["-i".into(), source.to_string_lossy().into(), "-filter_complex".into(), fc.join(";"),
                       "-map".into(), "[a]".into(), "-c:a".into(), "libmp3lame".into(), "-q:a".into(), "3".into(), mp3.to_string_lossy().into()])
        .await?;
    for f in [&extrait, &source] {
        let _ = tokio::fs::remove_file(f).await;
    }
    Ok(tokio::fs::read(&mp3).await?)
}

/// Lieux que l'on peut prêter à la voix : durée de la traîne (s), passe-bas (Hz), pré-délai (ms), dosage. Le
/// dosage met la résonance sous la voix sèche : −18 dB (petite pièce), −12, −9, −6 dB (cathédrale).
fn lieu(nom: &str) -> Option<(f64, u32, u32, f64)> {
    let dosage = |sous_la_voix: f64| 10f64.powf((sous_la_voix - 9.0) / 20.0);
    match nom {
        // Studio : une pièce sèche et proche, posée sur la voix dont on a retiré la vraie pièce (« enveloppant »).
        "studio" => Some((0.18, 6000, 8, dosage(-22.0))),
        "chambre" => Some((0.3, 7000, 3, dosage(-18.0))),
        "conference" => Some((1.0, 5000, 15, dosage(-12.0))),
        "salle" => Some((1.8, 4500, 25, dosage(-9.0))),
        "cathedrale" => Some((4.5, 3500, 45, dosage(-6.0))),
        _ => None,
    }
}

/// Réponse impulsionnelle d'un lieu (bruit rose stéréo qui s'éteint), fabriquée une fois puis gardée.
async fn reponse_espace(etat: &EtatPartage, nom: &str) -> Result<Option<(PathBuf, f64)>> {
    let Some((duree, passe_bas, predelai, part)) = lieu(nom) else { return Ok(None) };
    let f = etat.cfg.donnees.join("sons").join(format!("lieu_{nom}.wav"));
    if !f.exists() {
        tokio::fs::create_dir_all(f.parent().unwrap()).await?;
        ffmpeg_simple(vec![
            "-filter_complex".into(),
            format!("anoisesrc=d={duree}:c=pink:r=48000:a=0.8:s=11[g];anoisesrc=d={duree}:c=pink:r=48000:a=0.8:s=23[d];\
                     [g][d]amerge=inputs=2,afade=t=out:st=0:d={duree}:curve=exp,lowpass=f={passe_bas},\
                     adelay=delays={predelai}:all=1"),
            "-ac".into(), "2".into(), f.to_string_lossy().into(),
        ])
        .await?;
    }
    Ok(Some((f, part)))
}

/// Sonie intégrée d'un rush (LUFS), mesurée une fois sur sa version de travail puis gardée.
async fn sonie_rush(etat: &EtatPartage, projet: &str, rush: &str) -> Option<f64> {
    let dossier = etat.dossier_travail(projet, rush);
    let memo = dossier.join("sonie.txt");
    if let Some(v) = tokio::fs::read_to_string(&memo).await.ok().and_then(|s| s.trim().parse().ok()) {
        return Some(v);
    }
    let sortie = Command::new("ffmpeg")
        .args(["-hide_banner", "-nostats", "-i"])
        .arg(dossier.join("mezz.mkv"))
        .args(["-vn", "-af", "ebur128=framelog=quiet", "-f", "null", "-"])
        .stdin(Stdio::null())
        .output()
        .await
        .ok()?;
    let texte = String::from_utf8_lossy(&sortie.stderr);
    let i: f64 = texte.lines().rev().find_map(|l| l.trim().strip_prefix("I:")?.trim().strip_suffix("LUFS")?.trim().parse().ok())?;
    let _ = tokio::fs::write(&memo, format!("{i}")).await;
    Some(i)
}

/// Gain qui amène la voix du montage à −23 LUFS (moyenne des rushs, pondérée par leur temps dans le montage).
async fn gain_voix(etat: &EtatPartage, p: &Projet, prog: &Programme) -> f64 {
    let mut temps: HashMap<&str, f64> = HashMap::new();
    for pl in &prog.plans {
        *temps.entry(pl.rush.as_str()).or_default() += pl.duree;
    }
    let (mut somme, mut poids) = (0.0, 0.0);
    for (rush, t) in temps {
        if let Some(i) = sonie_rush(etat, &p.id, rush).await.filter(|i| i.is_finite() && *i > -70.0) {
            somme += i * t;
            poids += t;
        }
    }
    if poids == 0.0 { 0.0 } else { (-23.0 - somme / poids).clamp(-12.0, 30.0) }
}

/// Empreinte de tout ce qui change une vidéo rendue : un aperçu dont l'empreinte diffère de celle du projet
/// n'est plus à jour (l'interface le dit, même après un rechargement de la page).
pub fn empreinte(p: &Projet) -> String {
    // Ce que l'aperçu dessine ou joue en direct ne le rend pas périmé : son (partie directe), sous-titres (réglages,
    // mots affichés, emphase), titres des parties (le début des parties, lui, compte : il pose une transition),
    // textes et images posés (sauf le fond vert et le son d'une vidéo, inscrits dans l'aperçu), habillage.
    let mut reglages = p.reglages.clone();
    for c in REGLAGES_SON_DIRECT.iter().chain(&["sous_titres", "style_sous_titres", "position_sous_titres", "titres_chapitres",
                                                 "fondu_ouverture", "fondu_fin", "duree_fondus",
                                                 "cadence", "codec_final", "nettete_ia", "plage_dynamique"]) {
        reglages.remove(*c);
    }
    let decisions = p.montage.as_ref().map(|m| m.phrases.iter().map(|d| {
        json!([d.cle, d.garder, d.regime, d.cadre, d.transition, d.transition_choisie, d.mouvement])
    }).collect::<Vec<_>>());
    let parties = p.montage.as_ref().map(|m| m.chapitres.iter().map(|c| c.cle.clone()).collect::<Vec<_>>());
    let inscrites: Vec<_> = p.incrustations.iter().filter(|i| !i.effet.is_empty() || i.fond_vert || i.son).collect();
    let v = json!([reglages, decisions, p.montage.as_ref().map(|m| json!([m.accroche, m.accroche_choisie])), parties,
                   inscrites, p.generique, p.corrections, p.fusions,
                   p.medias.iter().map(|m| [&m.id, &m.fichier]).collect::<Vec<_>>(), p.rushs.iter().map(|r| &r.id).collect::<Vec<_>>()]);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.to_string().hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Le son appliqué, en clair, pour le rapport : on sait avec quoi l'aperçu a été fait.
pub fn resume_son(r: &crate::reglages::Reglages) -> String {
    let choix = |cle: &str| crate::reglages::libelle_choix(cle, &texte(r, cle)).unwrap_or_else(|| texte(r, cle));
    let mut morceaux = vec![format!("bruit : {}", choix("debruitage").to_lowercase()), format!("écho : {}", choix("reverberation").to_lowercase()),
                            format!("écho et bruit retirés par l'IA : {}", choix("dereverberation").to_lowercase())];
    if vrai(r, "restauration") {
        morceaux.push("timbre reconstruit par l'IA, vérifié mot à mot".into());
    }
    morceaux.push(if vrai(r, "voix") { format!("voix : {}", choix("timbre_voix").to_lowercase()) } else { "voix : sans traitement".into() });
    if texte(r, "reference_timbre") != "aucune" {
        morceaux.push(format!("timbre ramené vers « {} » à {:.0} %", choix("reference_timbre").to_lowercase(), nombre(r, "intensite_timbre")));
    }
    if vrai(r, "deesseur") {
        morceaux.push("« s » adoucis".into());
    }
    morceaux.join(" · ")
}

#[allow(clippy::too_many_arguments)]
async fn assembler(etat: &EtatPartage, p: &Projet, prog: &Programme, liste: &Path, ass: &Path, sortie: &Path,
                   passe: Passe, nvenc: bool) -> Result<()> {
    let apercu = !passe.dix_bits;
    let (ips, fmt) = (passe.ips, passe.format_overlay());
    let r = &p.reglages;
    let (wo, ho) = (prog.largeur, prog.hauteur);
    let total = prog.duree;
    let mut args: Vec<String> = vec!["-f".into(), "concat".into(), "-safe".into(), "0".into(), "-i".into(), liste.to_string_lossy().into()];
    let mut entree = 1usize;
    let mut fc: Vec<String> = Vec::new();

    // Image : cadence fixe, puis PostProd (couleur, teint, diffusion, netteté, grain…), puis les effets ponctuels.
    fc.push(format!("[0:v]setpts=N/({ips}*TB)[ppe]"));
    // L'aperçu sort sans PostProd : le navigateur l'applique en direct, chaque réglage se voit aussitôt. Le rendu
    // final, lui, l'inscrit dans l'image.
    // Au rendu final, la couleur passe par sa table en 16 bits (la même que l'aperçu en direct) : les filtres de
    // couleur de ffmpeg (eq, curves, colorbalance…) ne travaillent qu'en 8 bits.
    fc.push(if apercu {
        "[ppe]null[pps]".to_string()
    } else {
        // En HDR, la couleur de la PostProd (pensée en SDR) n'est pas appliquée ; le reste (teint, netteté, grain…) l'est.
        let mut pp = p.postprod.clone();
        if passe.hdr {
            (pp.couleur, pp.exposition, pp.contraste, pp.saturation, pp.temperature) = ("naturel".into(), 0.0, 0.0, 0.0, 0.0);
        }
        let table = table_couleurs_cube(etat, &pp).await?;
        pp.filtre_avec_table("ppe", "pps", wo, ho, table.as_deref())
    });
    let mut v = vec![];
    let effets = |nom: &str| prog.incrustations.iter().filter(|i| i.effet == nom).map(|i| (i.debut, i.fin)).collect::<Vec<_>>();
    let secousses = effets("secousse");
    if !secousses.is_empty() {
        let b = fenetres(&secousses);
        v.push(format!("crop=w=iw*0.94:h=ih*0.94:x='iw*0.03+({b})*iw*0.025*sin(t*63)':y='ih*0.03+({b})*ih*0.025*cos(t*71)',scale={wo}:{ho}"));
    }
    let nb = effets("noir_blanc");
    if !nb.is_empty() {
        v.push(format!("hue=s=0:enable='{}'", fenetres(&nb)));
    }
    fc.push(format!("[pps]{}[v0]", if v.is_empty() { "null".to_string() } else { v.join(",") }));
    let mut base = "v0".to_string();
    let mut sons_incrustes: Vec<String> = Vec::new();
    let marge = (0.04 * wo as f64).round() as u32;
    for (k, inc) in prog.incrustations.iter().filter(|i| i.effet.is_empty() && i.texte.trim().is_empty()).enumerate() {
        let Some(m) = p.medias.iter().find(|m| m.id == inc.media) else { continue };
        // Aperçu : l'image est dessinée en direct par le navigateur ; seuls restent rendus le fond vert (détourage)
        // et le son d'une vidéo, qui part dans la piste de fond.
        let direct = apercu && !inc.fond_vert;
        let avec_son = inc.son && m.a_son && m.genre == "video";
        if direct && !avec_son {
            continue;
        }
        let fichier = etat.dossier_projet(&p.id).join("medias").join(&m.fichier);
        let duree = format!("{:.3}", inc.fin - inc.debut);
        match m.genre.as_str() {
            "image" => args.extend(["-loop", "1", "-framerate", "30", "-t", &duree].map(String::from)),
            "gif" => args.extend(["-ignore_loop", "0", "-t", &duree].map(String::from)),
            "video" => args.extend(["-stream_loop", "-1", "-t", &duree].map(String::from)),
            _ => continue,
        }
        args.extend(["-i".into(), fichier.to_string_lossy().into()]);
        let plein = inc.position == "plein" && inc.x.is_none();
        let largeur = ((wo as f64 * inc.taille) as u32 / 2 * 2).max(16);
        let echelle = if plein {
            format!("scale={wo}:{ho}:force_original_aspect_ratio=increase,crop={wo}:{ho}")
        } else if inc.entree == "zoom" {
            // Le média grandit de 10 % à sa taille en 0,25 s, sans dépasser.
            format!("scale=w='max(2,trunc({largeur}*if(lt(t,0.25),max(0.1,1-pow(1-t/0.25,2)),1)/2)*2)':h=-2:eval=frame")
        } else if inc.entree == "splash" {
            // Le média grandit jusqu'à 115 %, puis se pose à sa taille (t est le temps propre du média).
            format!("scale=w='max(2,trunc({largeur}*if(lt(t,0.21),max(0.1,1.15*t/0.21),if(lt(t,0.3),1.15-0.15*(t-0.21)/0.09,1))/2)*2)':h=-2:eval=frame")
        } else {
            format!("scale={largeur}:-2")
        };
        let cle = if inc.fond_vert { ",chromakey=0x00FF00:0.13:0.08" } else { "" };
        let d = inc.fin - inc.debut;
        let fondu = if matches!(inc.entree.as_str(), "" | "fondu") {
            format!(",fade=t=in:st=0:d=0.15:alpha=1,fade=t=out:st={:.3}:d=0.15:alpha=1", (d - 0.15).max(0.0))
        } else if inc.entree == "aucune" {
            String::new()
        } else {
            format!(",fade=t=out:st={:.3}:d=0.15:alpha=1", (d - 0.15).max(0.0))
        };
        if !direct {
            fc.push(format!("[{entree}:v]fps={ips},{echelle},format=yuva420p{cle}{fondu},{},setpts=PTS-STARTPTS+{:.3}/TB[inc{k}]",
                            passe.vers_sortie_rgba(), inc.debut));
        }
        let (mut x, mut y) = match (inc.x, inc.y) {
            _ if plein => ("0".to_string(), "0".to_string()),
            (Some(cx), Some(cy)) => (format!("{:.1}-w/2", cx.clamp(0.0, 1.0) * wo as f64), format!("{:.1}-h/2", cy.clamp(0.0, 1.0) * ho as f64)),
            _ => match inc.position.as_str() {
                "haut_gauche" => (format!("{marge}"), format!("{marge}")),
                "haut_droite" => (format!("W-w-{marge}"), format!("{marge}")),
                "bas_gauche" => (format!("{marge}"), format!("H-h-{marge}")),
                "bas_droite" => (format!("W-w-{marge}"), format!("H-h-{marge}")),
                "haut" => ("(W-w)/2".to_string(), format!("{marge}")),
                "bas" => ("(W-w)/2".to_string(), format!("H-h-{}", marge * 6)),
                _ => ("(W-w)/2".to_string(), "(H-h)/2".to_string()),
            },
        };
        // Glissés : le média arrive du bord en 0,3 s (t est le temps de la vidéo).
        let reste = format!("pow(max(0,1-(t-{:.3})/0.3),2)", inc.debut);
        match inc.entree.as_str() {
            "glisse_gauche" => x = format!("({x})-(({x})+w)*{reste}"),
            "glisse_droite" => x = format!("({x})+(W-({x}))*{reste}"),
            "glisse_haut" => y = format!("({y})-(({y})+h)*{reste}"),
            "glisse_bas" => y = format!("({y})+(H-({y}))*{reste}"),
            // Secousse : le média tremble en arrivant, de moins en moins, pendant 0,45 s.
            "secousse" => {
                let amorti = format!("max(0,1-(t-{:.3})/0.45)", inc.debut);
                x = format!("({x})+W*0.025*sin((t-{:.3})*70)*{amorti}", inc.debut);
                y = format!("({y})+H*0.02*cos((t-{:.3})*83)*{amorti}", inc.debut);
            }
            _ => {}
        }
        // Entre apostrophes : les formules des arrivées (glissés, secousse) contiennent des virgules, que le graphe
        // prendrait sinon pour des séparateurs de filtres.
        if !direct {
            fc.push(format!("[{base}][inc{k}]overlay=x='{x}':y='{y}':enable='between(t,{:.3},{:.3})':eof_action=pass{fmt}[vi{k}]",
                            inc.debut, inc.fin));
            base = format!("vi{k}");
        }
        if avec_son {
            let ms = (inc.debut * 1000.0).round() as u64;
            fc.push(format!("[{entree}:a]aresample=48000,adelay=delays={ms}:all=1[si{k}]"));
            sons_incrustes.push(format!("si{k}"));
        }
        entree += 1;
    }
    // Habillage : les logos, sur toute la durée du montage (les textes passent par les sous-titres).
    for (j, hab) in p.habillage.iter().filter(|x| x.genre == "logo" && !apercu).enumerate() {
        let Some(m) = p.medias.iter().find(|m| m.id == hab.media) else { continue };
        let fichier = etat.dossier_projet(&p.id).join("medias").join(&m.fichier);
        if m.genre == "gif" {
            args.extend(["-ignore_loop", "0"].map(String::from));
        } else {
            args.extend(["-loop", "1", "-framerate", "30"].map(String::from));
        }
        args.extend(["-t".into(), format!("{total:.3}"), "-i".into(), fichier.to_string_lossy().into()]);
        let largeur = ((wo as f64 * hab.taille) as u32 / 2 * 2).max(16);
        fc.extend(graphe_logo(&format!("{entree}:v"), &base, &format!("vh{j}"), j, largeur, hab, marge, passe));
        base = format!("vh{j}");
        entree += 1;
    }
    let polices = etat.cfg.racine.join("modeles").join("polices");
    // Pixels carrés : un recadrage d'effet (secousse) laisse sinon une forme de pixel à peine faussée (4064:4059),
    // que l'enchaînement avec le générique refuse.
    fc.push(format!("[{base}]subtitles=f={}:fontsdir={},{},setsar=1[v]", ass.to_string_lossy(), polices.to_string_lossy(), passe.setparams()));

    // Son.
    let gain = gain_voix(etat, p, prog).await;
    let ir = reponse_espace(etat, &texte(r, "espace")).await?;
    let timbre = filtres_timbre(etat, &p.id, r).await;
    fc.extend(graphe_voix(r, gain, "0:a", "voix", ir.as_ref(), !apercu, &timbre));
    let mut melange = vec!["voix".to_string()];
    let musique = texte(r, "musique");
    if let Some(m) = p.medias.iter().find(|m| m.id == musique && nombre(r, "volume_musique") > 0.0) {
        let fichier = etat.dossier_projet(&p.id).join("medias").join(&m.fichier);
        args.extend(["-stream_loop".into(), "-1".into(), "-t".into(), format!("{total:.3}"), "-i".into(), fichier.to_string_lossy().into()]);
        let volume = nombre(r, "volume_musique") / 100.0 * 0.6;
        melange[0] = "voixm".into();
        fc.push("[voix]asplit=2[voixm][voixsc]".into());
        fc.push(format!(
            "[{entree}:a]aresample=48000,aformat=channel_layouts=stereo,volume={volume:.3},afade=t=out:st={:.3}:d=2[mus]",
            (total - 2.0).max(0.0)
        ));
        fc.push("[mus][voixsc]sidechaincompress=threshold=0.02:ratio=10:attack=15:release=350[musd]".into());
        melange.push("musd".into());
        entree += 1;
    }
    melange.extend(sons_incrustes);
    if vrai(r, "bruitage") && !prog.transitions.is_empty() {
        let w = bruitage(etat).await?;
        args.extend(["-i".into(), w.to_string_lossy().into()]);
        let n = prog.transitions.len();
        fc.push(format!("[{entree}:a]asplit={n}{}", (0..n).map(|i| format!("[w{i}]")).collect::<String>()));
        for (i, t) in prog.transitions.iter().enumerate() {
            let ms = ((t - 0.3).max(0.0) * 1000.0).round() as u64;
            fc.push(format!("[w{i}]adelay=delays={ms}:all=1,volume=0.6[wd{i}]"));
            melange.push(format!("wd{i}"));
        }
    }
    let (avant, apres) = durees_generique(p);
    if apercu {
        return assembler_apercu(etat, p, sortie, liste, args, fc, entree, melange, total, (avant, apres), (wo, ho), ass, nvenc, passe).await;
    }
    let mix = if melange.len() > 1 {
        fc.push(format!("{}amix=inputs={}:duration=first:normalize=0[mix]",
                        melange.iter().map(|m| format!("[{m}]")).collect::<String>(), melange.len()));
        "mix".to_string()
    } else {
        melange[0].clone()
    };
    // Le niveau final (−14 LUFS) se règle après coup, d'un gain fixe mesuré sur tout le rendu (normaliser_son) :
    // loudnorm en une passe est dynamique et faisait « pomper » la voix.
    fc.push(format!("[{mix}]aresample=48000[a]"));

    // Générique : une séquence avant le montage, une après, au même format, enchaînées en fondu.
    let (mut v_final, mut a_final) = ("v".to_string(), "a".to_string());
    if avant > 0.0 || apres > 0.0 {
        let tmp = ass.parent().unwrap_or(Path::new("."));
        let mut suite = String::new();
        let mut n = 1;
        if let (Some(s), true) = (&p.generique.debut, avant > 0.0) {
            sequence_generique(etat, p, s, avant, 0, (wo, ho), &mut args, &mut fc, &mut entree, tmp, passe)?;
            suite.push_str("[gv0][ga0]");
            n += 1;
        }
        suite.push_str("[v][a]");
        if let (Some(s), true) = (&p.generique.fin, apres > 0.0) {
            sequence_generique(etat, p, s, apres, 1, (wo, ho), &mut args, &mut fc, &mut entree, tmp, passe)?;
            suite.push_str("[gv1][ga1]");
            n += 1;
        }
        fc.push(format!("{suite}concat=n={n}:v=1:a=1[vg][ag]"));
        (v_final, a_final) = ("vg".to_string(), "ag".to_string());
    }
    let total = avant + total + apres;
    // Fondus d'ouverture et de fin, sur toute la vidéo (générique compris) : image et son ensemble. L'aperçu ne les
    // inscrit pas : le navigateur les joue en direct (textes-direct.js, son-direct.js).
    let (ouverture, fin) = (texte(r, "fondu_ouverture"), texte(r, "fondu_fin"));
    if ouverture != "aucun" || fin != "aucun" {
        let d = nombre(r, "duree_fondus").clamp(0.2, 3.0).min(total / 2.0);
        // Vers le noir : fade, qui garde les 10 bits ; vers le blanc : fondu_blanc (le fade en couleur repasse en 8 bits).
        let mut af = Vec::new();
        let mut v = v_final.clone();
        for (genre, entrant) in [(ouverture.as_str(), true), (fin.as_str(), false)] {
            if genre == "aucun" {
                continue;
            }
            let suite = format!("vfd{}", if entrant { 0 } else { 1 });
            let st = if entrant { 0.0 } else { total - d };
            if genre == "blanc" {
                let k = if entrant { format!("clip(1-T/{d:.3},0,1)") } else { format!("clip((T-{st:.3})/{d:.3},0,1)") };
                fc.push(fondu_blanc(&v, &suite, &format!("fb{}", if entrant { 0 } else { 1 }), &k, passe));
            } else {
                fc.push(format!("[{v}]fade=t={}:st={st:.3}:d={d:.3}[{suite}]", if entrant { "in" } else { "out" }));
            }
            af.push(format!("afade=t={}:st={st:.3}:d={d:.3}", if entrant { "in" } else { "out" }));
            v = suite;
        }
        fc.push(format!("[{a_final}]{}[afd]", af.join(",")));
        (v_final, a_final) = (v, "afd".to_string());
    }

    args.extend([
        "-filter_complex".into(), fc.join(";"),
        "-map".into(), format!("[{v_final}]"), "-map".into(), format!("[{a_final}]"),
        "-t".into(), format!("{total:.3}"),
    ]);
    // Le HDR exige le HEVC 10 bits (le H.264 10 bits ne se lit presque nulle part).
    let hevc = texte(r, "codec_final") != "h264" || passe.hdr;
    args.extend(encodeur(nvenc, if hevc { "final_hevc" } else { "assemblage" }));
    let brut = sortie.with_extension("brut.mkv");
    let (matrice, primaires, transfert) = if passe.hdr { ("bt2020nc", "bt2020", "arib-std-b67") } else { ("bt709", "bt709", "bt709") };
    args.extend([
        "-r".into(), format!("{}", passe.ips), "-fps_mode".into(), "cfr".into(),
        "-colorspace".into(), matrice.into(), "-color_primaries".into(), primaires.into(), "-color_trc".into(), transfert.into(),
        "-c:a".into(), "pcm_s24le".into(), "-ar".into(), "48000".into(), "-ac".into(), "2".into(),
        brut.to_string_lossy().into(),
    ]);
    lancer_assemblage(etat, p, &args, liste, total).await?;
    let res = normaliser_son(etat, &p.id, &brut, sortie, total, hevc).await;
    let _ = tokio::fs::remove_file(&brut).await;
    res
}

/// Niveau final du rendu : la sonie intégrée de tout le son est mesurée, puis portée à −14 LUFS d'un gain fixe ;
/// un limiteur ne retient que les crêtes au-dessus de −2 dBFS. L'image est recopiée telle quelle.
async fn normaliser_son(etat: &EtatPartage, projet: &str, brut: &Path, sortie: &Path, duree: f64, hevc: bool) -> Result<()> {
    etat.signaler(projet, "Mesure du niveau du son", 0.0);
    let mesure = Command::new("ffmpeg")
        .args(["-hide_banner", "-nostats", "-i"]).arg(brut)
        .args(["-vn", "-af", "loudnorm=I=-14:TP=-1.5:LRA=11:print_format=json", "-f", "null", "-"])
        .stdin(Stdio::null())
        .output()
        .await
        .context("ffmpeg introuvable")?;
    let texte = String::from_utf8_lossy(&mesure.stderr);
    // Le bilan de loudnorm est le dernier bloc entre accolades ; d'autres lignes peuvent le suivre.
    let json = texte.rfind('{').and_then(|i| texte[i..].find('}').map(|j| &texte[i..=i + j]))
        .and_then(|t| serde_json::from_str::<Value>(t).ok())
        .ok_or_else(|| anyhow!("mesure du niveau du son illisible"))?;
    let lire = |cle: &str| json[cle].as_str().and_then(|v| v.parse::<f64>().ok());
    let integre = lire("input_i").filter(|v| v.is_finite()).unwrap_or(-14.0);
    let gain = (-14.0 - integre).clamp(-20.0, 30.0);
    let mut args: Vec<String> = vec!["-i".into(), brut.to_string_lossy().into(), "-map".into(), "0:v".into(), "-map".into(), "0:a".into(),
                                     "-c:v".into(), "copy".into()];
    if hevc {
        args.extend(["-tag:v".into(), "hvc1".into()]);
    }
    args.extend([
        "-af".into(), format!("volume={gain:.2}dB,alimiter=limit={:.4}:attack=5:release=50:level=false,aresample=48000", 10f64.powf(-2.0 / 20.0)),
        "-c:a".into(), "aac".into(), "-b:a".into(), "256k".into(), "-ar".into(), "48000".into(), "-ac".into(), "2".into(),
        "-movflags".into(), "+faststart".into(), sortie.to_string_lossy().into(),
    ]);
    ffmpeg(etat, projet, "Niveau du son et finition", &args, duree).await
}

/// Réglages de sortie de la vidéo assemblée (image et son).
fn sortie_video(sortie: &Path, passe: Passe) -> Vec<String> {
    vec![
        "-r".into(), format!("{}", passe.ips), "-fps_mode".into(), "cfr".into(),
        "-colorspace".into(), "bt709".into(), "-color_primaries".into(), "bt709".into(), "-color_trc".into(), "bt709".into(),
        "-c:a".into(), "aac".into(), "-b:a".into(), "256k".into(), "-ar".into(), "48000".into(), "-ac".into(), "2".into(),
        "-movflags".into(), "+faststart".into(),
        sortie.to_string_lossy().into(),
    ]
}

async fn lancer_assemblage(etat: &EtatPartage, p: &Projet, args: &[String], liste: &Path, total: f64) -> Result<()> {
    // La commande reste à côté des plans : le dossier temporaire n'est gardé qu'en cas d'échec, pour le diagnostic.
    let _ = tokio::fs::write(liste.with_file_name("assemblage.json"), serde_json::to_vec_pretty(&args).unwrap_or_default()).await;
    ffmpeg(etat, &p.id, "Assemblage, son et sous-titres", args, total).await
}

/// Pistes à part d'un aperçu, à côté de sa vidéo : le fond (musique, bruitages, sons des médias), qui passe par le
/// gain final du son en direct, et le générique, déjà à son niveau.
pub fn pistes_apercu(video: &str) -> (String, String) {
    let base = video.trim_end_matches(".mp4");
    (format!("{base}.fond.m4a"), format!("{base}.generique.m4a"))
}

/// Assemblage d'un aperçu : la vidéo porte la voix de base (niveau, bruit, écho retiré, calage) ; le fond et le
/// générique sortent dans leurs propres fichiers. Le navigateur joue en direct le reste de la chaîne de la voix
/// et mélange les trois, chacun à son niveau. Toutes les pistes ont la durée de la vidéo, générique compris.
#[allow(clippy::too_many_arguments)]
async fn assembler_apercu(etat: &EtatPartage, p: &Projet, sortie: &Path, liste: &Path, mut args: Vec<String>, mut fc: Vec<String>,
                          mut entree: usize, melange: Vec<String>, total: f64, (avant, apres): (f64, f64), (wo, ho): (u32, u32),
                          ass: &Path, nvenc: bool, passe: Passe) -> Result<()> {
    const FORMAT: &str = "aformat=sample_fmts=fltp:sample_rates=48000:channel_layouts=stereo";
    let silence = |d: f64, nom: &str| format!("aevalsrc=0|0:c=stereo:s=48000:d={d:.3},{FORMAT}[{nom}]");
    fc.push(format!("[{}]{FORMAT}[vx]", melange[0]));
    let fond = melange.len() > 1;
    if fond {
        fc.push(format!("{}amix=inputs={}:duration=first:normalize=0,{FORMAT}[fd]",
                        melange[1..].iter().map(|m| format!("[{m}]")).collect::<String>(), melange.len() - 1));
    }
    let (mut v_final, mut voix, mut fond_final) = ("v".to_string(), "vx".to_string(), "fd".to_string());
    let mut generique = None;
    if avant > 0.0 || apres > 0.0 {
        let tmp = ass.parent().unwrap_or(Path::new("."));
        let (mut images, mut voixs, mut fonds, mut gens) = (String::new(), String::new(), String::new(), String::new());
        let mut n = 1;
        let morceau = |k: usize, d: f64, fc: &mut Vec<String>, voixs: &mut String, fonds: &mut String, gens: &mut String| {
            fc.push(silence(d, &format!("sv{k}")));
            fc.push(silence(d, &format!("sf{k}")));
            fc.push(format!("[ga{k}]{FORMAT}[gq{k}]"));
            voixs.push_str(&format!("[sv{k}]"));
            fonds.push_str(&format!("[sf{k}]"));
            gens.push_str(&format!("[gq{k}]"));
        };
        if let (Some(s), true) = (&p.generique.debut, avant > 0.0) {
            sequence_generique(etat, p, s, avant, 0, (wo, ho), &mut args, &mut fc, &mut entree, tmp, passe)?;
            images.push_str("[gv0]");
            morceau(0, avant, &mut fc, &mut voixs, &mut fonds, &mut gens);
            n += 1;
        }
        images.push_str("[v]");
        voixs.push_str("[vx]");
        if fond {
            fonds.push_str("[fd]");
        } else {
            fc.push(silence(total, "fdm"));
            fonds.push_str("[fdm]");
        }
        fc.push(silence(total, "gm"));
        gens.push_str("[gm]");
        let mut fin_gen = String::new();
        if let (Some(s), true) = (&p.generique.fin, apres > 0.0) {
            sequence_generique(etat, p, s, apres, 1, (wo, ho), &mut args, &mut fc, &mut entree, tmp, passe)?;
            images.push_str("[gv1]");
            morceau(1, apres, &mut fc, &mut voixs, &mut fonds, &mut fin_gen);
            n += 1;
        }
        gens.push_str(&fin_gen);
        // Les silences de fin de la voix et du fond suivent l'ordre des morceaux : [début][montage][fin].
        fc.push(format!("{images}concat=n={n}:v=1:a=0[vg]"));
        fc.push(format!("{voixs}concat=n={n}:v=0:a=1[vxg]"));
        fc.push(format!("{fonds}concat=n={n}:v=0:a=1[fdg]"));
        fc.push(format!("{gens}concat=n={n}:v=0:a=1[gg]"));
        (v_final, voix, fond_final) = ("vg".into(), "vxg".into(), "fdg".into());
        generique = Some("gg".to_string());
    }
    let total = avant + total + apres;
    args.extend(["-filter_complex".into(), fc.join(";"),
                 "-map".into(), format!("[{v_final}]"), "-map".into(), format!("[{voix}]"), "-t".into(), format!("{total:.3}")]);
    args.extend(encodeur(nvenc, "assemblage_apercu"));
    args.extend(sortie_video(sortie, passe));
    let nom = sortie.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let (f_fond, f_gen) = pistes_apercu(&nom);
    let piste = |etiquette: &str, fichier: &str| -> Vec<String> {
        vec!["-map".into(), format!("[{etiquette}]"), "-t".into(), format!("{total:.3}"), "-c:a".into(), "aac".into(),
             "-b:a".into(), "192k".into(), "-ar".into(), "48000".into(), "-ac".into(), "2".into(),
             sortie.with_file_name(fichier).to_string_lossy().into()]
    };
    if fond || generique.is_some() {
        args.extend(piste(&fond_final, &f_fond));
    }
    if let Some(g) = &generique {
        args.extend(piste(g, &f_gen));
    }
    lancer_assemblage(etat, p, &args, liste, total).await
}

/// Netteté IA (Real-ESRGAN, sous bail de la carte), en une passe, sur ceux des plans `indices` qui la demandent :
/// fichiers nets par indice de plan.
async fn nettete_ia(etat: &EtatPartage, p: &Projet, prog: &Programme, indices: &[usize],
                    sources: &std::collections::HashMap<String, (u32, u32)>, tmp: &Path, passe: Passe)
                    -> Result<std::collections::HashMap<usize, PathBuf>> {
    let projet = p.id.as_str();
    let mut nettes = std::collections::HashMap::new();
    let a_nettoyer: Vec<(usize, &Plan)> = indices.iter().map(|&i| (i, &prog.plans[i])).filter(|(_, pl)| pl.nettete).collect();
    if a_nettoyer.is_empty() {
        return Ok(nettes);
    }
    let visage = texte(&p.reglages, "nettete_ia") == "visage";
    let mut travaux = Vec::new();
    for (i, pl) in &a_nettoyer {
        let z = zone_entiere(pl, sources[&pl.rush]);
        let f = tmp.join(format!("net{i:04}.mkv"));
        travaux.push(json!({"entree": etat.dossier_travail(projet, &pl.rush).join("mezz.mkv"), "debut": pl.t0, "duree": pl.duree,
                            "crop": [z.x, z.y, z.w, z.h], "sortie": f, "visage": visage, "melange": MELANGE_IA,
                            "largeur_max": largeur_nette(pl, z.w, prog.largeur), "ips": passe.ips}));
        nettes.insert(*i, f);
    }
    let fichier = tmp.join("nettete.json");
    tokio::fs::write(&fichier, serde_json::to_vec(&travaux)?).await?;
    // Durée annoncée au bail : le temps croît avec la taille des zones (mesuré le 2026-10-26 : ~3 s de calcul par
    // seconde de plan pour 0,9 Mpx, ~8 s pour 2,8 Mpx en 4K).
    let secondes: f64 = a_nettoyer.iter().map(|(_, pl)| {
        let z = zone_entiere(pl, sources[&pl.rush]);
        pl.duree * (1.0 + 2.5 * (z.w * z.h) as f64 / 1e6)
    }).sum();
    let bail = etat
        .carte
        .prendre(etat, projet, "other:nettete", carte::VRAM_NETTETE_MO, (secondes * 1.5 + 120.0) as u64)
        .await?;
    let res = capteur(etat, projet, "nettete.py", &[fichier.to_string_lossy().into()]).await;
    etat.carte.rendre(bail).await;
    res?;
    Ok(nettes)
}

/// Quelques secondes du montage à l'instant `t` (temps du montage, générique exclu), en qualité finale : les plans
/// du rendu final (définition, netteté IA), sans PostProd, habillage, textes ni son, pour juger la netteté des
/// zooms à côté de l'aperçu. Écrit `sorties/qualite-<date>.mp4` (le précédent est effacé).
pub async fn plan_qualite(etat: &EtatPartage, projet: &str, t: f64) -> Result<Value> {
    let p = etat.projet(projet).await?;
    let analyses = etat.analyses_du_projet(&p).await?;
    if analyses.is_empty() {
        bail!("aucun rush analysé");
    }
    let ps = decoupe::phrases_projet(&p, &analyses);
    let prog = decoupe::programme(&p, &analyses, &ps, false);
    let premier = prog.plans.iter().rposition(|pl| pl.sortie_t0 <= t + 1e-6).context("aucun plan à cet instant")?;
    // Le plan de l'instant et les suivants, jusqu'à trois secondes au moins (quatre plans au plus).
    let mut indices = vec![premier];
    let mut duree = prog.plans[premier].duree;
    while duree < 3.0 && indices.len() < 4 && premier + indices.len() < prog.plans.len() {
        let i = premier + indices.len();
        duree += prog.plans[i].duree;
        indices.push(i);
    }
    let dossier = etat.dossier_projet(projet);
    let tmp = dossier.join("tmp").join("qualite");
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    tokio::fs::create_dir_all(&tmp).await?;
    let sources: std::collections::HashMap<String, (u32, u32)> =
        analyses.iter().map(|(id, a)| (id.clone(), (a.largeur, a.hauteur))).collect();
    mettre_a_jour_travail(etat, &p, true).await?;
    // L'extrait se regarde dans le navigateur, en SDR : même en sortie HDR, on le calcule depuis le travail SDR.
    let passe = Passe { hdr: false, ..passe_finale(etat, &p, &analyses).await? };
    let nettes = nettete_ia(etat, &p, &prog, &indices, &sources, &tmp, passe).await?;
    let mut liste = String::new();
    for &i in &indices {
        let pl = &prog.plans[i];
        let f = tmp.join(format!("plan{i:04}.mkv"));
        let mezz = etat.dossier_travail(projet, &pl.rush).join("mezz.mkv");
        rendre_plan(pl, &mezz, nettes.get(&i).map(|x| x.as_path()), None, sources[&pl.rush], (prog.largeur, prog.hauteur), &f, passe, false).await?;
        liste.push_str(&format!("file '{}'\n", f.to_string_lossy()));
    }
    tokio::fs::write(tmp.join("liste.txt"), liste).await?;
    let sorties = dossier.join("sorties");
    tokio::fs::create_dir_all(&sorties).await?;
    let mut lus = tokio::fs::read_dir(&sorties).await?;
    while let Some(e) = lus.next_entry().await? {
        if e.file_name().to_string_lossy().starts_with("qualite-") {
            let _ = tokio::fs::remove_file(e.path()).await;
        }
    }
    let nom = format!("qualite-{}.mp4", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
    let mut args: Vec<String> = ["-f", "concat", "-safe", "0", "-i"].iter().map(|x| x.to_string()).collect();
    args.push(tmp.join("liste.txt").to_string_lossy().into());
    // Le navigateur ne lit le H.264 qu'en 8 bits : l'extrait l'est, à très haut débit (il sert à juger la netteté).
    args.extend(["-an", "-c:v", "libx264", "-preset", "veryfast", "-crf", "12", "-pix_fmt", "yuv420p",
                 "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", "-movflags", "+faststart"]
        .iter().map(|x| x.to_string()));
    args.push(sorties.join(&nom).to_string_lossy().into());
    ffmpeg_simple(args).await?;
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    Ok(json!({
        "fichier": nom, "debut": prog.plans[premier].sortie_t0, "duree": duree,
        "largeur": prog.largeur, "hauteur": prog.hauteur,
        "plans": indices.iter().map(|&i| { let pl = &prog.plans[i];
            json!({"agrandissement": (pl.agrandissement * 100.0).round() / 100.0, "nettete": nettes.contains_key(&i)}) }).collect::<Vec<_>>(),
    }))
}

/// Passe du rendu final : la cadence du fichier de travail (ou 30 i/s si on l'a choisi), en 10 bits.
async fn passe_finale(etat: &EtatPartage, p: &Projet, analyses: &[(String, std::sync::Arc<crate::modele::Analyse>)]) -> Result<Passe> {
    let mut ips: f64 = 30.0;
    if texte(&p.reglages, "cadence") != "30" {
        for (rush, _) in analyses {
            let d = ffprobe(&etat.dossier_travail(&p.id, rush).join("mezz.mkv")).await?;
            let v = d["streams"].as_array().and_then(|s| s.iter().find(|s| s["codec_type"] == "video")).cloned().unwrap_or_default();
            ips = ips.max(ratio(v["r_frame_rate"].as_str().unwrap_or("30/1")).round());
        }
    }
    // HDR : demandé, et tous les rushs en HLG (un rush SDR ou PQ ne se mêle pas à une sortie HLG).
    let hdr = texte(&p.reglages, "plage_dynamique") == "hdr" && !p.rushs.is_empty()
        && p.rushs.iter().all(|r| r.source.as_ref().and_then(|s| s.hdr.as_deref()) == Some("HLG"));
    if hdr && TABLE_HLG.get().is_none() {
        let f = etat.cfg.donnees.join("luts").join("sdr-vers-hlg.cube");
        if !f.exists() {
            tokio::fs::create_dir_all(f.parent().unwrap()).await?;
            crate::fichiers::ecrire(&f, crate::couleur::table_sdr_vers_hlg().as_bytes(), false)?;
        }
        let _ = TABLE_HLG.set(f.to_string_lossy().into_owned());
    }
    Ok(Passe { ips: ips.min(60.0), dix_bits: true, hdr })
}

/// Fichier de travail absent (libéré dans « Fichiers du projet ») ou, pour un rendu final (`dix_bits`), d'avant la
/// chaîne 10 bits (8 bits, 30 i/s) : refait depuis le rush. Les temps ne changent pas : analyse, proxy et voix
/// traitées restent valables.
pub async fn mettre_a_jour_travail(etat: &EtatPartage, p: &Projet, dix_bits: bool) -> Result<()> {
    let veut_hdr = dix_bits && texte(&p.reglages, "plage_dynamique") == "hdr";
    for r in &p.rushs {
        // Sortie HDR : un second fichier de travail, en HLG, à côté du premier (l'aperçu reste en SDR).
        let hlg = etat.dossier_travail(&p.id, &r.id).join("mezz_hlg.mkv");
        let source = etat.dossier_projet(&p.id).join("rushs").join(&r.fichier);
        if veut_hdr && !hlg.exists() && source.exists() && r.source.as_ref().and_then(|s| s.hdr.as_deref()) == Some("HLG") {
            let infos = sonder_rush(&source).await?;
            let provisoire = hlg.with_file_name("mezz_hlg.en-cours.mkv");
            preparer_en(etat, &p.id, &source, &infos, &provisoire, true).await?;
            tokio::fs::rename(&provisoire, &hlg).await?;
        }
        let mezz = etat.dossier_travail(&p.id, &r.id).join("mezz.mkv");
        if let Ok(d) = ffprobe(&mezz).await {
            let v = d["streams"].as_array().and_then(|s| s.iter().find(|s| s["codec_type"] == "video")).cloned().unwrap_or_default();
            if !dix_bits || v["pix_fmt"].as_str() == Some("yuv420p10le") {
                continue;
            }
        }
        let source = etat.dossier_projet(&p.id).join("rushs").join(&r.fichier);
        if !source.exists() {
            continue;
        }
        let infos = sonder_rush(&source).await?;
        let provisoire = mezz.with_file_name("mezz.en-cours.mkv");
        preparer(etat, &p.id, &source, &infos, &provisoire).await?;
        tokio::fs::rename(&provisoire, &mezz).await?;
    }
    Ok(())
}

/// Aperçu (demi-définition, sans netteté IA) ou rendu final.
pub async fn rendre(etat: &EtatPartage, projet: &str, apercu: bool) -> Result<Sortie> {
    let debut = Instant::now();
    let p = etat.projet(projet).await?;
    let analyses = etat.analyses_du_projet(&p).await?;
    if analyses.is_empty() {
        bail!("aucun rush analysé");
    }
    mettre_a_jour_travail(etat, &p, !apercu).await?;
    let passe = if apercu { Passe::APERCU } else { passe_finale(etat, &p, &analyses).await? };
    let ps = decoupe::phrases_projet(&p, &analyses);
    let prog = decoupe::programme(&p, &analyses, &ps, apercu);
    if prog.plans.is_empty() {
        bail!("le montage ne garde aucune phrase");
    }
    let dossier = etat.dossier_projet(projet);
    let tmp = dossier.join("tmp").join(if apercu { "apercu" } else { "rendu" });
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    tokio::fs::create_dir_all(&tmp).await?;
    let sources: std::collections::HashMap<String, (u32, u32)> =
        analyses.iter().map(|(id, a)| (id.clone(), (a.largeur, a.hauteur))).collect();

    // Netteté IA, en une passe pour tous les plans concernés.
    let tous: Vec<usize> = (0..prog.plans.len()).collect();
    let avant_nettete = Instant::now();
    let nettes = nettete_ia(etat, &p, &prog, &tous, &sources, &tmp, passe).await?;
    if !nettes.is_empty() {
        tracing::info!("netteté IA de {projet} : {} plans, {} s", nettes.len(), avant_nettete.elapsed().as_secs());
    }

    // Encodage matériel seulement si la carte nous fait une petite place ; sinon x264.
    let bail_nvenc = if etat.nvenc { etat.carte.essayer("other:encodage", carte::VRAM_ENCODAGE_MO, 1800).await } else { None };
    let nvenc = bail_nvenc.is_some();
    let resultat = rendre_plans_et_assembler(etat, &p, &prog, &analyses, &tmp, &dossier, &nettes, passe, nvenc).await;
    if let Some(b) = bail_nvenc {
        etat.carte.rendre(b).await;
    }
    let nom = resultat?;
    // L'ancienne sortie du même genre est remplacée.
    let sorties = dossier.join("sorties");
    let ancienne = if apercu { p.apercu.as_ref() } else { p.rendu.as_ref() }.map(|s| s.fichier.clone());
    if let Some(a) = ancienne {
        let (fond, generique) = pistes_apercu(&a);
        let niveaux = format!("{}.niveaux.json", a.trim_end_matches(".mp4"));
        for f in [a, fond, generique, niveaux] {
            let _ = tokio::fs::remove_file(sorties.join(f)).await;
        }
    }
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    let duree_source: f64 = analyses.iter().map(|(_, a)| a.duree).sum();
    let mut rapport = decoupe::rapport(&prog, duree_source);
    rapport["largeur"] = json!(prog.largeur);
    rapport["hauteur"] = json!(prog.hauteur);
    rapport["duree_rendu_s"] = json!(debut.elapsed().as_secs());
    rapport["encodeur"] = json!(if nvenc { "NVENC" } else { "processeur" });
    rapport["codec"] = json!(if apercu || texte(&p.reglages, "codec_final") == "h264" { "h264" } else { "hevc" });
    rapport["ips"] = json!(passe.ips);
    rapport["bits"] = json!(if passe.dix_bits { 10 } else { 8 });
    rapport["hdr"] = json!(passe.hdr);
    rapport["son"] = json!(resume_son(&p.reglages));
    // Ce que l'aperçu a dans son son (le reste se joue en direct) : l'interface prévient s'il ne suit plus les réglages.
    rapport["son_inscrit"] = json!({"debruitage": texte(&p.reglages, "debruitage"), "dereverberation": texte(&p.reglages, "dereverberation"),
                                    "restauration": vrai(&p.reglages, "restauration"),
                                    "calage_son": nombre(&p.reglages, "calage_son"),
                                    "voix_ia": if vrai(&p.reglages, "restauration") { bilans_voix(etat, &p) } else { json!([]) }});
    // Aperçu sans PostProd : l'interface l'applique en direct (un ancien aperçu l'a déjà dans son image).
    rapport["postprod_direct"] = json!(apercu);
    rapport["habillage_direct"] = json!(apercu);
    // Sous-titres, titres de parties, textes et images posés : dessinés en direct sur cet aperçu.
    rapport["textes_direct"] = json!(apercu);
    if apercu {
        // Son en direct : la vidéo porte la voix de base ; le fond et le générique, s'il y en a, sont à côté.
        let (fond, generique) = pistes_apercu(&nom);
        let existe = |f: &str| sorties.join(f).exists().then(|| f.to_string());
        rapport["son_direct"] = json!({"fond": existe(&fond), "generique": existe(&generique)});
    }
    rapport["empreinte"] = json!(empreinte(&p));
    let (avant, apres) = durees_generique(&p);
    rapport["generique"] = json!({"debut": avant, "fin": apres});
    rapport["plans_detail"] = decoupe::plans_json(&prog);
    // Contrôle : le son et l'image du fichier final ont la même durée ; sinon la voix dérive au fil des plans.
    if let Ok(d) = ffprobe(&sorties.join(&nom)).await {
        let duree = |genre: &str| {
            d["streams"].as_array().into_iter().flatten().find(|s| s["codec_type"] == genre)
                .and_then(|s| s["duration"].as_str()).and_then(|s| s.parse::<f64>().ok())
        };
        if let (Some(v), Some(a)) = (duree("video"), duree("audio")) {
            let ecart = ((a - v) * 1000.0).round();
            rapport["ecart_son_image_ms"] = json!(ecart);
            if ecart.abs() > 40.0 {
                rapport["avertissements"].as_array_mut().map(|l| l.push(json!(format!(
                    "le son et l'image n'ont pas la même durée ({ecart} ms) : la voix peut être décalée"))));
            }
        }
    }
    Ok(Sortie { fichier: nom, cree: maintenant(), rapport })
}

#[allow(clippy::too_many_arguments)]
async fn rendre_plans_et_assembler(etat: &EtatPartage, p: &Projet, prog: &Programme,
                                   analyses: &[(String, std::sync::Arc<crate::modele::Analyse>)], tmp: &Path,
                                   dossier: &Path, nettes: &std::collections::HashMap<usize, PathBuf>, passe: Passe,
                                   nvenc: bool) -> Result<String> {
    let projet = p.id.as_str();
    let apercu = !passe.dix_bits;
    let sources: std::collections::HashMap<String, (u32, u32)> =
        analyses.iter().map(|(id, a)| (id.clone(), (a.largeur, a.hauteur))).collect();
    let mezz = |rush: &str| etat.dossier_travail(projet, rush).join("mezz.mkv");
    // Durée de chaque étape, au journal : un rendu final 4K à 60 i/s a pris 2 h 25 le 2026-10-27 sans qu'on sache où.
    let chrono = Instant::now();
    let mut voix: std::collections::HashMap<String, PathBuf> = Default::default();
    for (rush, a) in analyses {
        if vrai(&p.reglages, "restauration") {
            voix.insert(rush.clone(), voix_reconstruite(etat, projet, rush, a.duree).await?);
        } else if let Some(f) = voix_sans_echo(etat, projet, rush, &texte(&p.reglages, "dereverberation"), a.duree).await? {
            voix.insert(rush.clone(), f);
        }
    }
    let duree_voix = chrono.elapsed().as_secs();
    // Plans.
    let sortie_dims = (prog.largeur, prog.hauteur);
    let total = prog.plans.len();
    let fichiers: Vec<PathBuf> = (0..total).map(|i| tmp.join(format!("plan{i:04}.mkv"))).collect();
    let mut faits = 0usize;
    let taches: Vec<_> = prog
        .plans
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, pl)| {
            let m = mezz(&pl.rush);
            let nette = nettes.get(&i).cloned();
            let f = fichiers[i].clone();
            let src = sources[&pl.rush];
            let v = voix.get(&pl.rush).cloned();
            async move { rendre_plan(&pl, &m, nette.as_deref(), v.as_deref(), src, sortie_dims, &f, passe, nvenc).await }
        })
        .collect();
    let mut flux = stream::iter(taches).buffer_unordered(PLANS_EN_PARALLELE);
    while let Some(r) = flux.next().await {
        r?;
        faits += 1;
        etat.signaler(projet, "Rendu des plans", 100.0 * faits as f64 / total as f64);
    }
    drop(flux);

    let duree_plans = chrono.elapsed().as_secs() - duree_voix;
    let liste = tmp.join("liste.txt");
    tokio::fs::write(&liste, fichiers.iter().map(|f| format!("file '{}'\n", f.to_string_lossy())).collect::<String>()).await?;
    let ass = tmp.join("sous_titres.ass");
    tokio::fs::write(&ass, crate::couleur::couleurs_ass(&sous_titres(prog, p, apercu), passe.hdr)).await?;
    let sorties = dossier.join("sorties");
    tokio::fs::create_dir_all(&sorties).await?;
    let nom = format!("{}-{}.mp4", if apercu { "apercu" } else { "rendu" }, chrono::Utc::now().format("%Y%m%d-%H%M%S"));
    assembler(etat, p, prog, &liste, &ass, &sorties.join(&nom), passe, nvenc).await?;
    tracing::info!("{} de {projet} : voix {duree_voix} s, plans {duree_plans} s, assemblage {} s ({} i/s, {} bits)",
                   if apercu { "aperçu" } else { "rendu final" }, chrono.elapsed().as_secs() - duree_voix - duree_plans,
                   passe.ips, if passe.dix_bits { 10 } else { 8 });
    Ok(nom)
}

// ---------------------------------------------------------------- PostProd : image avant / après

/// Une image de la source à cet instant, sans ou avec la PostProd (JPEG), pour comparer sans rendre.
/// Table de couleurs (Hald CLUT niveau 8, image 512 × 512, cube de 64³) de la partie couleur d'une PostProd,
/// calculée par ffmpeg lui-même : l'aperçu en direct reproduit exactement ses couleurs. Gardée par réglage.
pub async fn table_couleurs(etat: &EtatPartage, pp: &crate::postprod::PostProd) -> Result<Vec<u8>> {
    let filtres = pp.filtres_couleur();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    filtres.join(",").hash(&mut h);
    let f = etat.cfg.donnees.join("luts").join(format!("{:016x}.png", h.finish()));
    if let Ok(b) = tokio::fs::read(&f).await {
        return Ok(b);
    }
    tokio::fs::create_dir_all(f.parent().unwrap()).await?;
    // Comme dans le rendu, les filtres reçoivent une image YUV.
    let chaine = if filtres.is_empty() { "null".to_string() } else { filtres.join(",") };
    ffmpeg_simple(vec!["-f".into(), "lavfi".into(), "-i".into(), "haldclutsrc=level=8".into(), "-frames:v".into(), "1".into(),
                       "-vf".into(), format!("format=yuv444p,{chaine},format=rgb24"), f.to_string_lossy().into()])
        .await?;
    Ok(tokio::fs::read(&f).await?)
}

/// Partie couleur de la PostProd en table 3D (.cube, 64 niveaux, valeurs 16 bits), pour le rendu final : la même
/// table que l'aperçu en direct, appliquée sans repasser en 8 bits. Aucune : la couleur est neutre.
async fn table_couleurs_cube(etat: &EtatPartage, pp: &crate::postprod::PostProd) -> Result<Option<PathBuf>> {
    let filtres = pp.filtres_couleur();
    if filtres.is_empty() {
        return Ok(None);
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    filtres.join(",").hash(&mut h);
    let f = etat.cfg.donnees.join("luts").join(format!("{:016x}.cube", h.finish()));
    if f.exists() {
        return Ok(Some(f));
    }
    tokio::fs::create_dir_all(f.parent().unwrap()).await?;
    let sortie = Command::new("ffmpeg")
        .args(["-hide_banner", "-v", "error", "-f", "lavfi", "-i", "haldclutsrc=level=8", "-frames:v", "1", "-vf",
               &format!("format=yuv444p,{},format=rgb48le", filtres.join(",")), "-f", "rawvideo", "-"])
        .stdin(Stdio::null())
        .output()
        .await
        .context("ffmpeg introuvable")?;
    if !sortie.status.success() {
        bail!("table de couleurs : {}", String::from_utf8_lossy(&sortie.stderr).trim());
    }
    // Image Hald de niveau 8 : 64³ pixels, rouge le plus rapide, puis vert, puis bleu (l'ordre du format .cube).
    let n = 64usize;
    if sortie.stdout.len() != n * n * n * 6 {
        bail!("table de couleurs : taille inattendue ({} octets)", sortie.stdout.len());
    }
    let mut texte = format!("LUT_3D_SIZE {n}\n");
    for px in sortie.stdout.chunks_exact(6) {
        let c = |i: usize| u16::from_le_bytes([px[i], px[i + 1]]) as f64 / 65535.0;
        texte.push_str(&format!("{:.6} {:.6} {:.6}\n", c(0), c(2), c(4)));
    }
    let provisoire = f.with_extension("en-cours.cube");
    tokio::fs::write(&provisoire, texte).await?;
    tokio::fs::rename(&provisoire, &f).await?;
    Ok(Some(f))
}

pub async fn image_postprod(mezz: &Path, t: f64, pp: &crate::postprod::PostProd, avec: bool) -> Result<Vec<u8>> {
    let d = ffprobe(mezz).await?;
    let v = d["streams"].as_array().and_then(|s| s.iter().find(|s| s["codec_type"] == "video")).cloned().unwrap_or_default();
    let (l, h) = (v["width"].as_u64().unwrap_or(1080) as f64, v["height"].as_u64().unwrap_or(1920) as f64);
    // Petit côté ramené à 720 : assez pour juger la peau et le grain.
    let k = 720.0 / l.min(h);
    let (lw, hh) = (((l * k) as u32) / 2 * 2, ((h * k) as u32) / 2 * 2);
    let chaine = if avec { pp.filtre("i", "o", lw, hh) } else { "[i]null[o]".to_string() };
    let sortie = Command::new("ffmpeg")
        .args(["-hide_banner", "-v", "error", "-ss", &format!("{t:.3}"), "-i"])
        .arg(mezz)
        .args(["-frames:v", "1", "-filter_complex", &format!("[0:v]scale={lw}:{hh}:flags=lanczos,format=yuv420p[i];{chaine}"),
               "-map", "[o]", "-f", "image2pipe", "-c:v", "mjpeg", "-q:v", "3", "-"])
        .output()
        .await?;
    if !sortie.status.success() || sortie.stdout.is_empty() {
        bail!("image impossible : {}", String::from_utf8_lossy(&sortie.stderr).trim());
    }
    Ok(sortie.stdout)
}

// ---------------------------------------------------------------- export

/// Préréglages d'export : (clé, libellé).
pub const PRESETS: &[(&str, &str)] = &[("youtube", "YouTube"), ("leger", "Léger"), ("max", "Qualité maximale")];

/// Version exportée du rendu final. YouTube et archive : le rendu final lui-même (lien, sans réencodage) — il
/// dépasse déjà ce que YouTube recommande, et YouTube réencode tout envoi : chaque compression de plus ôtait de la
/// précision. Léger (iPhone, messagerie) : réencodé en 720p.
pub async fn exporter(etat: &EtatPartage, projet: &str, preset: &str) -> Result<crate::modele::Export> {
    let p = etat.projet(projet).await?;
    let rendu = p.rendu.as_ref().ok_or_else(|| anyhow!("lancez d'abord le rendu final"))?;
    let source = etat.dossier_projet(projet).join("sorties").join(&rendu.fichier);
    let (l, h) = (rendu.rapport["largeur"].as_u64().unwrap_or(1080) as u32, rendu.rapport["hauteur"].as_u64().unwrap_or(1920) as u32);
    if preset != "leger" {
        let dossier = etat.dossier_projet(projet).join("sorties");
        let nom = format!("export-{preset}-{}.mp4", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
        let cible = dossier.join(&nom);
        if tokio::fs::hard_link(&source, &cible).await.is_err() {
            tokio::fs::copy(&source, &cible).await?;
        }
        let taille = tokio::fs::metadata(&cible).await.map(|m| m.len()).unwrap_or(0);
        return Ok(crate::modele::Export { id: crate::modele::nouvel_id(), preset: preset.into(), fichier: nom, cree: maintenant(),
                                          taille, largeur: l, hauteur: h, youtube: None });
    }
    let duree = rendu.rapport["duree_montage"].as_f64().unwrap_or(0.0);
    let petit = l.min(h);
    // Le léger descend à 720 au petit côté ; les autres gardent la définition du rendu.
    let cible = if preset == "leger" { petit.min(720) } else { petit };
    let (lo, ho) = if l <= h { (cible, (h as f64 * cible as f64 / l as f64) as u32 / 2 * 2) } else { ((l as f64 * cible as f64 / h as f64) as u32 / 2 * 2, cible) };
    let quatre_k = petit >= 2160;
    let video: Vec<String> = match preset {
        "youtube" => {
            let (debit, max) = if quatre_k { ("45M", "60M") } else { ("12M", "16M") };
            vec!["-c:v", "libx264", "-preset", "medium", "-profile:v", "high", "-level:v", if quatre_k { "5.1" } else { "4.2" },
                 "-b:v", debit, "-maxrate", max, "-bufsize", max, "-g", "15", "-bf", "2"].into_iter().map(String::from).collect()
        }
        "leger" => vec!["-c:v", "libx264", "-preset", "medium", "-profile:v", "high", "-level:v", "4.0",
                        "-crf", "26", "-maxrate", "3M", "-bufsize", "6M"].into_iter().map(String::from).collect(),
        _ => vec!["-c:v", "libx264", "-preset", "slow", "-profile:v", "high", "-crf", "15"].into_iter().map(String::from).collect(),
    };
    let audio = match preset { "youtube" => "384k", "leger" => "128k", _ => "320k" };
    let dossier = etat.dossier_projet(projet).join("sorties");
    let nom = format!("export-{preset}-{}.mp4", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
    let mut args: Vec<String> = vec!["-i".into(), source.to_string_lossy().into()];
    let echelle = if (lo, ho) != (l, h) { format!("scale={lo}:{ho}:flags=lanczos,") } else { String::new() };
    // Rendu HDR : la version légère (messageries, ancien téléphone) repasse en SDR, comme le fichier de travail.
    let sdr = if rendu.rapport["hdr"].as_bool() == Some(true) {
        "zscale=tin=arib-std-b67:min=bt2020nc:pin=bt2020:rin=tv:t=linear:npl=203,format=gbrpf32le,zscale=p=bt709,\
         tonemap=tonemap=mobius:desat=0,zscale=t=bt709:m=bt709:r=tv,format=yuv420p,"
    } else {
        ""
    };
    args.extend(["-vf".into(), format!("{sdr}{echelle}{BT709}")]);
    args.extend(video);
    args.extend([
        "-pix_fmt", "yuv420p", "-r", "30", "-fps_mode", "cfr", "-tag:v", "avc1",
        "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709",
        "-c:a", "aac", "-b:a", audio, "-ar", "48000", "-ac", "2", "-movflags", "+faststart",
    ].map(String::from));
    args.push(dossier.join(&nom).to_string_lossy().into());
    let libelle = PRESETS.iter().find(|(k, _)| *k == preset).map(|(_, l)| *l).unwrap_or(preset);
    ffmpeg(etat, projet, &format!("Export {libelle}"), &args, duree).await?;
    let taille = tokio::fs::metadata(dossier.join(&nom)).await.map(|m| m.len()).unwrap_or(0);
    Ok(crate::modele::Export { id: crate::modele::nouvel_id(), preset: preset.into(), fichier: nom, cree: maintenant(), taille,
                               largeur: lo, hauteur: ho, youtube: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Écrit le graphe de chaque ambiance de la bibliothèque : `EDIVID_GRAPHES=<dossier> cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn graphes_des_ambiances() {
        let dossier = PathBuf::from(std::env::var("EDIVID_GRAPHES").expect("EDIVID_GRAPHES"));
        let tout: Value = serde_json::from_str(include_str!("../bibliotheque/ambiances.json")).unwrap();
        for a in tout["ambiances"].as_array().unwrap() {
            let mut r = crate::reglages::defauts();
            for (k, v) in a["reglages"].as_object().unwrap() {
                assert!(crate::reglages::valider(k, v).is_ok(), "{} : {k} = {v} refusé", a["cle"]);
                r.insert(k.clone(), v.clone());
            }
            // Une ambiance ne contredit pas les règles du son : appliquée, elle reste telle quelle.
            assert!(crate::reglages::accorder_son(&mut r.clone()).is_empty(), "{} : réglages du son contradictoires", a["cle"]);
            let ir = lieu(&texte(&r, "espace")).map(|(_, _, _, part)| (dossier.join(format!("lieu_{}.wav", texte(&r, "espace"))), part));
            let mut fc = graphe_voix(&r, 20.0, "0:a", "voix", ir.as_ref(), true, &[]);
            fc.push("[voix]loudnorm=I=-14:TP=-1.5:LRA=11,aresample=48000[a]".into());
            std::fs::write(dossier.join(format!("{}.graphe", a["cle"].as_str().unwrap())), fc.join(";")).unwrap();
            std::fs::write(dossier.join(format!("{}.direct", a["cle"].as_str().unwrap())), filtres_direct(&r, &[]).join("\n")).unwrap();
        }
    }
}

#[cfg(test)]
mod tests_logo {
    use super::*;

    /// Écrit le graphe du logo (ombre et halo) : `EDIVID_GRAPHES=<dossier> cargo test logo -- --ignored`.
    #[test]
    #[ignore]
    fn graphe_du_logo() {
        let dossier = PathBuf::from(std::env::var("EDIVID_GRAPHES").expect("EDIVID_GRAPHES"));
        for (nom, halo, couleur) in [("faible", 0.01, "blanc"), ("moyen", 0.3, "blanc"), ("noir", 0.3, "noir")] {
            let hab = crate::modele::Habillage { genre: "logo".into(), position: "haut_gauche".into(), taille: 0.16, opacite: 0.9,
                                                 ombre: 0.7, halo, halo_couleur: couleur.into(), ..Default::default() };
            std::fs::write(dossier.join(format!("logo_{nom}.graphe")), graphe_logo("1:v", "0:v", "v", 0, 154, &hab, 38, Passe::APERCU).join(";")).unwrap();
        }
    }
}

#[cfg(test)]
mod tests_qualite {
    use super::*;

    /// Plan en qualité finale sur un projet réel (carte graphique sous bail) :
    /// `EDIVID_PROJET=<id> EDIVID_T=<s> cargo test qualite -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn plan_en_qualite_finale() {
        let cfg = crate::config::Config::charger().unwrap();
        let etat = crate::etat::Etat::nouveau(cfg, false).await.unwrap();
        let t: f64 = std::env::var("EDIVID_T").unwrap().parse().unwrap();
        let r = plan_qualite(&etat, &std::env::var("EDIVID_PROJET").unwrap(), t).await.unwrap();
        println!("{r}");
    }
}

#[cfg(test)]
mod tests_capteur {
    use super::*;

    /// Un capteur sous bail de la carte : `EDIVID_SCRIPT=nettete.py EDIVID_ARGS=<arg1>|<arg2> cargo test capteur -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn capteur_sous_bail() {
        let cfg = crate::config::Config::charger().unwrap();
        let etat = crate::etat::Etat::nouveau(cfg, false).await.unwrap();
        let bail = etat.carte.prendre(&etat, "essai", "other:essai", carte::VRAM_RESTAURATION_MO, 600).await.unwrap();
        // Lancé à la main (même environnement que `capteur`) pour voir toute sa sortie, mémoire de la carte comprise.
        let script = std::env::var("EDIVID_SCRIPT").unwrap();
        let python = match script.as_str() {
            "restaure.py" => &etat.cfg.python_voicefixer,
            "rehausse.py" => &etat.cfg.python_rehaussement,
            "separe.py" | "qualite_son.py" | "egaliseur.py" => &etat.cfg.python_son,
            "seche_ia.py" => &etat.cfg.python_clearvoice,
            "debruite.py" => &etat.cfg.python_deepfilter,
            _ => &etat.cfg.python,
        };
        let r = Command::new(python)
            .arg(etat.cfg.racine.join("capteurs").join(&script))
            .args(std::env::var("EDIVID_ARGS").unwrap().split('|'))
            .env("HF_HUB_OFFLINE", "1")
            .env("PYTORCH_CUDA_ALLOC_CONF", "expandable_segments:True")
            .output()
            .await;
        etat.carte.rendre(bail).await;
        let r = r.unwrap();
        println!("{}", String::from_utf8_lossy(&r.stdout).lines().filter(|l| !l.contains("\"pct\"")).collect::<Vec<_>>().join("\n"));
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    }
}

#[cfg(test)]
mod tests_rendu_final {
    use super::*;

    /// Rendu final complet, sur une copie des données (EDIVID_DONNEES) : il remplace l'ancien rendu de la copie.
    /// `EDIVID_DONNEES=<copie> EDIVID_PROJET=<id> [EDIVID_APERCU=1] cargo test rendu_final_complet -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn rendu_final_complet() {
        let cfg = crate::config::Config::charger().unwrap();
        let etat = crate::etat::Etat::nouveau(cfg, crate::rendu::nvenc_disponible().await).await.unwrap();
        let apercu = std::env::var("EDIVID_APERCU").is_ok();
        let s = rendre(&etat, &std::env::var("EDIVID_PROJET").unwrap(), apercu).await.unwrap();
        println!("{}", serde_json::to_string_pretty(&json!({"fichier": s.fichier, "rapport": s.rapport})).unwrap());
    }
}
