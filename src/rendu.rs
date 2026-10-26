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
/// 48 000 échantillons par seconde, 30 images par seconde.
const ECHANTILLONS_PAR_IMAGE: u64 = 1_600;
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
        bail!("ffmpeg en échec ({etape}) : {}", erreurs.lines().rev().take(4).collect::<Vec<_>>().join(" | "));
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
        (true, "plan_apercu") => &["-c:v", "h264_nvenc", "-preset", "p2", "-rc", "vbr", "-cq", "26", "-b:v", "0"],
        (true, "plan") => &["-c:v", "h264_nvenc", "-preset", "p5", "-rc", "vbr", "-cq", "13", "-b:v", "0"],
        (true, "assemblage_apercu") => &["-c:v", "h264_nvenc", "-preset", "p4", "-rc", "vbr", "-cq", "24", "-b:v", "0"],
        (true, _) => &["-c:v", "h264_nvenc", "-preset", "p7", "-tune", "hq", "-rc", "vbr", "-cq", "17", "-b:v", "0",
                       "-spatial-aq", "1", "-temporal-aq", "1", "-profile:v", "high"],
        (false, "plan_apercu") => &["-c:v", "libx264", "-preset", "ultrafast", "-crf", "23"],
        (false, "plan") => &["-c:v", "libx264", "-preset", "veryfast", "-crf", "12"],
        (false, "assemblage_apercu") => &["-c:v", "libx264", "-preset", "veryfast", "-crf", "23"],
        (false, _) => &["-c:v", "libx264", "-preset", "slow", "-crf", "17"],
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

/// Fichier de travail : rotation appliquée, HDR converti en SDR Rec.709, 30 i/s fixes, son PCM.
pub async fn preparer(etat: &EtatPartage, projet: &str, source: &Path, infos: &InfosSource, mezz: &Path) -> Result<()> {
    let d = ffprobe(source).await?;
    let a_son = d["streams"].as_array().into_iter().flatten().any(|s| s["codec_type"] == "audio");
    let mut vf = Vec::new();
    match infos.hdr.as_deref() {
        Some(hdr) => {
            let (tin, npl) = if hdr == "HLG" { ("arib-std-b67", 203) } else { ("smpte2084", 100) };
            vf.push(format!(
                "zscale=tin={tin}:min=bt2020nc:pin=bt2020:rin=tv:t=linear:npl={npl},format=gbrpf32le,zscale=p=bt709,\
                 tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv"
            ));
        }
        None => {}
    }
    vf.push(format!("fps={IPS}"));
    vf.push("format=yuv420p".into());
    vf.push(BT709.into());
    let mut args: Vec<String> = vec!["-i".into(), source.to_string_lossy().into()];
    if !a_son {
        args.extend(["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"].map(String::from));
    }
    args.extend(["-map".into(), "0:v:0".into(), "-map".into(), if a_son { "0:a:0".into() } else { "1:a".into() }]);
    args.extend([
        "-vf".into(), vf.join(","),
        "-c:v".into(), "libx264".into(), "-preset".into(), "veryfast".into(), "-crf".into(), "16".into(),
        "-g".into(), "30".into(), "-pix_fmt".into(), "yuv420p".into(),
        "-colorspace".into(), "bt709".into(), "-color_primaries".into(), "bt709".into(), "-color_trc".into(), "bt709".into(),
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
        "-vf".into(), "scale='if(gt(iw,ih),-2,540)':'if(gt(iw,ih),540,-2)'".into(),
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
    let mut enfant = Command::new(&etat.cfg.python)
        .arg(etat.cfg.racine.join("capteurs").join(script))
        .args(args)
        .env("HF_HUB_OFFLINE", "1")
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
            "echo" => "Retrait de l'écho de la voix",
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
        bail!("{script} en échec : {}", utile.iter().rev().take(3).rev().cloned().collect::<Vec<_>>().join(" | "));
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
fn filtre_plan(p: &Plan, largeur_entree: u32, sortie: (u32, u32), nette: bool) -> String {
    let (wo, ho) = sortie;
    let n = (p.duree * IPS).round().max(1.0);
    let z_trans = if p.transition.as_deref() == Some("zoom") { ZOOM_TRANSITION } else { 1.0 };
    let mut f: Vec<String> = Vec::new();
    if p.z_fin <= 1.0 + 1e-6 && z_trans == 1.0 {
        f.push(format!("scale={wo}:{ho}:flags=lanczos"));
    } else {
        let pre = ((2.0 * wo as f64) / largeur_entree as f64).ceil().clamp(1.0, 4.0) as u32;
        if pre > 1 {
            f.push(format!("scale=iw*{pre}:ih*{pre}:flags=lanczos"));
        }
        let zm = p.z_fin * z_trans;
        let progres = if zm > 1.0 + 1e-6 { format!("min(1,max(0,(zoom-1)/{:.5}))", zm - 1.0) } else { "0".into() };
        let (cx, cy) = p.centre_serre;
        let nt = 0.25 * IPS;
        // Une poussée répartie sur plusieurs plans avance régulièrement ; seule, elle ralentit en fin de plan.
        let avance = if p.lineaire { format!("min(on/{n},1)") } else { format!("(1-pow(1-min(on/{n},1),2))") };
        f.push(format!(
            "zoompan=z='(1+{:.5}*{avance})*(1+{:.3}*pow(max(0,1-on/{nt}),2))':\
             x='max(0,min(iw-iw/zoom,(0.5+({cx:.5}-0.5)*{progres})*iw-iw/zoom/2))':\
             y='max(0,min(ih-ih/zoom,(0.5+({cy:.5}-0.5)*{progres})*ih-ih/zoom/2))':d=1:s={wo}x{ho}:fps={IPS}",
            p.z_fin - 1.0,
            z_trans - 1.0
        ));
    }
    if !nette && p.agrandissement > 1.8 {
        f.push("unsharp=5:5:0.5:5:5:0".into());
    }
    let echelle = wo as f64 / 1080.0;
    let flou = |taille: f64| ((taille * echelle).round() as u32).max(2);
    match p.transition.as_deref() {
        Some("flash") => f.push("fade=t=in:st=0:d=0.15:color=white".into()),
        Some("fondu") => f.push("fade=t=in:st=0:d=0.25".into()),
        Some("glitch") => f.push("rgbashift=rh=-14:bh=14:enable='lt(t,0.2)',noise=alls=35:allf=t:enable='lt(t,0.2)'".into()),
        Some("glisse") => f.push(format!(
            "avgblur=sizeX={}:sizeY=1:enable='lt(t,0.05)',avgblur=sizeX={}:sizeY=1:enable='between(t,0.05,0.1)',\
             avgblur=sizeX={}:sizeY=1:enable='between(t,0.1,0.15)'",
            flou(60.0), flou(30.0), flou(12.0)
        )),
        _ => {}
    }
    match p.sortie.as_deref() {
        Some("fondu") => f.push(format!("fade=t=out:st={:.3}:d=0.12", (p.duree - 0.12).max(0.0))),
        Some("glisse") => f.push(format!("avgblur=sizeX={}:sizeY=1:enable='gte(t,{:.3})'", flou(40.0), (p.duree - 0.08).max(0.0))),
        _ => {}
    }
    f.push("setsar=1".into());
    f.push("format=yuv420p".into());
    f.join(",")
}

#[allow(clippy::too_many_arguments)]
async fn rendre_plan(p: &Plan, mezz: &Path, nette: Option<&Path>, voix: Option<&Path>, source: (u32, u32),
                     sortie: (u32, u32), fichier: &Path, apercu: bool, nvenc: bool) -> Result<()> {
    let zone = zone_entiere(p, source);
    let n = (p.duree * IPS).round().max(1.0) as u32;
    // Image et son de durée exacte : n images et n × 1 600 échantillons. Avec « -frames:v », ffmpeg coupait
    // le son au dernier bloc entier : jusqu'à 20 ms perdus par plan, près d'une seconde d'avance de la voix
    // sur 200 plans (mesuré le 2026-10-18).
    let echantillons = n as u64 * ECHANTILLONS_PAR_IMAGE;
    let mut args: Vec<String> = Vec::new();
    let vf = match nette {
        Some(f) => {
            args.extend(["-t".into(), format!("{:.3}", p.duree), "-i".into(), f.to_string_lossy().into()]);
            filtre_plan(p, largeur_nette(p, zone.w, sortie.0), sortie, true)
        }
        None => format!("crop={}:{}:{}:{},{}", zone.w, zone.h, zone.x, zone.y, filtre_plan(p, zone.w, sortie, false)),
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
    args.extend(encodeur(nvenc, if apercu { "plan_apercu" } else { "plan" }));
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

fn sous_titres(prog: &Programme, p: &Projet) -> String {
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
    for (t, titre) in &prog.titres {
        l.push(format!("Dialogue: 1,{},{},Titre,,0,0,0,,{{\\fad(120,200)}}{}", temps_ass(t + 0.1), temps_ass(t + 1.9),
                       titre.to_uppercase()));
    }
    textes(prog, &mut l);
    habillage_textes(p, prog, &mut l);
    if !vrai(r, "sous_titres") {
        return l.join("\n") + "\n";
    }
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
    for (i, g) in groupes.iter().enumerate() {
        let fin_naturelle = g[g.len() - 1].t1 + 0.4;
        let fin = groupes.get(i + 1).map(|s| s[0].t0.min(fin_naturelle)).unwrap_or(fin_naturelle);
        let texte_groupe = g
            .iter()
            .map(|m| {
                let mot = sans_ponctuation(&m.mot);
                if dynamique {
                    let mot = mot.to_uppercase();
                    if m.fort { format!("{{\\c&H0000E5FF&\\fscx115\\fscy115}}{mot}{{\\r}}") } else { mot }
                } else {
                    mot
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let anim = if dynamique { "{\\fscx85\\fscy85\\t(0,90,\\fscx100\\fscy100)}" } else { "" };
        l.push(format!("Dialogue: 0,{},{},Mot,,0,0,0,,{anim}{texte_groupe}", temps_ass(g[0].t0), temps_ass(fin)));
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
                      args: &mut Vec<String>, fc: &mut Vec<String>, entree: &mut usize, tmp: &Path) -> Result<()> {
    let medias = etat.dossier_projet(&p.id).join("medias");
    let media = p.medias.iter().find(|m| m.id == s.media);
    let fondu = format!("fade=t=in:st=0:d=0.35,fade=t=out:st={:.3}:d=0.35", (d - 0.35).max(0.0));
    let afondu = format!("afade=t=in:st=0:d=0.3,afade=t=out:st={:.3}:d=0.35", (d - 0.35).max(0.0));
    let finir = format!("trim=0:{d:.3},setpts=PTS-STARTPTS,fps={IPS},setsar=1,format=yuv420p,{BT709},{fondu}");
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
            "[{i}:v]fps={IPS},split[gs{k}a][gs{k}b];\
             [gs{k}b]scale={wo}:{ho}:force_original_aspect_ratio=increase,crop={wo}:{ho},gblur=sigma=30[gs{k}c];\
             [gs{k}a]scale={wo}:{ho}:force_original_aspect_ratio=decrease[gs{k}d];\
             [gs{k}c][gs{k}d]overlay=(W-w)/2:(H-h)/2,{finir}[gv{k}]"
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
            fc.push(format!("[{i}:v]scale={wo}:{ho}:force_original_aspect_ratio=increase,crop={wo}:{ho}{assombrir}[gc{k}]"));
        }
        None => {
            args.extend(["-f".into(), "lavfi".into(), "-t".into(), format!("{d:.3}"), "-i".into(), format!("color=c=0x101014:s={wo}x{ho}:r={IPS}")]);
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
fn graphe_voix(r: &crate::reglages::Reglages, gain: f64, entree: &str, sortie: &str, ir: Option<&(PathBuf, f64)>, entiere: bool) -> Vec<String> {
    let mut a = filtres_base(r, gain);
    if entiere {
        a.extend(filtres_direct(r));
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
pub const REGLAGES_SON_DIRECT: &[&str] = &["reverberation", "voix", "timbre_voix", "deesseur", "espace"];

/// Partie de la chaîne de la voix jouée en direct sur l'aperçu : porte de traîne, timbre, sifflantes. Le navigateur
/// (web/son-dsp.js) exécute ces mêmes filtres ffmpeg ; il n'en connaît que ceux-ci : highpass, lowpass,
/// equalizer, bass, treble, agate, acompressor, deesser.
pub fn filtres_direct(r: &crate::reglages::Reglages) -> Vec<String> {
    let mut a = Vec::new();
    // Écho de la pièce : une porte douce baisse la traîne entre les phrases, après avoir retiré le grave boueux.
    match texte(r, "reverberation").as_str() {
        "attenuer" => a.push("highpass=f=90,equalizer=f=300:t=q:w=1:g=-3,agate=threshold=0.015:ratio=3:range=0.2:attack=3:release=120:knee=2".into()),
        "forte" => a.push("highpass=f=100,equalizer=f=300:t=q:w=1:g=-4,equalizer=f=600:t=q:w=1.2:g=-2,\
                           agate=threshold=0.02:ratio=5:range=0.1:attack=2:release=90:knee=1.5".into()),
        _ => {}
    }
    if vrai(r, "voix") {
        a.push(match texte(r, "timbre_voix").as_str() {
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
    let filtres = filtres_direct(&p.reglages);
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

/// Son du rush sans l'écho de la pièce (WPE, capteurs/echo.py), calculé une fois par rush et par dosage.
async fn voix_sans_echo(etat: &EtatPartage, projet: &str, rush: &str, force: &str) -> Result<Option<PathBuf>> {
    let part = match force {
        "douce" => 0.6,
        "forte" => 1.0,
        _ => return Ok(None),
    };
    let dossier = etat.dossier_travail(projet, rush);
    let f = dossier.join(format!("voix_sans_echo_{force}.wav"));
    if !f.exists() {
        let provisoire = dossier.join(format!("voix_sans_echo_{force}.en-cours.wav"));
        capteur(etat, projet, "echo.py", &[dossier.join("mezz.mkv").to_string_lossy().into(),
                                           provisoire.to_string_lossy().into(), format!("{part}")]).await?;
        tokio::fs::rename(&provisoire, &f).await?;
    }
    Ok(Some(f))
}

/// Écoute d'essai : `duree` secondes d'un rush à partir de `debut`, traitées avec les réglages de son donnés, en
/// MP3. Tout se fait sur le processeur ; le résultat est gardé (même extrait, mêmes réglages : même fichier).
pub async fn essai_son(etat: &EtatPartage, projet: &str, rush: &str, debut: f64, duree: f64, r: &crate::reglages::Reglages) -> Result<Vec<u8>> {
    let cles = ["debruitage", "dereverberation", "reverberation", "voix", "timbre_voix", "deesseur", "espace"];
    let empreinte = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!("{debut:.2}-{duree:.2}-{}", cles.iter().map(|c| r.get(*c).map(|v| v.to_string()).unwrap_or_default()).collect::<Vec<_>>().join("|"))
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
    let part = match texte(r, "dereverberation").as_str() { "douce" => Some(0.6), "forte" => Some(1.0), _ => None };
    if let Some(part) = part {
        let sans = dossier.join(format!("{empreinte}-sans-echo.wav"));
        capteur(etat, projet, "echo.py", &[extrait.to_string_lossy().into(), sans.to_string_lossy().into(), format!("{part}")]).await?;
        source = sans;
    }
    let gain = sonie_rush(etat, projet, rush).await.map(|i| (-23.0 - i).clamp(-12.0, 30.0)).unwrap_or(0.0);
    let ir = reponse_espace(etat, &texte(r, "espace")).await?;
    let mut fc = graphe_voix(r, gain, "0:a", "voix", ir.as_ref(), true);
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
    // Les réglages joués en direct sur l'aperçu ne le rendent pas périmé.
    let mut reglages = p.reglages.clone();
    for c in REGLAGES_SON_DIRECT {
        reglages.remove(*c);
    }
    let v = json!([reglages, p.montage.as_ref().map(|m| json!([m.phrases, m.accroche, m.accroche_choisie, m.chapitres])),
                   p.incrustations, p.habillage, p.generique, p.corrections, p.fusions,
                   p.medias.iter().map(|m| [&m.id, &m.fichier]).collect::<Vec<_>>(), p.rushs.iter().map(|r| &r.id).collect::<Vec<_>>()]);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.to_string().hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Le son appliqué, en clair, pour le rapport : on sait avec quoi l'aperçu a été fait.
pub fn resume_son(r: &crate::reglages::Reglages) -> String {
    let choix = |cle: &str| crate::reglages::libelle_choix(cle, &texte(r, cle)).unwrap_or_else(|| texte(r, cle));
    let mut morceaux = vec![format!("bruit : {}", choix("debruitage").to_lowercase()), format!("écho : {}", choix("reverberation").to_lowercase())];
    morceaux.push(if vrai(r, "voix") { format!("voix : {}", choix("timbre_voix").to_lowercase()) } else { "voix : sans traitement".into() });
    if vrai(r, "deesseur") {
        morceaux.push("« s » adoucis".into());
    }
    morceaux.join(" · ")
}

#[allow(clippy::too_many_arguments)]
async fn assembler(etat: &EtatPartage, p: &Projet, prog: &Programme, liste: &Path, ass: &Path, sortie: &Path,
                   apercu: bool, nvenc: bool) -> Result<()> {
    let r = &p.reglages;
    let (wo, ho) = (prog.largeur, prog.hauteur);
    let total = prog.duree;
    let mut args: Vec<String> = vec!["-f".into(), "concat".into(), "-safe".into(), "0".into(), "-i".into(), liste.to_string_lossy().into()];
    let mut entree = 1usize;
    let mut fc: Vec<String> = Vec::new();

    // Image : cadence fixe, puis PostProd (couleur, teint, diffusion, netteté, grain…), puis les effets ponctuels.
    fc.push(format!("[0:v]setpts=N/({IPS}*TB)[ppe]"));
    // L'aperçu sort sans PostProd : le navigateur l'applique en direct, chaque réglage se voit aussitôt. Le rendu
    // final, lui, l'inscrit dans l'image.
    fc.push(if apercu { "[ppe]null[pps]".to_string() } else { p.postprod.filtre("ppe", "pps", wo, ho) });
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
        fc.push(format!("[{entree}:v]fps=30,{echelle},format=yuva420p{cle}{fondu},setpts=PTS-STARTPTS+{:.3}/TB[inc{k}]", inc.debut));
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
        fc.push(format!("[{base}][inc{k}]overlay=x={x}:y={y}:enable='between(t,{:.3},{:.3})':eof_action=pass[vi{k}]",
                        inc.debut, inc.fin));
        base = format!("vi{k}");
        if inc.son && m.a_son && m.genre == "video" {
            let ms = (inc.debut * 1000.0).round() as u64;
            fc.push(format!("[{entree}:a]aresample=48000,adelay=delays={ms}:all=1[si{k}]"));
            sons_incrustes.push(format!("si{k}"));
        }
        entree += 1;
    }
    // Habillage : les logos, sur toute la durée du montage (les textes passent par les sous-titres).
    for (j, hab) in p.habillage.iter().filter(|x| x.genre == "logo").enumerate() {
        let Some(m) = p.medias.iter().find(|m| m.id == hab.media) else { continue };
        let fichier = etat.dossier_projet(&p.id).join("medias").join(&m.fichier);
        if m.genre == "gif" {
            args.extend(["-ignore_loop", "0"].map(String::from));
        } else {
            args.extend(["-loop", "1", "-framerate", "30"].map(String::from));
        }
        args.extend(["-t".into(), format!("{total:.3}"), "-i".into(), fichier.to_string_lossy().into()]);
        let largeur = ((wo as f64 * hab.taille) as u32 / 2 * 2).max(16);
        let (x, y) = position_habillage(&hab.position, marge);
        fc.push(format!("[{entree}:v]fps={IPS},scale={largeur}:-2,format=yuva420p,colorchannelmixer=aa={:.2}[hab{j}]", hab.opacite));
        fc.push(format!("[{base}][hab{j}]overlay=x={x}:y={y}:eof_action=pass[vh{j}]"));
        base = format!("vh{j}");
        entree += 1;
    }
    let polices = etat.cfg.racine.join("modeles").join("polices");
    // Pixels carrés : un recadrage d'effet (secousse) laisse sinon une forme de pixel à peine faussée (4064:4059),
    // que l'enchaînement avec le générique refuse.
    fc.push(format!("[{base}]subtitles=f={}:fontsdir={},{BT709},setsar=1[v]", ass.to_string_lossy(), polices.to_string_lossy()));

    // Son.
    let gain = gain_voix(etat, p, prog).await;
    let ir = reponse_espace(etat, &texte(r, "espace")).await?;
    fc.extend(graphe_voix(r, gain, "0:a", "voix", ir.as_ref(), !apercu));
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
        return assembler_apercu(etat, p, sortie, liste, args, fc, entree, melange, total, (avant, apres), (wo, ho), ass, nvenc).await;
    }
    let mix = if melange.len() > 1 {
        fc.push(format!("{}amix=inputs={}:duration=first:normalize=0[mix]",
                        melange.iter().map(|m| format!("[{m}]")).collect::<String>(), melange.len()));
        "mix".to_string()
    } else {
        melange[0].clone()
    };
    fc.push(format!("[{mix}]loudnorm=I=-14:TP=-1.5:LRA=11,aresample=48000[a]"));

    // Générique : une séquence avant le montage, une après, au même format, enchaînées en fondu.
    let (mut v_final, mut a_final) = ("v", "a");
    if avant > 0.0 || apres > 0.0 {
        let tmp = ass.parent().unwrap_or(Path::new("."));
        let mut suite = String::new();
        let mut n = 1;
        if let (Some(s), true) = (&p.generique.debut, avant > 0.0) {
            sequence_generique(etat, p, s, avant, 0, (wo, ho), &mut args, &mut fc, &mut entree, tmp)?;
            suite.push_str("[gv0][ga0]");
            n += 1;
        }
        suite.push_str("[v][a]");
        if let (Some(s), true) = (&p.generique.fin, apres > 0.0) {
            sequence_generique(etat, p, s, apres, 1, (wo, ho), &mut args, &mut fc, &mut entree, tmp)?;
            suite.push_str("[gv1][ga1]");
            n += 1;
        }
        fc.push(format!("{suite}concat=n={n}:v=1:a=1[vg][ag]"));
        (v_final, a_final) = ("vg", "ag");
    }
    let total = avant + total + apres;

    args.extend([
        "-filter_complex".into(), fc.join(";"),
        "-map".into(), format!("[{v_final}]"), "-map".into(), format!("[{a_final}]"),
        "-t".into(), format!("{total:.3}"),
    ]);
    args.extend(encodeur(nvenc, "assemblage"));
    args.extend(sortie_video(sortie));
    lancer_assemblage(etat, p, &args, liste, total).await
}

/// Réglages de sortie de la vidéo assemblée (image et son).
fn sortie_video(sortie: &Path) -> Vec<String> {
    vec![
        "-pix_fmt".into(), "yuv420p".into(),
        "-r".into(), format!("{IPS}"), "-fps_mode".into(), "cfr".into(),
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
                          ass: &Path, nvenc: bool) -> Result<()> {
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
            sequence_generique(etat, p, s, avant, 0, (wo, ho), &mut args, &mut fc, &mut entree, tmp)?;
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
            sequence_generique(etat, p, s, apres, 1, (wo, ho), &mut args, &mut fc, &mut entree, tmp)?;
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
    args.extend(sortie_video(sortie));
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

/// Aperçu (demi-définition, sans netteté IA) ou rendu final.
pub async fn rendre(etat: &EtatPartage, projet: &str, apercu: bool) -> Result<Sortie> {
    let debut = Instant::now();
    let p = etat.projet(projet).await?;
    let analyses = etat.analyses_du_projet(&p).await?;
    if analyses.is_empty() {
        bail!("aucun rush analysé");
    }
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
    let mezz = |rush: &str| etat.dossier_travail(projet, rush).join("mezz.mkv");

    // Netteté IA, en une passe pour tous les plans concernés.
    let mut nettes: std::collections::HashMap<usize, PathBuf> = std::collections::HashMap::new();
    let a_nettoyer: Vec<(usize, &Plan)> = prog.plans.iter().enumerate().filter(|(_, p)| p.nettete).collect();
    if !a_nettoyer.is_empty() {
        let visage = texte(&p.reglages, "nettete_ia") == "visage";
        let mut travaux = Vec::new();
        for (i, pl) in &a_nettoyer {
            let z = zone_entiere(pl, sources[&pl.rush]);
            let f = tmp.join(format!("net{i:04}.mkv"));
            travaux.push(json!({"entree": mezz(&pl.rush), "debut": pl.t0, "duree": pl.duree,
                                "crop": [z.x, z.y, z.w, z.h], "sortie": f, "visage": visage, "melange": MELANGE_IA,
                                "largeur_max": largeur_nette(pl, z.w, prog.largeur)}));
            nettes.insert(*i, f);
        }
        let fichier = tmp.join("nettete.json");
        tokio::fs::write(&fichier, serde_json::to_vec(&travaux)?).await?;
        let secondes: f64 = a_nettoyer.iter().map(|(_, p)| p.duree).sum();
        let bail = etat
            .carte
            .prendre(etat, projet, "other:nettete", carte::VRAM_NETTETE_MO, (secondes * 2.0 + 120.0) as u64)
            .await?;
        let res = capteur(etat, projet, "nettete.py", &[fichier.to_string_lossy().into()]).await;
        etat.carte.rendre(bail).await;
        res?;
    }

    // Encodage matériel seulement si la carte nous fait une petite place ; sinon x264.
    let bail_nvenc = if etat.nvenc { etat.carte.essayer("other:encodage", carte::VRAM_ENCODAGE_MO, 1800).await } else { None };
    let nvenc = bail_nvenc.is_some();
    let resultat = rendre_plans_et_assembler(etat, &p, &prog, &analyses, &tmp, &dossier, &nettes, apercu, nvenc).await;
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
    rapport["encodeur"] = json!(if nvenc { "NVENC" } else { "x264" });
    rapport["son"] = json!(resume_son(&p.reglages));
    // Aperçu sans PostProd : l'interface l'applique en direct (un ancien aperçu l'a déjà dans son image).
    rapport["postprod_direct"] = json!(apercu);
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
                                   dossier: &Path, nettes: &std::collections::HashMap<usize, PathBuf>, apercu: bool,
                                   nvenc: bool) -> Result<String> {
    let projet = p.id.as_str();
    let sources: std::collections::HashMap<String, (u32, u32)> =
        analyses.iter().map(|(id, a)| (id.clone(), (a.largeur, a.hauteur))).collect();
    let mezz = |rush: &str| etat.dossier_travail(projet, rush).join("mezz.mkv");
    let mut voix: std::collections::HashMap<String, PathBuf> = Default::default();
    for (rush, _) in analyses {
        if let Some(f) = voix_sans_echo(etat, projet, rush, &texte(&p.reglages, "dereverberation")).await? {
            voix.insert(rush.clone(), f);
        }
    }
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
            async move { rendre_plan(&pl, &m, nette.as_deref(), v.as_deref(), src, sortie_dims, &f, apercu, nvenc).await }
        })
        .collect();
    let mut flux = stream::iter(taches).buffer_unordered(PLANS_EN_PARALLELE);
    while let Some(r) = flux.next().await {
        r?;
        faits += 1;
        etat.signaler(projet, "Rendu des plans", 100.0 * faits as f64 / total as f64);
    }
    drop(flux);

    let liste = tmp.join("liste.txt");
    tokio::fs::write(&liste, fichiers.iter().map(|f| format!("file '{}'\n", f.to_string_lossy())).collect::<String>()).await?;
    let ass = tmp.join("sous_titres.ass");
    tokio::fs::write(&ass, sous_titres(&prog, &p)).await?;
    let sorties = dossier.join("sorties");
    tokio::fs::create_dir_all(&sorties).await?;
    let nom = format!("{}-{}.mp4", if apercu { "apercu" } else { "rendu" }, chrono::Utc::now().format("%Y%m%d-%H%M%S"));
    assembler(etat, p, prog, &liste, &ass, &sorties.join(&nom), apercu, nvenc).await?;
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

/// Version exportée du rendu final : YouTube (débits recommandés), léger (iPhone, messagerie) ou archive.
pub async fn exporter(etat: &EtatPartage, projet: &str, preset: &str) -> Result<crate::modele::Export> {
    let p = etat.projet(projet).await?;
    let rendu = p.rendu.as_ref().ok_or_else(|| anyhow!("lancez d'abord le rendu final"))?;
    let source = etat.dossier_projet(projet).join("sorties").join(&rendu.fichier);
    let (l, h) = (rendu.rapport["largeur"].as_u64().unwrap_or(1080) as u32, rendu.rapport["hauteur"].as_u64().unwrap_or(1920) as u32);
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
    args.extend(["-vf".into(), format!("{echelle}{BT709}")]);
    args.extend(video);
    args.extend([
        "-pix_fmt", "yuv420p", "-r", "30", "-fps_mode", "cfr",
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
            let ir = lieu(&texte(&r, "espace")).map(|(_, _, _, part)| (dossier.join(format!("lieu_{}.wav", texte(&r, "espace"))), part));
            let mut fc = graphe_voix(&r, 20.0, "0:a", "voix", ir.as_ref(), true);
            fc.push("[voix]loudnorm=I=-14:TP=-1.5:LRA=11,aresample=48000[a]".into());
            std::fs::write(dossier.join(format!("{}.graphe", a["cle"].as_str().unwrap())), fc.join(";")).unwrap();
            std::fs::write(dossier.join(format!("{}.direct", a["cle"].as_str().unwrap())), filtres_direct(&r).join("\n")).unwrap();
        }
    }
}
