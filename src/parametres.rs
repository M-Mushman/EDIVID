//! Paramètres de l'instance : courriel et Google Drive. Gardés dans `<donnees>/parametres.json`
//! (droits 600) ; le mot de passe SMTP ne repart jamais vers le navigateur.

use std::os::unix::fs::PermissionsExt;
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
    let chemin = dossier.join("parametres.json");
    let tmp = dossier.join("parametres.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(p)?)?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    std::fs::rename(tmp, chemin)?;
    Ok(())
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

/// Applique ce que le navigateur renvoie ; un mot de passe vide garde l'ancien.
pub fn fusionner(actuels: &Parametres, recu: &Value) -> Result<Parametres> {
    let mut nouveau = actuels.clone();
    if let Some(u) = recu.get("url_publique").and_then(Value::as_str) {
        nouveau.url_publique = u.trim().trim_end_matches('/').to_string();
    }
    if let Some(c) = recu.get("courriel") {
        let mut c: Courriel = serde_json::from_value(c.clone())?;
        if c.mot_de_passe.is_empty() {
            c.mot_de_passe = actuels.courriel.mot_de_passe.clone();
        }
        c.destinataires = c.destinataires.iter().map(|d| d.trim().to_string()).filter(|d| d.contains('@')).collect();
        nouveau.courriel = c;
    }
    if let Some(d) = recu.get("drive") {
        let mut d: Drive = serde_json::from_value(d.clone())?;
        if !d.distant.is_empty() && !d.distant.ends_with(':') && !d.distant.starts_with('/') {
            d.distant.push(':');
        }
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
