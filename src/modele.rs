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
    /// Phrases soudées à la suivante (clé de la première) : la transcription coupait une phrase en deux.
    #[serde(default)]
    pub fusions: Vec<String>,
    /// Versions exportées du rendu final.
    #[serde(default)]
    pub exports: Vec<Export>,
    /// Logos et textes posés sur toute la durée du montage.
    #[serde(default)]
    pub habillage: Vec<Habillage>,
    /// Séquences avant et après le montage.
    #[serde(default)]
    pub generique: Generique,
}

/// Élément posé sur toute la durée du montage : un logo (image de la médiathèque) ou un texte.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct Habillage {
    pub id: String,
    /// « logo » ou « texte ».
    pub genre: String,
    /// Image du logo (médiathèque du projet).
    pub media: String,
    pub texte: String,
    /// haut_gauche, haut_droite, bas_gauche, bas_droite, haut ou bas.
    pub position: String,
    /// Logo : largeur ; texte : hauteur des lettres. En part de la largeur de l'image.
    pub taille: f64,
    /// De 0,1 (presque transparent) à 1.
    pub opacite: f64,
    /// Texte : blanc, noir ou jaune.
    pub couleur: String,
}

/// Générique : une séquence avant le montage, une après.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct Generique {
    pub debut: Option<Sequence>,
    pub fin: Option<Sequence>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct Sequence {
    /// « video » : une vidéo de la médiathèque ; « carton » : un titre sur une image (ou sur un fond uni).
    pub genre: String,
    /// La vidéo, ou l'image de fond du carton (vide : fond uni).
    pub media: String,
    pub titre: String,
    pub sous_titre: String,
    /// Carton : sa durée ; vidéo : durée maximale gardée (0 : entière). En secondes.
    pub duree: f64,
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
    /// En cours de téléchargement depuis Google Drive.
    Telechargement,
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
    /// L'accroche a été choisie à la main : on la prend telle quelle (vide = pas d'accroche), sans repli.
    #[serde(default)]
    pub accroche_choisie: bool,
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
    /// Transition choisie à la main au début de la phrase (« coupe » = aucune) ; prime sur le LLM et le réglage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_choisie: Option<String>,
    /// Mouvement de caméra choisi à la main : « fixe », « pousse » ou « punch » ; absent = selon le rythme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mouvement: Option<String>,
    /// Rangs des mots de la phrase retirés des sous-titres (ils restent dits dans la vidéo).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub masques: Vec<usize>,
    /// Rangs des mots mis en emphase, choisis à la main ; absent = ceux du LLM (mots_forts).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emphase: Option<Vec<usize>>,
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
    /// Clés des phrases d'origine soudées à celle-ci (vide sinon).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fusionnees: Vec<String>,
}

pub fn maintenant() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn nouvel_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
}
