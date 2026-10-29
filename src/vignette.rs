//! Vignette YouTube : Edina dessinée dans un style de la bibliothèque d'habillage, dans une scène que le LLM tire
//! de la vidéo, avec le titre posé par le code (capteurs/vignette.py). Quatre propositions par fabrication, dans
//! `sorties/vignettes/` ; la personne en choisit une dans la fiche de la vidéo.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::etat::EtatPartage;
use crate::{llm, rayons, rendu};

/// FLUX.2 klein 4B avec ses deux images de référence, modèle déchargé par morceaux sur le processeur.
const VRAM_VIGNETTE_MO: u64 = 13_500;
const PROPOSITIONS: u64 = 4;
/// « references » (scène entière dessinée d'après la photo et une image du style), « retouche » (la photo
/// redessinée dans le style, décor changé) ou « mixte » (deux de chaque) : voir capteurs/vignette.py.
const METHODE: &str = "mixte";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Demande {
    /// Photo d'Edina (rayon photos de la bibliothèque).
    pub photo: String,
    /// Style (rayon styles) : ses images servent de référence, une par proposition, à tour de rôle.
    pub style: String,
    /// Logo by Edina sous le titre.
    pub logo: bool,
}

pub fn dossier(etat: &EtatPartage, projet: &str) -> PathBuf {
    etat.dossier_projet(projet).join("sorties").join("vignettes")
}

/// Les vignettes déjà fabriquées, les plus récentes d'abord.
pub fn liste(etat: &EtatPartage, projet: &str) -> Vec<String> {
    let mut v: Vec<(std::time::SystemTime, String)> = std::fs::read_dir(dossier(etat, projet)).into_iter().flatten().flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            (n.starts_with("vignette_") && n.ends_with(".jpg")).then(|| (e.metadata().and_then(|m| m.modified()).ok(), n))
        })
        .filter_map(|(t, n)| Some((t?, n)))
        .collect();
    v.sort_by(|a, b| b.cmp(a));
    v.into_iter().map(|(_, n)| n).collect()
}

/// Vérifie une demande avant de la mettre en file ; `r` : la bibliothèque du propriétaire du projet.
pub fn verifier(r: &Path, d: &Demande) -> Result<()> {
    if !rayons::photo_valide(&d.photo) || !r.join("photos").join(&d.photo).is_file() {
        bail!("choisissez une photo de la bibliothèque");
    }
    if !rayons::id_valide(&d.style) || !r.join("styles").join(&d.style).is_dir() {
        bail!("choisissez un style de la bibliothèque");
    }
    Ok(())
}

fn texte_vignette(p: &crate::modele::Projet) -> String {
    [&p.fiche.texte_vignette, &p.fiche.titre, &p.nom].into_iter().find(|t| !t.trim().is_empty()).cloned().unwrap_or_default()
}

fn logo(r: &Path, avec: bool) -> String {
    let f = r.join("logos").join("by-edina.png");
    if avec && f.is_file() { f.to_string_lossy().into() } else { String::new() }
}

/// Fabrique quatre propositions : la scène par le LLM, puis le dessin sous bail de la carte.
pub async fn fabriquer(etat: &EtatPartage, projet: &str, d: &Demande) -> Result<()> {
    let p = etat.projet(projet).await?;
    let r = rayons::racine(&etat.cfg.donnees, &p.proprietaire);
    verifier(&r, d)?;
    let contenu = crate::youtube::texte_du_montage(etat, &p).await.unwrap_or_default();
    let schema = json!({"type": "object", "properties": {"scene": {"type": "string"}, "lieu": {"type": "string"}},
                        "required": ["scene", "lieu"]});
    let consigne = "Tu prépares la vignette YouTube d'une vidéo face caméra en français. La personne qui parle est Edina, \
une femme de plus de cinquante ans, souriante et en pleine forme. À partir du titre, de la description et du texte de la \
vidéo, décris en anglais, en une ou deux phrases, une scène lumineuse et positive où elle apparaît seule : le lieu, ce \
qu'elle fait, sa tenue, l'ambiance (« scene ») ; puis, à part, le lieu seul en quelques mots anglais, sans personne (« lieu », par exemple « a sunny lakeside path with mountains »). Pas de texte ni d'écriteau. Réponds en JSON.";
    let entree = format!("Titre : {}\nDescription : {}\n\nTexte de la vidéo :\n{}", p.fiche.titre, p.fiche.description,
                         contenu.chars().take(8000).collect::<String>());
    let v = llm::appeler(etat, projet, consigne, &entree, &schema, false).await?;
    let scene = v["scene"].as_str().unwrap_or("").trim().to_string();
    let lieu = v["lieu"].as_str().unwrap_or("").trim().to_string();
    if scene.is_empty() {
        bail!("le LLM n'a pas décrit de scène");
    }
    let styles: Vec<String> = rayons::images(&r.join("styles").join(&d.style)).into_iter()
        .map(|n| r.join("styles").join(&d.style).join(n).to_string_lossy().into()).collect();
    if styles.is_empty() {
        bail!("ce style n'a aucune image");
    }
    let sortie = dossier(etat, projet);
    tokio::fs::create_dir_all(&sortie).await?;
    // Graines tirées de l'heure : chaque fabrication ajoute ses propositions aux précédentes.
    let base = chrono::Utc::now().timestamp() as u64 % 100_000_000 * 10;
    let graines: Vec<u64> = (0..PROPOSITIONS).map(|k| base + k).collect();
    let travail = json!({"photo": r.join("photos").join(&d.photo), "styles": styles, "scene": scene, "lieu": lieu,
                         "methode": METHODE, "texte": texte_vignette(&p),
                         "logo": logo(&r, d.logo), "sortie": sortie, "graines": graines});
    let fichier = sortie.join(format!("travail_{base}.json"));
    tokio::fs::write(&fichier, serde_json::to_vec_pretty(&travail)?).await?;
    let bail = etat.carte.prendre(etat, projet, "other:vignette", VRAM_VIGNETTE_MO, 300).await?;
    let res = rendu::capteur(etat, projet, "vignette.py", &["dessiner".into(), fichier.to_string_lossy().into()]).await;
    etat.carte.rendre(bail).await;
    res?;
    etat.emettre(projet, "projet", json!({}));
    Ok(())
}

/// Repose le texte (et le logo) sur toutes les vignettes déjà dessinées, sur le processeur.
pub async fn retitrer(etat: &EtatPartage, projet: &str, logo_voulu: bool) -> Result<()> {
    let p = etat.projet(projet).await?;
    let r = rayons::racine(&etat.cfg.donnees, &p.proprietaire);
    let texte = texte_vignette(&p);
    let travaux: Vec<PathBuf> = std::fs::read_dir(dossier(etat, projet)).into_iter().flatten().flatten().map(|e| e.path())
        .filter(|f| f.file_name().is_some_and(|n| n.to_string_lossy().starts_with("travail_")))
        .collect();
    for f in travaux {
        let mut t: serde_json::Value = serde_json::from_slice(&tokio::fs::read(&f).await?)?;
        t["texte"] = json!(texte);
        t["logo"] = json!(logo(&r, logo_voulu));
        tokio::fs::write(&f, serde_json::to_vec_pretty(&t)?).await?;
        rendu::capteur(etat, projet, "vignette.py", &["titrer".into(), f.to_string_lossy().into()]).await?;
    }
    Ok(())
}

/// Chemin d'une vignette du projet, ou une erreur si le nom n'en est pas une.
pub fn chemin(etat: &EtatPartage, projet: &str, nom: &str) -> Result<PathBuf> {
    let id = nom.strip_suffix(".jpg").and_then(|n| n.strip_prefix("vignette_")).ok_or_else(|| anyhow!("nom invalide"))?;
    if !id.chars().all(|c| c.is_ascii_digit()) {
        bail!("nom invalide");
    }
    let f = dossier(etat, projet).join(nom);
    if !f.is_file() {
        bail!("vignette introuvable");
    }
    Ok(f)
}

pub fn existe(dossier_vignettes: &Path, nom: &str) -> bool {
    nom.starts_with("vignette_") && nom.ends_with(".jpg") && !nom.contains('/') && dossier_vignettes.join(nom).is_file()
}
