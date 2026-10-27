//! Bibliothèque de demandes : celles livrées avec EDIVID (`bibliotheque/demandes.json`) et celles
//! que les utilisateurs de cette instance enregistrent (`<donnees>/bibliotheque.json`).

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::modele::{maintenant, nouvel_id};

const INTEGREE: &str = include_str!("../bibliotheque/demandes.json");

#[derive(Clone, Serialize, Deserialize)]
pub struct Demande {
    pub id: String,
    pub titre: String,
    pub texte: String,
    pub cree: String,
}

pub fn integree() -> Value {
    serde_json::from_str(INTEGREE).expect("bibliotheque/demandes.json invalide")
}

pub fn perso(donnees: &Path) -> Vec<Demande> {
    std::fs::read_to_string(donnees.join("bibliotheque.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn enregistrer(donnees: &Path, d: &[Demande]) -> Result<()> {
    crate::fichiers::ecrire(&donnees.join("bibliotheque.json"), &serde_json::to_vec_pretty(d)?, false)?;
    Ok(())
}

pub fn ajouter(donnees: &Path, titre: &str, texte: &str) -> Result<Demande> {
    let mut v = perso(donnees);
    let d = Demande {
        id: nouvel_id(),
        titre: titre.trim().chars().take(80).collect(),
        texte: texte.trim().chars().take(2000).collect(),
        cree: maintenant(),
    };
    v.push(d.clone());
    enregistrer(donnees, &v)?;
    Ok(d)
}

pub fn retirer(donnees: &Path, id: &str) -> Result<()> {
    let mut v = perso(donnees);
    v.retain(|d| d.id != id);
    enregistrer(donnees, &v)
}
