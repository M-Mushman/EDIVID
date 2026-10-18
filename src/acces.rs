//! Accès : appareil du tailnet reconnu par Tailscale (sans rien saisir), sinon code d'accès (demandé une
//! fois par navigateur) ou compte Google autorisé. Les ACL du tailnet restent la première porte.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Request, State};
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
/// Posé par `tailscale serve` (HTTPS du tailnet) quand l'appareil appartient à une personne (pas à une
/// étiquette comme le VPS) ; le relais efface celui qu'un client aurait fabriqué (vérifié le 2026-10-18).
/// Il ne fait foi que par ce relais, qui arrive sur l'écoute locale : voir `garde`.
const IDENTITE_TAILNET: &str = "tailscale-user-login";

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
    // Appareil du tailnet : il entre comme avec le code, sans que personne ait rien à saisir.
    if entetes.get(IDENTITE_TAILNET).is_some_and(|v| !v.is_empty()) {
        return Some(Session { email: None });
    }
    let par_entete = entetes
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|code| code.trim().eq_ignore_ascii_case(&etat.cfg.code_acces))
        .unwrap_or(false);
    par_entete.then_some(Session { email: None })
}

/// En HTTP/1.1 (adresse http://…:8030), un navigateur n'ouvre que six connexions par site ; chaque onglet
/// EDIVID en garde une pour ses événements, et quelques onglets suffisent à tout bloquer (mesuré le
/// 2026-10-18 : six onglets, plus rien ne répond). L'adresse HTTPS du tailnet passe par HTTP/2, sans
/// cette limite : une page demandée en clair sur le tailnet y est donc renvoyée.
async fn vers_https(etat: &EtatPartage, chemin: &str, get: bool, hote: &str, suite: &str) -> Option<Response> {
    if chemin.starts_with("/api/") || !get {
        return None;
    }
    let local = hote.starts_with("127.0.0.1") || hote.starts_with("localhost");
    // tailscale serve relaie l'HTTPS vers 127.0.0.1 : seules les requêtes arrivées en clair sur le tailnet sont visées.
    if local || !hote.ends_with(":8030") {
        return None;
    }
    let cible = etat.parametres.read().await.url_publique.clone();
    if !cible.starts_with("https://") {
        return None;
    }
    Some(axum::response::Redirect::temporary(&format!("{}{suite}", cible.trim_end_matches('/'))).into_response())
}

pub async fn garde(State(etat): State<EtatPartage>, mut requete: Request, suite: Next) -> Response {
    // Sur l'adresse directe du tailnet (…:8030), n'importe quel appareil pourrait écrire l'identité : retirée.
    let par_relais = requete.extensions().get::<ConnectInfo<SocketAddr>>().is_some_and(|c| c.0.ip().is_loopback());
    if !par_relais {
        requete.headers_mut().remove(IDENTITE_TAILNET);
    }
    let chemin = requete.uri().path().to_string();
    let hote = requete.headers().get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("").to_string();
    let suite_url = requete.uri().path_and_query().map(|p| p.as_str().to_string()).unwrap_or_else(|| "/".into());
    let get = requete.method() == axum::http::Method::GET;
    if let Some(redirection) = vers_https(&etat, &chemin, get, &hote, &suite_url).await {
        return redirection;
    }
    let libre = !chemin.starts_with("/api/")
        || matches!(chemin.as_str(), "/api/session" | "/api/connexion" | "/api/deconnexion" | "/api/google/connexion" | "/api/google/retour");
    if libre || session(&etat, requete.headers()).await.is_some() {
        let mut reponse = suite.run(requete).await;
        // L'interface (HTML, JS, CSS) est revalidée à chaque chargement : jamais une ancienne version après une mise à jour.
        if !chemin.starts_with("/api/") {
            reponse.headers_mut().insert(header::CACHE_CONTROL, axum::http::HeaderValue::from_static("no-cache"));
        }
        reponse
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
