//! ffmpeg : préparation des rushs, rendu des plans, assemblage (filtres, son, incrustations,
//! sous-titres). Les modèles (Whisper, netteté IA) passent par les capteurs Python.

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
use crate::modele::{maintenant, InfosSource, Projet, Sortie};
use crate::reglages::{nombre, texte, vrai};

const PLANS_EN_PARALLELE: usize = 6;
const ZOOM_TRANSITION: f64 = 1.4;

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
        (true, "plan") => &["-c:v", "h264_nvenc", "-preset", "p5", "-rc", "vbr", "-cq", "16", "-b:v", "0"],
        (true, "assemblage_apercu") => &["-c:v", "h264_nvenc", "-preset", "p4", "-rc", "vbr", "-cq", "24", "-b:v", "0"],
        (true, _) => &["-c:v", "h264_nvenc", "-preset", "p6", "-rc", "vbr", "-cq", "19", "-b:v", "0", "-profile:v", "high"],
        (false, "plan_apercu") => &["-c:v", "libx264", "-preset", "ultrafast", "-crf", "23"],
        (false, "plan") => &["-c:v", "libx264", "-preset", "veryfast", "-crf", "14"],
        (false, "assemblage_apercu") => &["-c:v", "libx264", "-preset", "veryfast", "-crf", "23"],
        (false, _) => &["-c:v", "libx264", "-preset", "medium", "-crf", "18"],
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
        f.push(format!(
            "zoompan=z='(1+{:.5}*(1-pow(1-min(on/{n},1),2)))*(1+{:.3}*pow(max(0,1-on/{nt}),2))':\
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
async fn rendre_plan(p: &Plan, mezz: &Path, nette: Option<&Path>, source: (u32, u32), sortie: (u32, u32),
                     fichier: &Path, apercu: bool, nvenc: bool) -> Result<()> {
    let zone = zone_entiere(p, source);
    let n = (p.duree * IPS).round().max(1.0) as u32;
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
    let (v, a) = if nette.is_some() { ("0:v", "1:a") } else { ("0:v", "0:a") };
    args.extend([
        "-map".into(), v.into(), "-map".into(), a.into(),
        "-vf".into(), vf,
        "-af".into(), format!("aresample=48000,afade=t=in:d=0.01,afade=t=out:st={:.3}:d=0.01", (p.duree - 0.01).max(0.0)),
        "-frames:v".into(), n.to_string(),
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
        String::new(),
        "[Events]".into(),
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text".into(),
    ];
    for (t, titre) in &prog.titres {
        l.push(format!("Dialogue: 1,{},{},Titre,,0,0,0,,{{\\fad(120,200)}}{}", temps_ass(t + 0.1), temps_ass(t + 1.9),
                       titre.to_uppercase()));
    }
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

// ---------------------------------------------------------------- assemblage

fn filtre_couleur(nom: &str) -> Option<&'static str> {
    Some(match nom {
        "eclatant" => "eq=contrast=1.06:saturation=1.18:gamma=0.98,unsharp=5:5:0.35:5:5:0",
        "chaud" => "colortemperature=temperature=5200:mix=0.6,eq=saturation=1.08",
        "froid" => "colortemperature=temperature=8000:mix=0.5",
        "cinema" => "eq=contrast=1.1:saturation=0.9,colorbalance=rs=-0.05:bs=0.08:rh=0.08:bh=-0.06,vignette=PI/5",
        "noir_blanc" => "hue=s=0,eq=contrast=1.15",
        "vintage" => "curves=preset=vintage,noise=alls=8:allf=t,vignette=PI/4",
        _ => return None,
    })
}

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

#[allow(clippy::too_many_arguments)]
async fn assembler(etat: &EtatPartage, p: &Projet, prog: &Programme, liste: &Path, ass: &Path, sortie: &Path,
                   apercu: bool, nvenc: bool) -> Result<()> {
    let r = &p.reglages;
    let (wo, ho) = (prog.largeur, prog.hauteur);
    let total = prog.duree;
    let mut args: Vec<String> = vec!["-f".into(), "concat".into(), "-safe".into(), "0".into(), "-i".into(), liste.to_string_lossy().into()];
    let mut entree = 1usize;
    let mut fc: Vec<String> = Vec::new();

    // Image.
    let mut v = vec![];
    if let Some(c) = filtre_couleur(&texte(r, "filtre")) {
        v.push(c.to_string());
    }
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
    fc.push(format!("[0:v]{}[v0]", if v.is_empty() { "null".to_string() } else { v.join(",") }));
    let mut base = "v0".to_string();
    let mut sons_incrustes: Vec<String> = Vec::new();
    let marge = (0.04 * wo as f64).round() as u32;
    for (k, inc) in prog.incrustations.iter().filter(|i| i.effet.is_empty()).enumerate() {
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
        let echelle = if inc.position == "plein" {
            format!("scale={wo}:{ho}:force_original_aspect_ratio=increase,crop={wo}:{ho}")
        } else {
            format!("scale={}:-2", ((wo as f64 * inc.taille) as u32 / 2 * 2).max(16))
        };
        let cle = if inc.fond_vert { ",chromakey=0x00FF00:0.13:0.08" } else { "" };
        let d = inc.fin - inc.debut;
        fc.push(format!(
            "[{entree}:v]fps=30,{echelle},format=yuva420p{cle},fade=t=in:st=0:d=0.15:alpha=1,fade=t=out:st={:.3}:d=0.15:alpha=1,\
             setpts=PTS-STARTPTS+{:.3}/TB[inc{k}]",
            (d - 0.15).max(0.0), inc.debut
        ));
        let (x, y) = match inc.position.as_str() {
            "plein" => ("0".to_string(), "0".to_string()),
            "haut_gauche" => (format!("{marge}"), format!("{marge}")),
            "haut_droite" => (format!("W-w-{marge}"), format!("{marge}")),
            "bas_gauche" => (format!("{marge}"), format!("H-h-{marge}")),
            "bas_droite" => (format!("W-w-{marge}"), format!("H-h-{marge}")),
            _ => ("(W-w)/2".to_string(), "(H-h)/2".to_string()),
        };
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
    let polices = etat.cfg.racine.join("modeles").join("polices");
    fc.push(format!("[{base}]subtitles=f={}:fontsdir={}[v]", ass.to_string_lossy(), polices.to_string_lossy()));

    // Son.
    let mut a = vec![];
    match texte(r, "debruitage").as_str() {
        "leger" => a.push("afftdn=nr=10:nf=-40:tn=1"),
        "fort" => a.push("afftdn=nr=22:nf=-35:tn=1"),
        _ => {}
    }
    if vrai(r, "voix") {
        a.push("highpass=f=80,equalizer=f=250:t=q:w=1:g=-2,equalizer=f=3500:t=q:w=1.2:g=3,acompressor=threshold=-20dB:ratio=3:attack=10:release=150:makeup=2");
    }
    if vrai(r, "deesseur") {
        a.push("deesser=i=0.4");
    }
    fc.push(format!("[0:a]{}[voix]", if a.is_empty() { "anull".to_string() } else { a.join(",") }));
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
    let mix = if melange.len() > 1 {
        fc.push(format!("{}amix=inputs={}:duration=first:normalize=0[mix]",
                        melange.iter().map(|m| format!("[{m}]")).collect::<String>(), melange.len()));
        "mix".to_string()
    } else {
        melange[0].clone()
    };
    fc.push(format!("[{mix}]loudnorm=I=-14:TP=-1.5:LRA=11,aresample=48000[a]"));

    args.extend([
        "-filter_complex".into(), fc.join(";"),
        "-map".into(), "[v]".into(), "-map".into(), "[a]".into(),
        "-t".into(), format!("{total:.3}"),
    ]);
    args.extend(encodeur(nvenc, if apercu { "assemblage_apercu" } else { "assemblage" }));
    args.extend([
        "-pix_fmt".into(), "yuv420p".into(),
        "-c:a".into(), "aac".into(), "-b:a".into(), "192k".into(),
        "-movflags".into(), "+faststart".into(),
        sortie.to_string_lossy().into(),
    ]);
    ffmpeg(etat, &p.id, "Assemblage, son et sous-titres", &args, total).await
}

/// Aperçu (demi-définition, sans netteté IA) ou rendu final.
pub async fn rendre(etat: &EtatPartage, projet: &str, apercu: bool) -> Result<Sortie> {
    let debut = Instant::now();
    let p = etat.projet(projet).await?;
    let analyses = etat.analyses_du_projet(&p).await?;
    if analyses.is_empty() {
        bail!("aucun rush analysé");
    }
    let ps = decoupe::phrases(&analyses);
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
                                "crop": [z.x, z.y, z.w, z.h], "sortie": f, "visage": visage,
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
        let _ = tokio::fs::remove_file(sorties.join(a)).await;
    }
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    let duree_source: f64 = analyses.iter().map(|(_, a)| a.duree).sum();
    let mut rapport = decoupe::rapport(&prog, duree_source);
    rapport["largeur"] = json!(prog.largeur);
    rapport["hauteur"] = json!(prog.hauteur);
    rapport["duree_rendu_s"] = json!(debut.elapsed().as_secs());
    rapport["encodeur"] = json!(if nvenc { "NVENC" } else { "x264" });
    rapport["plans_detail"] = json!(prog
        .plans
        .iter()
        .map(|pl| json!({"t": (pl.sortie_t0 * 100.0).round() / 100.0, "d": (pl.duree * 100.0).round() / 100.0,
                         "phrase": pl.phrase, "regime": pl.regime, "cadre": pl.cadre, "accroche": pl.accroche,
                         "transition": pl.transition}))
        .collect::<Vec<_>>());
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
            async move { rendre_plan(&pl, &m, nette.as_deref(), src, sortie_dims, &f, apercu, nvenc).await }
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
