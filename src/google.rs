//! Connexion avec Google (OAuth 2.0, application Web). Elle ouvre la session ET relie le Drive du
//! compte : le jeton est confié à rclone, qui le renouvelle lui-même.
//!
//! - mode « connexion » : réservé aux comptes autorisés (Paramètres) ;
//! - mode « liaison » : depuis une session déjà ouverte, autorise le compte et relie son Drive ;
//! - mode « youtube » : depuis une session ouverte avec Google, choix de la chaîne où publier. Autorisation à part :
//!   une chaîne de marque est un compte sans Drive, que Google refuse (« Service non disponible ») si la même
//!   demande réclame aussi le Drive.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::process::Command;

use crate::acces;
use crate::drive;
use crate::etat::EtatPartage;
use crate::parametres::{self, Compte};

const AUTORISATION: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const JETON: &str = "https://oauth2.googleapis.com/token";
const IDENTITE: &str = "https://openidconnect.googleapis.com/v1/userinfo";
const PORTEES: &str = "openid email profile https://www.googleapis.com/auth/drive";

pub struct Demande {
    mode: String,
    /// Compte de la session, pour le mode « youtube » : la chaîne lui est rattachée.
    email: String,
    cree: Instant,
}

#[derive(Default)]
pub struct Demandes(std::sync::Mutex<HashMap<String, Demande>>);

fn retour(url_publique: &str) -> String {
    format!("{}/api/google/retour", url_publique.trim_end_matches('/'))
}

fn encoder(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[derive(Deserialize)]
pub struct Mode {
    #[serde(default)]
    mode: String,
    /// Adresse du compte à réautoriser : Google la propose d'office.
    #[serde(default)]
    compte: String,
}

/// Envoie le navigateur chez Google.
pub async fn aller(State(etat): State<EtatPartage>, entetes: HeaderMap, Query(m): Query<Mode>) -> Response {
    let p = etat.parametres.read().await.clone();
    if p.google.client_id.is_empty() || p.google.client_secret.is_empty() {
        return Redirect::to("/#/parametres?google=non_configure").into_response();
    }
    let mode = match m.mode.as_str() {
        "liaison" | "youtube" => m.mode.as_str(),
        _ => "connexion",
    };
    let session = acces::session(&etat, &entetes).await;
    if mode == "liaison" && session.is_none() {
        return Redirect::to("/?google=session_requise").into_response();
    }
    let email = session.and_then(|s| s.email).unwrap_or_default();
    if mode == "youtube" && email.is_empty() {
        return Redirect::to("/?google=session_google#/parametres").into_response();
    }
    let jeton: String = uuid::Uuid::new_v4().simple().to_string();
    {
        let mut d = etat.google.0.lock().unwrap();
        d.retain(|_, v| v.cree.elapsed() < Duration::from_secs(900));
        d.insert(jeton.clone(), Demande { mode: mode.into(), email: email.clone(), cree: Instant::now() });
    }
    // Pour la chaîne : Google montre toujours le choix des chaînes du compte.
    let (portees, invite, indice) =
        if mode == "youtube" { (crate::youtube::PORTEES_CHAINE, "select_account%20consent", email) } else { (PORTEES, "consent", m.compte) };
    let url = format!(
        "{AUTORISATION}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt={invite}\
         &state={jeton}{}",
        encoder(&p.google.client_id),
        encoder(&retour(&p.url_publique)),
        encoder(portees),
        if indice.is_empty() { String::new() } else { format!("&login_hint={}", encoder(&indice)) }
    );
    Redirect::to(&url).into_response()
}

#[derive(Deserialize)]
pub struct Retour {
    #[serde(default)]
    code: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    error: String,
}

/// Retour de Google : échange du code, identité, autorisation, liaison du Drive.
pub async fn revenir(State(etat): State<EtatPartage>, Query(r): Query<Retour>) -> Response {
    let demande = etat.google.0.lock().unwrap().remove(&r.state);
    let Some(demande) = demande else {
        return Redirect::to("/?google=expire").into_response();
    };
    if !r.error.is_empty() {
        return Redirect::to("/?google=annule").into_response();
    }
    if demande.mode == "youtube" {
        return match traiter_chaine(&etat, &demande.email, &r.code).await {
            Ok(()) => Redirect::to("/?google=youtube#/parametres").into_response(),
            Err(e) => {
                tracing::warn!("chaîne YouTube : {e:#}");
                Redirect::to(&format!("/?google=erreur_youtube&detail={}#/parametres", encoder(&format!("{e:#}")))).into_response()
            }
        };
    }
    match traiter(&etat, &demande.mode, &r.code).await {
        Ok((email, cible)) => {
            let temoin = acces::temoin_google(&etat, &email);
            let portees = crate::youtube::portees(&etat, &email);
            let manque = acces_manquants(&portees);
            let cible = if manque.is_empty() { cible } else { format!("/?google=partiel&manque={}#/parametres", manque.join(",")) };
            ([(header::SET_COOKIE, temoin)], Redirect::to(&cible)).into_response()
        }
        Err(e) => {
            tracing::warn!("connexion Google : {e:#}");
            let message = encoder(&format!("{e:#}"));
            Redirect::to(&format!("/?google=erreur&detail={message}")).into_response()
        }
    }
}

/// Accès accordés qui manquent : Google présente le Drive en case à cocher (décochée par défaut pour une
/// application non validée) ; sans elle, la connexion réussit mais le Drive refuse tout (« insufficientPermissions »).
fn acces_manquants(portees: &str) -> Vec<&'static str> {
    let mut manque = Vec::new();
    if !portees.contains("auth/drive") {
        manque.push("drive");
    }
    manque
}

/// Échange le code rendu par Google contre les jetons.
async fn echanger(etat: &EtatPartage, code: &str) -> Result<Value> {
    let p = etat.parametres.read().await.clone();
    let jetons: Value = etat
        .http
        .post(JETON)
        .form(&[
            ("code", code),
            ("client_id", &p.google.client_id),
            ("client_secret", &p.google.client_secret),
            ("redirect_uri", &retour(&p.url_publique)),
            ("grant_type", "authorization_code"),
        ])
        .timeout(Duration::from_secs(20))
        .send()
        .await?
        .json()
        .await?;
    Ok(jetons)
}

/// Retour du choix de la chaîne : le jeton de la chaîne est rattaché au compte de la session.
async fn traiter_chaine(etat: &EtatPartage, email: &str, code: &str) -> Result<()> {
    let jetons = echanger(etat, code).await?;
    if !jetons["scope"].as_str().unwrap_or("").contains(crate::youtube::PORTEE) {
        bail!("Google n'a pas accordé l'envoi de vidéos : recommencez et cochez la case « YouTube »");
    }
    let rafraichir = jetons["refresh_token"].as_str().ok_or_else(|| anyhow!("Google n'a pas donné de jeton durable : recommencez"))?;
    let acces_jeton = jetons["access_token"].as_str().ok_or_else(|| anyhow!("Google n'a pas donné de jeton : {jetons}"))?;
    let chaine = crate::youtube::nom_chaine(etat, acces_jeton).await.unwrap_or_default();
    crate::youtube::garder_chaine(etat, email, rafraichir)?;
    let mut params = etat.parametres.write().await;
    let c = params.comptes.iter_mut().find(|c| c.email.eq_ignore_ascii_case(email)).ok_or_else(|| anyhow!("compte {email} inconnu"))?;
    c.chaine = chaine;
    parametres::enregistrer(&etat.cfg.donnees, &params)
}

async fn traiter(etat: &EtatPartage, mode: &str, code: &str) -> Result<(String, String)> {
    let p = etat.parametres.read().await.clone();
    let jetons = echanger(etat, code).await?;
    let acces_jeton = jetons["access_token"].as_str().ok_or_else(|| anyhow!("Google n'a pas donné de jeton : {jetons}"))?;
    let identite: Value = etat.http.get(IDENTITE).bearer_auth(acces_jeton).timeout(Duration::from_secs(20)).send().await?.json().await?;
    let email = identite["email"].as_str().ok_or_else(|| anyhow!("adresse du compte introuvable"))?.to_lowercase();
    let nom = identite["name"].as_str().unwrap_or(&email).to_string();
    let deja = p.comptes.iter().any(|c| c.email.eq_ignore_ascii_case(&email));
    if mode == "connexion" && !deja {
        bail!("le compte {email} n'est pas autorisé : reliez-le d'abord dans Paramètres");
    }
    // Le Drive : rclone reçoit le jeton (il le renouvellera seul grâce au jeton de rafraîchissement).
    let distant = format!("edivid_{}", email.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>());
    let mut drive_relie = false;
    let drive_accorde = !acces_manquants(jetons["scope"].as_str().unwrap_or("")).contains(&"drive");
    if let Some(rafraichir) = jetons["refresh_token"].as_str() {
        // Gardé aussi sur azoth (portées accordées, et anciens comptes qui publiaient avec ce même jeton).
        crate::youtube::garder(etat, &email, crate::youtube::Jetons {
            refresh_token: rafraichir.to_string(),
            portees: jetons["scope"].as_str().unwrap_or("").to_string(),
            ..Default::default()
        })?;
        let expire = chrono::Utc::now() + chrono::Duration::seconds(jetons["expires_in"].as_i64().unwrap_or(3600));
        let jeton_rclone = json!({"access_token": acces_jeton, "token_type": "Bearer", "refresh_token": rafraichir,
                                  "expiry": expire.to_rfc3339()})
            .to_string();
        // Sans l'accès au Drive, pas de liaison : la veille échouerait toutes les deux minutes.
        if drive_accorde {
            relier_rclone(&distant, &p.google.client_id, &p.google.client_secret, &jeton_rclone).await?;
            drive_relie = true;
        }
    }
    // La chaîne YouTube se choisit à part ; si plus rien n'autorise l'envoi, son nom n'a plus lieu d'être affiché.
    let youtube = crate::youtube::autorise(etat, &email);
    // Le compte est autorisé (liaison) ou mis à jour (connexion).
    let nouveau = {
        let mut params = etat.parametres.write().await;
        match params.comptes.iter_mut().find(|c| c.email.eq_ignore_ascii_case(&email)) {
            Some(c) => {
                c.nom = nom.clone();
                if !youtube {
                    c.chaine.clear();
                }
                if drive_relie {
                    c.distant = format!("{distant}:");
                }
            }
            None => params.comptes.push(Compte {
                email: email.clone(),
                nom: nom.clone(),
                distant: if drive_relie { format!("{distant}:") } else { String::new() },
                veille: drive_relie,
                chaine: String::new(),
            }),
        }
        parametres::enregistrer(&etat.cfg.donnees, &params)?;
        params.clone()
    };
    // Dossiers d'entrée et de sortie créés sur ce Drive.
    if drive_relie {
        let mut d = nouveau.drive.clone();
        d.distant = format!("{distant}:");
        if let Err(e) = drive::verifier(&d).await {
            tracing::warn!("dossiers du Drive de {email} non créés : {e:#}");
        }
        etat.drive.reveil.notify_one();
    }
    let cible = if mode == "liaison" { "/#/parametres" } else { "/" };
    Ok((email, cible.to_string()))
}

/// Compte rclone du Drive, écrit directement dans le fichier de configuration de rclone (droits 600). Passés à
/// `rclone config create`, le secret du client et le jeton apparaissaient dans la liste des processus.
async fn relier_rclone(nom: &str, client_id: &str, client_secret: &str, jeton: &str) -> Result<()> {
    let sortie = Command::new(drive::rclone()).args(["config", "file"]).output().await.context("rclone introuvable")?;
    let fichier = String::from_utf8_lossy(&sortie.stdout).lines().last().map(|l| l.trim().to_string())
        .filter(|l| l.starts_with('/')).ok_or_else(|| anyhow::anyhow!("fichier de configuration de rclone introuvable"))?;
    let fichier = std::path::PathBuf::from(fichier);
    if let Some(d) = fichier.parent() {
        tokio::fs::create_dir_all(d).await?;
    }
    let ancien = tokio::fs::read_to_string(&fichier).await.unwrap_or_default();
    let texte = remplacer_section(&ancien, nom, &[("type", "drive"), ("client_id", client_id), ("client_secret", client_secret),
                                                  ("scope", "drive"), ("token", jeton)]);
    crate::fichiers::ecrire(&fichier, texte.as_bytes(), true)
}

/// Remplace (ou ajoute) la section `[nom]` d'un fichier INI de rclone, sans toucher aux autres.
fn remplacer_section(ancien: &str, nom: &str, cles: &[(&str, &str)]) -> String {
    let mut sortie = String::new();
    let mut dans = false;
    for ligne in ancien.lines() {
        let t = ligne.trim();
        if t.starts_with('[') && t.ends_with(']') {
            dans = &t[1..t.len() - 1] == nom;
        }
        if !dans {
            sortie.push_str(ligne);
            sortie.push('\n');
        }
    }
    if !sortie.is_empty() && !sortie.ends_with("\n\n") {
        sortie.push('\n');
    }
    sortie.push_str(&format!("[{nom}]\n"));
    for (k, v) in cles {
        sortie.push_str(&format!("{k} = {v}\n"));
    }
    sortie
}

#[cfg(test)]
mod tests {
    #[test]
    fn section_rclone() {
        let ancien = "[a]\ntype = drive\ntoken = x\n\n[edivid_lea]\ntype = drive\ntoken = vieux\n[b]\ntype = local\n";
        let t = super::remplacer_section(ancien, "edivid_lea", &[("type", "drive"), ("token", "neuf")]);
        assert!(t.contains("[a]\ntype = drive\ntoken = x") && t.contains("[b]\ntype = local"));
        assert!(t.contains("[edivid_lea]\ntype = drive\ntoken = neuf") && !t.contains("vieux"));
        assert_eq!(t.matches("[edivid_lea]").count(), 1);
    }
}
