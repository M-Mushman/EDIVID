//! Bibliothèque d'habillage, une par compte (`<donnees>/habillage/<compte>/`), commune à tous les projets de ce
//! compte : intros et fins prêtes dans chaque format de sortie, jingles, styles, photos et logos pour les vignettes,
//! médias versés par les projets supprimés. Chaque intro, fin ou jingle est un dossier `<rayon>/<id>/` avec sa
//! `fiche.json` ; les fichiers sont servis tels quels sous `/habillage/<compte>/`.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

pub const RAYONS_VIDEO: [&str; 2] = ["intros", "fins"];

/// Le dossier qui contient les bibliothèques de tous les comptes (servi sous `/habillage/`).
pub fn racine_commune(donnees: &Path) -> PathBuf {
    donnees.join("habillage")
}

/// Un compte réduit à un nom de dossier sûr (adresse en minuscules ; ni « / » ni « .. »).
pub fn dossier_compte(compte: &str) -> String {
    let c: String = compte.trim().to_lowercase().chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | '-') { c } else { '_' }).collect();
    let c = c.trim_matches('.').to_string();
    if c.is_empty() { "commun".into() } else { c }
}

/// La bibliothèque d'un compte.
pub fn racine(donnees: &Path, compte: &str) -> PathBuf {
    racine_commune(donnees).join(dossier_compte(compte))
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

pub fn fiche(r: &Path, rayon: &str, id: &str) -> Option<Value> {
    if !id_valide(id) {
        return None;
    }
    std::fs::read_to_string(r.join(rayon).join(id).join("fiche.json")).ok().and_then(|t| serde_json::from_str(&t).ok())
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

/// Tout le contenu de la bibliothèque d'un compte, pour l'interface ; `base` : l'adresse de ses fichiers.
pub fn catalogue(donnees: &Path, compte: &str) -> Value {
    let r = racine(donnees, compte);
    let mut c = json!({"compte": compte, "base": format!("/habillage/{}", dossier_compte(compte))});
    for rayon in RAYONS_VIDEO.iter().chain(&["jingles"]) {
        c[rayon] = json!(ids(&r.join(rayon)).into_iter()
            .filter_map(|id| fiche(&r, rayon, &id).map(|mut f| { f["id"] = json!(id); f }))
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
pub fn video_au_format(r: &Path, rayon: &str, id: &str, (wo, ho): (u32, u32)) -> Option<(PathBuf, bool)> {
    if !id_valide(id) {
        return None;
    }
    let dossier = r.join(rayon).join(id);
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

/// Recale sur la bibliothèque la durée des séquences « bibliothèque » du générique : une intro ou une fin refaite peut
/// changer de durée, et le rendu coupe la séquence à celle enregistrée dans le projet. Vrai si le projet a changé.
pub fn recaler_generique(donnees: &Path, p: &mut crate::modele::Projet) -> bool {
    let r = racine(donnees, &p.proprietaire);
    let mut change = false;
    for (s, rayon) in [(&mut p.generique.debut, RAYONS_VIDEO[0]), (&mut p.generique.fin, RAYONS_VIDEO[1])] {
        let Some(s) = s.as_mut().filter(|s| s.genre == "bibliotheque") else { continue };
        if let Some(d) = fiche(&r, rayon, &s.element).and_then(|f| f["duree"].as_f64()).map(|d| d.clamp(1.0, 60.0)) {
            if (d - s.duree).abs() > 1e-3 {
                s.duree = d;
                change = true;
            }
        }
    }
    change
}

/// Le fichier d'un jingle pour une séquence de `duree` secondes. Pour une intro ou une fin, sa version qui finit
/// franchement sur l'accord final du morceau (`intro` : la dernière mesure et l'accord ; `final` : la phrase qui y
/// mène et l'accord jusqu'à son extinction) quand il en a une, avec sa durée : le rendu la prend telle quelle (une
/// fin calée pour s'éteindre avec la séquence). Sinon sa plus courte version qui dure au moins autant, ou la plus longue.
pub fn jingle(r: &Path, id: &str, duree: f64, fin: bool) -> Option<(PathBuf, Option<f64>)> {
    let f = fiche(r, "jingles", id)?;
    let durees = f["durees"].as_object()?;
    let dossier = r.join("jingles").join(id);
    let franche = if fin { "final" } else { "intro" };
    if let Some(d) = durees.get(franche).and_then(Value::as_f64) {
        return Some((dossier.join(format!("{franche}.wav")), Some(d)));
    }
    let mut durees: Vec<(&String, f64)> = durees.iter().filter(|(k, _)| !["intro", "final"].contains(&k.as_str()))
        .filter_map(|(k, v)| Some((k, v.as_f64()?))).collect();
    durees.sort_by(|a, b| a.1.total_cmp(&b.1));
    let (nom, _) = durees.iter().find(|(_, d)| *d + 0.05 >= duree).or_else(|| durees.last())?;
    Some((dossier.join(format!("{nom}.wav")), None))
}

/// Verse un fichier dans le rayon `medias` d'une bibliothèque : gardé sous son nom, ou suffixé si un autre fichier
/// du même nom (et d'une autre taille) y est déjà. Renvoie le nom retenu.
pub fn verser(r: &Path, source: &Path, nom: &str) -> std::io::Result<String> {
    let dossier = r.join("medias");
    std::fs::create_dir_all(&dossier)?;
    let propre: String = nom.chars().map(|c| if c == '/' || c == '\\' { '_' } else { c }).collect();
    let propre = propre.trim_start_matches('.').to_string();
    let (tete, ext) = propre.rsplit_once('.').map(|(t, e)| (t.to_string(), format!(".{e}"))).unwrap_or((propre.clone(), String::new()));
    let taille = std::fs::metadata(source)?.len();
    for k in 0..1000 {
        let n = if k == 0 { format!("{tete}{ext}") } else { format!("{tete} ({k}){ext}") };
        let cible = dossier.join(&n);
        match std::fs::metadata(&cible) {
            Ok(m) if m.len() == taille => return Ok(n),
            Ok(_) => continue,
            Err(_) => {
                if std::fs::hard_link(source, &cible).is_err() {
                    std::fs::copy(source, &cible)?;
                }
                return Ok(n);
            }
        }
    }
    Err(std::io::Error::other("trop de fichiers du même nom"))
}
