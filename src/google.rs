//! Connexion avec Google (OAuth 2.0, application Web). Elle ouvre la session ET relie le Drive du
//! compte : le jeton est confié à rclone, qui le renouvelle lui-même.
//!
//! - mode « connexion » : réservé aux comptes autorisés (Paramètres) ;
//! - mode « liaison » : depuis une session déjà ouverte, autorise le compte et relie son Drive.

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
const PORTEES: &str = "openid email profile https://www.googleapis.com/auth/drive https://www.googleapis.com/auth/youtube.upload";

pub struct Demande {
    mode: String,
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
}

/// Envoie le navigateur chez Google.
pub async fn aller(State(etat): State<EtatPartage>, entetes: HeaderMap, Query(m): Query<Mode>) -> Response {
    let p = etat.parametres.read().await.clone();
    if p.google.client_id.is_empty() || p.google.client_secret.is_empty() {
        return Redirect::to("/#/parametres?google=non_configure").into_response();
    }
    let mode = if m.mode == "liaison" { "liaison" } else { "connexion" };
    if mode == "liaison" && acces::session(&etat, &entetes).await.is_none() {
        return Redirect::to("/?google=session_requise").into_response();
    }
    let jeton: String = uuid::Uuid::new_v4().simple().to_string();
    {
        let mut d = etat.google.0.lock().unwrap();
        d.retain(|_, v| v.cree.elapsed() < Duration::from_secs(900));
        d.insert(jeton.clone(), Demande { mode: mode.into(), cree: Instant::now() });
    }
    let url = format!(
        "{AUTORISATION}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent\
         &include_granted_scopes=true&state={jeton}",
        encoder(&p.google.client_id),
        encoder(&retour(&p.url_publique)),
        encoder(PORTEES)
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
    match traiter(&etat, &demande.mode, &r.code).await {
        Ok((email, cible)) => {
            let temoin = acces::temoin_google(&etat, &email);
            ([(header::SET_COOKIE, temoin)], Redirect::to(&cible)).into_response()
        }
        Err(e) => {
            tracing::warn!("connexion Google : {e:#}");
            let message = encoder(&format!("{e:#}"));
            Redirect::to(&format!("/?google=erreur&detail={message}")).into_response()
        }
    }
}

async fn traiter(etat: &EtatPartage, mode: &str, code: &str) -> Result<(String, String)> {
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
    let acces_jeton = jetons["access_token"].as_str().ok_or_else(|| anyhow!("Google n'a pas donné de jeton : {jetons}"))?;
    let identite: Value = etat.http.get(IDENTITE).bearer_auth(acces_jeton).timeout(Duration::from_secs(20)).send().await?.json().await?;
    let email = identite["email"].as_str().ok_or_else(|| anyhow!("adresse du compte introuvable"))?.to_lowercase();
    let nom = identite["name"].as_str().unwrap_or(&email).to_string();
    let deja = p.comptes.iter().any(|c| c.email.eq_ignore_ascii_case(&email));
    if mode == "connexion" && !deja {
        bail!("le compte {email} n'est pas autorisé : ouvrez EDIVID avec le code d'accès, puis reliez ce compte dans Paramètres");
    }
    // Le Drive : rclone reçoit le jeton (il le renouvellera seul grâce au jeton de rafraîchissement).
    let distant = format!("edivid_{}", email.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>());
    let mut drive_relie = false;
    if let Some(rafraichir) = jetons["refresh_token"].as_str() {
        // Gardé aussi pour YouTube, que rclone ne sait pas servir.
        crate::youtube::garder(etat, &email, crate::youtube::Jetons {
            refresh_token: rafraichir.to_string(),
            portees: jetons["scope"].as_str().unwrap_or("").to_string(),
        })?;
        let expire = chrono::Utc::now() + chrono::Duration::seconds(jetons["expires_in"].as_i64().unwrap_or(3600));
        let jeton_rclone = json!({"access_token": acces_jeton, "token_type": "Bearer", "refresh_token": rafraichir,
                                  "expiry": expire.to_rfc3339()})
            .to_string();
        relier_rclone(&distant, &p.google.client_id, &p.google.client_secret, &jeton_rclone).await?;
        drive_relie = true;
    }
    // Le compte est autorisé (liaison) ou mis à jour (connexion).
    let nouveau = {
        let mut params = etat.parametres.write().await;
        match params.comptes.iter_mut().find(|c| c.email.eq_ignore_ascii_case(&email)) {
            Some(c) => {
                c.nom = nom.clone();
                if drive_relie {
                    c.distant = format!("{distant}:");
                }
            }
            None => params.comptes.push(Compte {
                email: email.clone(),
                nom: nom.clone(),
                distant: if drive_relie { format!("{distant}:") } else { String::new() },
                veille: drive_relie,
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

async fn relier_rclone(nom: &str, client_id: &str, client_secret: &str, jeton: &str) -> Result<()> {
    let rclone = drive::rclone();
    let _ = Command::new(&rclone).args(["config", "delete", nom]).output().await;
    let sortie = Command::new(&rclone)
        .args(["config", "create", nom, "drive", &format!("client_id={client_id}"), &format!("client_secret={client_secret}"),
               "scope=drive", &format!("token={jeton}"), "--non-interactive"])
        .output()
        .await
        .context("rclone introuvable")?;
    if !sortie.status.success() {
        bail!("liaison du Drive refusée par rclone : {}", String::from_utf8_lossy(&sortie.stderr).trim());
    }
    Ok(())
}
