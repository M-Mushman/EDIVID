//! Accès : code d'accès (demandé une fois par navigateur) ou compte Google autorisé.
//! Les ACL du tailnet restent la première porte ; ceci est la seconde.

use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::etat::EtatPartage;

const TEMOIN: &str = "edivid_session";
const DUREE_SESSION_GOOGLE_S: i64 = 30 * 24 * 3600;

/// Qui est connecté : par le code (compte vide) ou par Google (son adresse).
pub struct Session {
    pub email: Option<String>,
}

fn temoins(entetes: &HeaderMap) -> Vec<String> {
    entetes
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .filter(|(k, _)| *k == TEMOIN)
        .map(|(_, v)| v.to_string())
        .collect()
}

fn signature(secret: &str, email: &str, expire: i64) -> String {
    format!("{:x}", Sha256::digest(format!("{secret}|{email}|{expire}")))[..40].to_string()
}

fn hex(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

fn dehex(s: &str) -> Option<String> {
    let octets: Option<Vec<u8>> = (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect();
    String::from_utf8(octets?).ok()
}

/// Témoin d'une session ouverte par Google.
pub fn temoin_google(etat: &EtatPartage, email: &str) -> String {
    let expire = chrono::Utc::now().timestamp() + DUREE_SESSION_GOOGLE_S;
    let valeur = format!("g.{}.{expire}.{}", hex(email), signature(&etat.cfg.secret, email, expire));
    format!("{TEMOIN}={valeur}; Path=/; HttpOnly; SameSite=Lax; Max-Age={DUREE_SESSION_GOOGLE_S}")
}

pub async fn session(etat: &EtatPartage, entetes: &HeaderMap) -> Option<Session> {
    for v in temoins(entetes) {
        if v == etat.cfg.jeton {
            return Some(Session { email: None });
        }
        let morceaux: Vec<&str> = v.split('.').collect();
        if let ["g", email_hex, expire, sig] = morceaux.as_slice() {
            let (Some(email), Ok(expire)) = (dehex(email_hex), expire.parse::<i64>()) else { continue };
            let autorise = etat.parametres.read().await.comptes.iter().any(|c| c.email.eq_ignore_ascii_case(&email));
            if autorise && expire > chrono::Utc::now().timestamp() && signature(&etat.cfg.secret, &email, expire) == *sig {
                return Some(Session { email: Some(email) });
            }
        }
    }
    let par_entete = entetes
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|code| code.trim().eq_ignore_ascii_case(&etat.cfg.code_acces))
        .unwrap_or(false);
    par_entete.then_some(Session { email: None })
}

pub async fn garde(State(etat): State<EtatPartage>, requete: Request, suite: Next) -> Response {
    let chemin = requete.uri().path();
    let libre = !chemin.starts_with("/api/")
        || matches!(chemin, "/api/session" | "/api/connexion" | "/api/deconnexion" | "/api/google/connexion" | "/api/google/retour");
    if libre || session(&etat, requete.headers()).await.is_some() {
        suite.run(requete).await
    } else {
        (StatusCode::UNAUTHORIZED, Json(json!({"erreur": "connexion requise"}))).into_response()
    }
}

#[derive(Deserialize)]
pub struct Connexion {
    code: String,
}

pub async fn voir_session(State(etat): State<EtatPartage>, entetes: HeaderMap) -> Json<serde_json::Value> {
    let s = session(&etat, &entetes).await;
    let p = etat.parametres.read().await;
    let compte = s.as_ref().and_then(|s| s.email.as_ref()).and_then(|e| p.comptes.iter().find(|c| c.email.eq_ignore_ascii_case(e)));
    Json(json!({
        "connecte": s.is_some(),
        "compte": compte.map(|c| json!({"email": c.email, "nom": c.nom})),
        "google": !p.google.client_id.is_empty() && !p.google.client_secret.is_empty(),
    }))
}

pub async fn connexion(State(etat): State<EtatPartage>, Json(c): Json<Connexion>) -> Response {
    let saisi: String = c.code.trim().to_uppercase().chars().filter(|c| !c.is_whitespace()).collect();
    if saisi.replace('-', "") != etat.cfg.code_acces.replace('-', "") {
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        return (StatusCode::UNAUTHORIZED, Json(json!({"erreur": "code incorrect"}))).into_response();
    }
    let temoin = format!("{TEMOIN}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=31536000", etat.cfg.jeton);
    ([(header::SET_COOKIE, temoin)], Json(json!({"connecte": true}))).into_response()
}

pub async fn deconnexion() -> Response {
    let temoin = format!("{TEMOIN}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
    ([(header::SET_COOKIE, temoin)], Json(json!({"connecte": false}))).into_response()
}
