//! Jeux de réglages : les réglages et la PostProd d'un projet, enregistrés sous un nom pour être
//! rechargés tels quels sur un autre projet, ou appliqués aux vidéos venues du Drive
//! (`<donnees>/jeux.json`).

use std::path::Path;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::modele::{maintenant, nouvel_id};
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

pub fn trouver(donnees: &Path, id: &str) -> Option<Jeu> {
    liste(donnees).into_iter().find(|j| j.id == id)
}

/// Enregistre un jeu ; un jeu du même nom est remplacé (on met à jour sa recette).
pub fn ajouter(donnees: &Path, nom: &str, reglages: Reglages, postprod: PostProd) -> Result<Jeu> {
    let nom: String = nom.trim().chars().take(60).collect();
    if nom.is_empty() {
        bail!("donnez un nom à ce jeu de réglages");
    }
    let mut jeux = liste(donnees);
    let id = jeux.iter().find(|j| j.nom.eq_ignore_ascii_case(&nom)).map(|j| j.id.clone()).unwrap_or_else(nouvel_id);
    jeux.retain(|j| j.id != id);
    let jeu = Jeu { id, nom, cree: maintenant(), reglages, postprod };
    jeux.push(jeu.clone());
    jeux.sort_by(|a, b| a.nom.to_lowercase().cmp(&b.nom.to_lowercase()));
    enregistrer(donnees, &jeux)?;
    Ok(jeu)
}

pub fn retirer(donnees: &Path, id: &str) -> Result<()> {
    let mut jeux = liste(donnees);
    jeux.retain(|j| j.id != id);
    enregistrer(donnees, &jeux)
}
