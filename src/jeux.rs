//! Jeux de réglages : les réglages, la PostProd, l'habillage et le générique d'un projet, enregistrés
//! sous un nom pour être rechargés tels quels sur un autre projet, ou appliqués aux vidéos venues du
//! Drive (`<donnees>/jeux.json`). Les logos et vidéos dont ils ont besoin sont copiés à part
//! (`<donnees>/jeux/<id>/`), pour survivre au projet d'origine.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::modele::{maintenant, nouvel_id, Generique, Habillage, Media, Projet};
use crate::postprod::PostProd;
use crate::reglages::Reglages;

#[derive(Clone, Serialize, Deserialize)]
pub struct Jeu {
    pub id: String,
    pub nom: String,
    pub cree: String,
    pub reglages: Reglages,
    #[serde(default)]
    pub postprod: PostProd,
    #[serde(default)]
    pub habillage: Vec<Habillage>,
    #[serde(default)]
    pub generique: Generique,
    /// Médias dont l'habillage et le générique ont besoin.
    #[serde(default)]
    pub medias: Vec<Media>,
}

pub fn liste(donnees: &Path) -> Vec<Jeu> {
    std::fs::read_to_string(donnees.join("jeux.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn enregistrer(donnees: &Path, jeux: &[Jeu]) -> Result<()> {
    std::fs::write(donnees.join("jeux.json"), serde_json::to_vec_pretty(jeux)?)?;
    Ok(())
}

pub fn dossier(donnees: &Path, id: &str) -> PathBuf {
    donnees.join("jeux").join(id)
}

pub fn trouver(donnees: &Path, id: &str) -> Option<Jeu> {
    liste(donnees).into_iter().find(|j| j.id == id)
}

/// Médias que l'habillage et le générique utilisent.
pub fn medias_utiles(habillage: &[Habillage], generique: &Generique) -> HashSet<String> {
    let mut ids: HashSet<String> = habillage.iter().filter(|h| !h.media.is_empty()).map(|h| h.media.clone()).collect();
    for s in [&generique.debut, &generique.fin].into_iter().flatten() {
        if !s.media.is_empty() {
            ids.insert(s.media.clone());
        }
    }
    ids
}

/// Copie les médias utiles d'un dossier à l'autre (sans écraser ce qui existe déjà).
pub async fn copier_medias(medias: &[Media], depuis: &Path, vers: &Path) -> Result<()> {
    tokio::fs::create_dir_all(vers).await?;
    for m in medias {
        let cible = vers.join(&m.fichier);
        if !cible.exists() {
            tokio::fs::copy(depuis.join(&m.fichier), &cible).await?;
        }
    }
    Ok(())
}

/// Enregistre un jeu ; un jeu du même nom est remplacé (on met à jour sa recette).
pub async fn ajouter(donnees: &Path, nom: &str, p: &Projet, dossier_medias: &Path) -> Result<Jeu> {
    let nom: String = nom.trim().chars().take(60).collect();
    if nom.is_empty() {
        bail!("donnez un nom à ce jeu de réglages");
    }
    let mut jeux = liste(donnees);
    let id = jeux.iter().find(|j| j.nom.eq_ignore_ascii_case(&nom)).map(|j| j.id.clone()).unwrap_or_else(nouvel_id);
    let utiles = medias_utiles(&p.habillage, &p.generique);
    let medias: Vec<Media> = p.medias.iter().filter(|m| utiles.contains(&m.id)).cloned().collect();
    let _ = tokio::fs::remove_dir_all(dossier(donnees, &id)).await;
    copier_medias(&medias, dossier_medias, &dossier(donnees, &id)).await?;
    jeux.retain(|j| j.id != id);
    let jeu = Jeu {
        id,
        nom,
        cree: maintenant(),
        reglages: p.reglages.clone(),
        postprod: p.postprod.clone(),
        habillage: p.habillage.clone(),
        generique: p.generique.clone(),
        medias,
    };
    jeux.push(jeu.clone());
    jeux.sort_by(|a, b| a.nom.to_lowercase().cmp(&b.nom.to_lowercase()));
    enregistrer(donnees, &jeux)?;
    Ok(jeu)
}

pub fn retirer(donnees: &Path, id: &str) -> Result<()> {
    let mut jeux = liste(donnees);
    jeux.retain(|j| j.id != id);
    let _ = std::fs::remove_dir_all(dossier(donnees, id));
    enregistrer(donnees, &jeux)
}
