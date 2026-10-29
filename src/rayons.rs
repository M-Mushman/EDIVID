//! Bibliothèque d'habillage, commune à tous les projets (`<donnees>/habillage/`) : intros et fins prêtes dans
//! chaque format de sortie, jingles, styles et photos pour les vignettes. Chaque élément est un dossier
//! `<rayon>/<id>/` avec sa `fiche.json` ; les fichiers sont servis tels quels sous `/habillage/`.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

pub const RAYONS_VIDEO: [&str; 2] = ["intros", "fins"];

pub fn racine(donnees: &Path) -> PathBuf {
    donnees.join("habillage")
}

/// Un identifiant d'élément sûr dans un chemin (lettres, chiffres, tiret, souligné).
pub fn id_valide(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Un nom de photo sûr (un fichier du rayon photos, sans chemin).
pub fn photo_valide(nom: &str) -> bool {
    !nom.is_empty() && nom.len() <= 128 && !nom.starts_with('.')
        && nom.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

pub fn fiche(donnees: &Path, rayon: &str, id: &str) -> Option<Value> {
    if !id_valide(id) {
        return None;
    }
    std::fs::read_to_string(racine(donnees).join(rayon).join(id).join("fiche.json")).ok().and_then(|t| serde_json::from_str(&t).ok())
}

fn ids(dossier: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dossier).into_iter().flatten().flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| id_valide(n))
        .collect();
    // paris3 avant paris13 : les chiffres comptent comme des nombres.
    v.sort_by_key(|n| {
        let (tete, queue) = n.split_at(n.find(|c: char| c.is_ascii_digit()).unwrap_or(n.len()));
        (tete.to_string(), queue.parse::<u64>().unwrap_or(0), n.clone())
    });
    v
}

pub fn images(dossier: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dossier).into_iter().flatten().flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| [".jpg", ".jpeg", ".png", ".webp"].iter().any(|x| n.to_lowercase().ends_with(x)))
        .collect();
    v.sort();
    v
}

/// Tout le contenu de la bibliothèque, pour l'interface.
pub fn catalogue(donnees: &Path) -> Value {
    let r = racine(donnees);
    let mut c = json!({});
    for rayon in RAYONS_VIDEO.iter().chain(&["jingles"]) {
        c[rayon] = json!(ids(&r.join(rayon)).into_iter()
            .filter_map(|id| fiche(donnees, rayon, &id).map(|mut f| { f["id"] = json!(id); f }))
            .collect::<Vec<_>>());
    }
    c["styles"] = json!(ids(&r.join("styles")).into_iter()
        .map(|id| json!({"id": id, "images": images(&r.join("styles").join(&id))})).collect::<Vec<_>>());
    c["photos"] = json!(images(&r.join("photos")));
    c
}

/// Le fichier d'une intro ou d'une fin pour le format de sortie (wo × ho), et s'il est exactement à ce format.
/// Sinon : la plus petite version de même proportion qui couvre le format, ou à défaut la plus grande ; sans version
/// de même proportion, la plus grande (le rendu la cadre alors sur fond flou).
pub fn video_au_format(donnees: &Path, rayon: &str, id: &str, (wo, ho): (u32, u32)) -> Option<(PathBuf, bool)> {
    if !id_valide(id) {
        return None;
    }
    let dossier = racine(donnees).join(rayon).join(id);
    let prefixe = if rayon == "intros" { "intro_" } else { "fin_" };
    let mut versions: Vec<(u32, u32, PathBuf)> = std::fs::read_dir(&dossier).ok()?.flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            let (w, h) = n.strip_prefix(prefixe)?.strip_suffix(".mp4")?.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?, e.path()))
        })
        .collect();
    versions.sort_by_key(|(w, h, _)| w * h);
    if let Some((_, _, f)) = versions.iter().find(|(w, h, _)| (*w, *h) == (wo, ho)) {
        return Some((f.clone(), true));
    }
    let meme = |(w, h, _): &&(u32, u32, PathBuf)| *w as u64 * ho as u64 == *h as u64 * wo as u64;
    let choix = versions.iter().filter(meme).find(|(w, _, _)| *w >= wo)
        .or_else(|| versions.iter().filter(meme).last())
        .or_else(|| versions.last())?;
    Some((choix.2.clone(), false))
}

/// Le fichier d'un jingle pour une séquence de `duree` secondes : sa plus courte version qui dure au moins autant,
/// sinon la plus longue.
pub fn jingle(donnees: &Path, id: &str, duree: f64) -> Option<PathBuf> {
    let f = fiche(donnees, "jingles", id)?;
    let mut durees: Vec<(String, f64)> = f["durees"].as_object()?.iter().filter_map(|(k, v)| Some((k.clone(), v.as_f64()?))).collect();
    durees.sort_by(|a, b| a.1.total_cmp(&b.1));
    let (nom, _) = durees.iter().find(|(_, d)| *d + 0.05 >= duree).or_else(|| durees.last())?;
    Some(racine(donnees).join("jingles").join(id).join(format!("{nom}.wav")))
}
