//! Données d'un projet, telles qu'enregistrées dans `projet.json`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::reglages::Reglages;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Projet {
    pub id: String,
    pub nom: String,
    pub cree: String,
    #[serde(default)]
    pub rushs: Vec<Rush>,
    #[serde(default)]
    pub medias: Vec<Media>,
    #[serde(default)]
    pub reglages: Reglages,
    #[serde(default)]
    pub montage: Option<Montage>,
    #[serde(default)]
    pub incrustations: Vec<Incrustation>,
    #[serde(default)]
    pub discussion: Vec<Echange>,
    #[serde(default)]
    pub apercu: Option<Sortie>,
    #[serde(default)]
    pub rendu: Option<Sortie>,
    /// Consignes de la personne au monteur, ajoutées à celles du LLM.
    #[serde(default)]
    pub consignes: String,
    /// Chaîne automatique (projet venu du Google Drive) : jusqu'où aller sans intervention.
    #[serde(default)]
    pub auto: Option<Auto>,
    /// Le prochain aperçu suit un montage complet : il donne lieu au courriel « montage prêt ».
    #[serde(default)]
    pub notifier_montage: bool,
    /// Chemin du rendu déposé sur le Google Drive.
    #[serde(default)]
    pub depot_drive: Option<String>,
    /// « Google Drive » pour un projet créé par la veille du dossier d'entrée.
    #[serde(default)]
    pub origine: String,
    /// Compte Google dont le Drive a fourni le projet : la vidéo y retourne, le courriel lui est envoyé.
    #[serde(default)]
    pub compte: String,
    /// Traitements d'image sur le montage entier (onglet PostProd).
    #[serde(default)]
    pub postprod: crate::postprod::PostProd,
    /// Textes de phrases corrigés à la main (orthographe, erreurs de transcription), par clé de phrase.
    #[serde(default)]
    pub corrections: std::collections::BTreeMap<String, String>,
    /// Versions exportées du rendu final.
    #[serde(default)]
    pub exports: Vec<Export>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Export {
    pub id: String,
    /// youtube, leger ou max.
    pub preset: String,
    pub fichier: String,
    pub cree: String,
    pub taille: u64,
    pub largeur: u32,
    pub hauteur: u32,
    /// Lien de la vidéo publiée sur YouTube.
    #[serde(default)]
    pub youtube: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Auto {
    pub rendu_final: bool,
    pub deposer_drive: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EtatRush {
    Televerse,
    Preparation,
    Prepare,
    Analyse,
    Pret,
    Erreur,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rush {
    pub id: String,
    pub nom: String,
    /// Nom du fichier d'origine dans `rushs/`.
    pub fichier: String,
    pub taille: u64,
    pub etat: EtatRush,
    #[serde(default)]
    pub erreur: Option<String>,
    #[serde(default)]
    pub source: Option<InfosSource>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InfosSource {
    pub largeur: u32,
    pub hauteur: u32,
    pub duree: f64,
    pub ips: f64,
    pub hdr: Option<String>,
    pub codec: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Media {
    pub id: String,
    pub nom: String,
    pub fichier: String,
    /// image, gif, video ou son.
    pub genre: String,
    pub duree: Option<f64>,
    pub a_son: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Montage {
    pub cree: String,
    pub phrases: Vec<Decision>,
    #[serde(default)]
    pub accroche: Vec<String>,
    #[serde(default)]
    pub chapitres: Vec<Chapitre>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Decision {
    /// Clé stable de la phrase : `<id du rush>:<rang>`.
    pub cle: String,
    pub garder: bool,
    pub regime: String,
    pub cadre: String,
    #[serde(default)]
    pub transition: Option<String>,
    #[serde(default)]
    pub mots_forts: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chapitre {
    pub cle: String,
    pub titre: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Incrustation {
    pub id: String,
    /// Média de la médiathèque, ou vide pour un effet intégré.
    #[serde(default)]
    pub media: String,
    /// "" (média), "secousse" ou "noir_blanc".
    #[serde(default)]
    pub effet: String,
    /// Phrase où l'incrustation commence.
    pub phrase: String,
    #[serde(default)]
    pub decalage: f64,
    pub duree: f64,
    /// plein, centre, haut_gauche, haut_droite, bas_gauche, bas_droite.
    pub position: String,
    /// Largeur relative à l'image, de 0.1 à 1.
    pub taille: f64,
    #[serde(default)]
    pub fond_vert: bool,
    #[serde(default)]
    pub son: bool,
    /// Texte incrusté (module Texte) ; vide pour un média ou un effet.
    #[serde(default)]
    pub texte: String,
    /// blanc, jaune, noir.
    #[serde(default)]
    pub couleur: String,
    /// aucun, boite, bandeau.
    #[serde(default)]
    pub fond: String,
    /// Effet d'entrée : aucune, fondu, splash, glisse_gauche, glisse_droite, glisse_haut, machine.
    #[serde(default)]
    pub entree: String,
    /// Centre choisi par glisser-déposer (0–1) ; à défaut, la position nommée.
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Echange {
    /// "moi" ou "edivid".
    pub role: String,
    pub texte: String,
    pub quand: String,
    #[serde(default)]
    pub actions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sortie {
    pub fichier: String,
    pub cree: String,
    pub rapport: Value,
}

/// Analyse d'un rush, écrite par `capteurs/analyse.py`.
#[derive(Clone, Debug, Deserialize)]
pub struct Analyse {
    pub largeur: u32,
    pub hauteur: u32,
    pub duree: f64,
    pub mots: Vec<Mot>,
    pub visages: Vec<Visage>,
    #[serde(skip)]
    pub enveloppe: Vec<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mot {
    pub t0: f64,
    pub t1: f64,
    pub mot: String,
    #[serde(default)]
    pub db: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Visage {
    pub t: f64,
    pub visage: [f64; 4],
    pub yeux: [f64; 4],
}

/// Phrase reconstituée à partir des mots d'un rush.
#[derive(Clone, Debug, Serialize)]
pub struct Phrase {
    pub cle: String,
    pub rush: String,
    pub t0: f64,
    pub t1: f64,
    pub silence_avant: f64,
    pub db: f64,
    pub texte: String,
    #[serde(skip)]
    pub mots: Vec<Mot>,
}

pub fn maintenant() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn nouvel_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
}
