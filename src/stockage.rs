//! Fichiers d'un projet sur le disque : ce qu'ils pèsent, et ce qui peut être libéré parce qu'EDIVID sait le
//! refaire. Les rushs d'origine, le rendu final, l'aperçu en cours, les analyses et les versions légères pour le
//! lecteur ne se libèrent pas ici (un rush ou un export se retirent à leur place dans l'interface).

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::etat::EtatPartage;
use crate::modele::Projet;

struct Categorie {
    cle: &'static str,
    titre: &'static str,
    aide: &'static str,
    supprimable: bool,
}

const CATEGORIES: &[Categorie] = &[
    Categorie { cle: "rushs", titre: "Rushs d'origine", aide: "Les vidéos déposées, telles quelles.", supprimable: false },
    Categorie { cle: "travail", titre: "Fichiers de travail",
                aide: "Copie 10 bits de chaque rush, prête au montage. Recréée depuis le rush au prochain aperçu ou rendu (environ deux fois la durée du rush).",
                supprimable: true },
    Categorie { cle: "voix", titre: "Voix traitées par IA",
                aide: "Voix débruitée ou reconstruite de chaque rush. Recalculée sur la carte graphique au prochain aperçu ou rendu qui en a besoin.",
                supprimable: true },
    Categorie { cle: "ecoutes", titre: "Écoutes d'essai", aide: "Extraits de vingt secondes des réglages de son essayés.", supprimable: true },
    Categorie { cle: "extraits", titre: "Extraits en qualité finale", aide: "Plans calculés par « Ce plan en qualité finale ».", supprimable: true },
    Categorie { cle: "temporaires", titre: "Fichiers temporaires", aide: "Restes d'un aperçu ou d'un rendu interrompu.", supprimable: true },
    Categorie { cle: "sorties", titre: "Aperçu, rendu final et exports", aide: "Les vidéos produites (un export se retire dans « Exporter »).", supprimable: false },
    Categorie { cle: "lecteur", titre: "Analyses et versions pour le lecteur", aide: "Transcription, visages, version légère des rushs.", supprimable: false },
];

/// Fichiers (ou dossiers) d'une catégorie.
fn chemins(etat: &EtatPartage, p: &Projet, cle: &str) -> Vec<PathBuf> {
    let dossier = etat.dossier_projet(&p.id);
    let travail = |r: &str| etat.dossier_travail(&p.id, r);
    let mut v = Vec::new();
    match cle {
        "rushs" => v.push(dossier.join("rushs")),
        "travail" => v.extend(p.rushs.iter().flat_map(|r| [travail(&r.id).join("mezz.mkv"), travail(&r.id).join("mezz_hlg.mkv")])),
        "voix" => {
            for r in &p.rushs {
                if let Ok(lus) = std::fs::read_dir(travail(&r.id)) {
                    v.extend(lus.flatten().map(|e| e.path()).filter(|f| {
                        let n = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        n.starts_with("voix_ia_") || n.starts_with("voix_reconstruite")
                    }));
                }
            }
        }
        "ecoutes" => v.extend(p.rushs.iter().map(|r| travail(&r.id).join("essais"))),
        "extraits" => v.extend(fichiers_de(&dossier.join("sorties"), |n| n.starts_with("qualite-"))),
        "temporaires" => v.push(dossier.join("tmp")),
        "sorties" => v.extend(fichiers_de(&dossier.join("sorties"), |n| !n.starts_with("qualite-"))),
        "lecteur" => {
            for r in &p.rushs {
                for f in ["proxy.mp4", "analyse.json", "enveloppe.f32", "sonie.txt"] {
                    v.push(travail(&r.id).join(f));
                }
            }
        }
        _ => {}
    }
    v
}

fn fichiers_de(dossier: &Path, garder: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    std::fs::read_dir(dossier).map(|l| l.flatten().map(|e| e.path())
        .filter(|f| f.file_name().is_some_and(|n| garder(&n.to_string_lossy()))).collect()).unwrap_or_default()
}

/// Octets d'un fichier ou d'un dossier (récursif).
fn taille(chemin: &Path) -> u64 {
    match std::fs::symlink_metadata(chemin) {
        Ok(m) if m.is_dir() => std::fs::read_dir(chemin).map(|l| l.flatten().map(|e| taille(&e.path())).sum()).unwrap_or(0),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

/// La place occupée par catégorie.
pub fn bilan(etat: &EtatPartage, p: &Projet) -> Value {
    let categories: Vec<Value> = CATEGORIES.iter().map(|c| {
        let octets: u64 = chemins(etat, p, c.cle).iter().map(|f| taille(f)).sum();
        json!({"cle": c.cle, "titre": c.titre, "aide": c.aide, "supprimable": c.supprimable, "octets": octets})
    }).collect();
    let total: u64 = taille(&etat.dossier_projet(&p.id));
    json!({"categories": categories, "total": total})
}

/// Libère une catégorie recalculable. Rend les octets libérés.
pub fn liberer(etat: &EtatPartage, p: &Projet, cle: &str) -> Result<u64> {
    let Some(c) = CATEGORIES.iter().find(|c| c.cle == cle) else { bail!("catégorie inconnue") };
    if !c.supprimable {
        bail!("« {} » ne se libère pas ici", c.titre);
    }
    let mut libere = 0;
    for f in chemins(etat, p, cle) {
        let t = taille(&f);
        let res = if f.is_dir() { std::fs::remove_dir_all(&f) } else { std::fs::remove_file(&f) };
        if res.is_ok() {
            libere += t;
        }
    }
    Ok(libere)
}

#[cfg(test)]
mod tests {
    /// Place d'un projet réel : `EDIVID_PROJET=<id> cargo test stockage -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn bilan_d_un_projet() {
        let cfg = crate::config::Config::charger().unwrap();
        let etat = crate::etat::Etat::nouveau(cfg, false).await.unwrap();
        let p = etat.projet(&std::env::var("EDIVID_PROJET").unwrap()).await.unwrap();
        println!("{}", serde_json::to_string_pretty(&super::bilan(&etat, &p)).unwrap());
    }
}
