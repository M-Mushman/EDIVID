//! Paramètres de l'instance : courriel et Google Drive. Gardés dans `<donnees>/parametres.json`
//! (droits 600) ; le mot de passe SMTP ne repart jamais vers le navigateur.

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Courriel {
    pub actif: bool,
    pub serveur: String,
    pub port: u16,
    /// ssl (port 465), starttls (port 587) ou aucune (relais local).
    pub securite: String,
    pub identifiant: String,
    pub mot_de_passe: String,
    pub expediteur: String,
    pub destinataires: Vec<String>,
    pub montage: bool,
    pub rendu: bool,
    pub erreur: bool,
}

impl Default for Courriel {
    fn default() -> Self {
        Self {
            actif: false,
            serveur: String::new(),
            port: 465,
            securite: "ssl".into(),
            identifiant: String::new(),
            mot_de_passe: String::new(),
            expediteur: "EDIVID <hello@karayan.tech>".into(),
            destinataires: vec![],
            montage: true,
            rendu: true,
            erreur: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Drive {
    pub actif: bool,
    /// Nom du distant rclone, avec ses deux-points : « gdrive: ».
    pub distant: String,
    pub entree: String,
    pub sortie: String,
    pub intervalle_s: u64,
    pub rendu_final: bool,
    pub deposer: bool,
    /// Projet dont les réglages et consignes sont copiés dans chaque projet venu du Drive.
    pub modele: String,
}

impl Default for Drive {
    fn default() -> Self {
        Self {
            actif: false,
            distant: "gdrive:".into(),
            entree: "EDIVID/À monter".into(),
            sortie: "EDIVID/Montages".into(),
            intervalle_s: 120,
            rendu_final: true,
            deposer: true,
            modele: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Google {
    pub client_id: String,
    pub client_secret: String,
}

/// Compte Google autorisé à se connecter ; son Drive est relié par rclone.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Compte {
    pub email: String,
    pub nom: String,
    /// Distant rclone de son Drive (« edivid_<compte>: »), vide tant qu'il n'est pas relié.
    pub distant: String,
    /// Veille du dossier d'entrée de son Drive.
    pub veille: bool,
    /// Nom de sa chaîne YouTube, lu à la connexion ; vide si inconnu ou sans accès à YouTube.
    pub chaine: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Parametres {
    /// Adresse de l'interface, pour les liens des courriels et le retour de Google.
    pub url_publique: String,
    pub courriel: Courriel,
    pub drive: Drive,
    pub google: Google,
    pub comptes: Vec<Compte>,
}

impl Default for Parametres {
    fn default() -> Self {
        Self {
            url_publique: "http://azoth.tailaae476.ts.net:8030".into(),
            courriel: Courriel::default(),
            drive: Drive::default(),
            google: Google::default(),
            comptes: vec![],
        }
    }
}

pub fn charger(dossier: &Path) -> Parametres {
    std::fs::read_to_string(dossier.join("parametres.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn enregistrer(dossier: &Path, p: &Parametres) -> Result<()> {
    crate::fichiers::ecrire(&dossier.join("parametres.json"), &serde_json::to_vec_pretty(p)?, true)
}

/// Vue envoyée au navigateur : sans le mot de passe.
pub fn vue(p: &Parametres) -> Value {
    let mut v = serde_json::to_value(p).unwrap_or_default();
    v["courriel"]["mot_de_passe"] = json!("");
    v["courriel"]["mot_de_passe_defini"] = json!(!p.courriel.mot_de_passe.is_empty());
    v["google"]["client_secret"] = json!("");
    v["google"]["secret_defini"] = json!(!p.google.client_secret.is_empty());
    v["google"]["configure"] = json!(!p.google.client_id.is_empty() && !p.google.client_secret.is_empty());
    v
}

/// Adresse publique admise : HTTPS, sur un nom du tailnet (…​.ts.net), sans chemin ni identifiants.
pub fn url_publique_admise(u: &str) -> bool {
    let Some(reste) = u.strip_prefix("https://") else { return false };
    let hote = reste.split(['/', '?', '#']).next().unwrap_or("");
    !hote.contains('@') && reste.len() == hote.len() && {
        let nom = hote.split(':').next().unwrap_or("");
        nom.ends_with(".ts.net") && nom.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
    }
}

/// Applique ce que le navigateur renvoie ; un mot de passe vide garde l'ancien (même serveur, même compte).
pub fn fusionner(actuels: &Parametres, recu: &Value) -> Result<Parametres> {
    let mut nouveau = actuels.clone();
    if let Some(u) = recu.get("url_publique").and_then(Value::as_str) {
        let u = u.trim().trim_end_matches('/').to_string();
        // Elle sert d'adresse de retour à Google et de cible à la redirection vers HTTPS : une adresse quelconque
        // enverrait tous les visiteurs ailleurs. Seule une adresse HTTPS du tailnet est admise.
        if !u.is_empty() && !url_publique_admise(&u) {
            anyhow::bail!("adresse publique refusée : une adresse https://….ts.net du tailnet est attendue");
        }
        nouveau.url_publique = u;
    }
    if let Some(c) = recu.get("courriel") {
        let mut c: Courriel = serde_json::from_value(c.clone())?;
        // Un mot de passe vide garde l'ancien — pour le même serveur seulement : sinon, changer le seul serveur
        // enverrait le mot de passe gardé à ce nouveau serveur.
        if c.mot_de_passe.is_empty() && c.serveur.trim().eq_ignore_ascii_case(actuels.courriel.serveur.trim())
            && c.identifiant == actuels.courriel.identifiant {
            c.mot_de_passe = actuels.courriel.mot_de_passe.clone();
        }
        if !c.mot_de_passe.is_empty() && c.securite == "aucune" {
            anyhow::bail!("un mot de passe ne part jamais en clair : choisissez une connexion chiffrée (TLS ou STARTTLS)");
        }
        c.destinataires = c.destinataires.iter().map(|d| d.trim().to_string()).filter(|d| d.contains('@')).collect();
        nouveau.courriel = c;
    }
    if let Some(d) = recu.get("drive") {
        let mut d: Drive = serde_json::from_value(d.clone())?;
        // Un nom de compte rclone seulement : un chemin local (« /… ») laissait parcourir et importer le disque.
        let nom = d.distant.trim().trim_end_matches(':');
        if !nom.is_empty() && !crate::fichiers::identifiant_sur(nom) {
            anyhow::bail!("compte Drive invalide : un nom rclone (lettres, chiffres, _ et -) est attendu");
        }
        d.distant = if nom.is_empty() { String::new() } else { format!("{nom}:") };
        d.entree = d.entree.trim().trim_matches('/').to_string();
        d.sortie = d.sortie.trim().trim_matches('/').to_string();
        d.intervalle_s = d.intervalle_s.clamp(30, 3600);
        nouveau.drive = d;
    }
    if let Some(g) = recu.get("google") {
        let mut g: Google = serde_json::from_value(g.clone())?;
        g.client_id = g.client_id.trim().to_string();
        g.client_secret = g.client_secret.trim().to_string();
        if g.client_secret.is_empty() {
            g.client_secret = actuels.google.client_secret.clone();
        }
        nouveau.google = g;
    }
    Ok(nouveau)
}
