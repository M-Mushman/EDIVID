//! Code d'accès : demandé une fois par navigateur, puis gardé dans un témoin de session.
//! Les ACL du tailnet restent la première porte ; ce code est la seconde.

use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::etat::EtatPartage;

const TEMOIN: &str = "edivid_session";

pub fn connecte(etat: &EtatPartage, entetes: &HeaderMap) -> bool {
    let par_temoin = entetes
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .any(|(k, v)| k == TEMOIN && v == etat.cfg.jeton);
    let par_entete = entetes
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|code| code.trim().eq_ignore_ascii_case(&etat.cfg.code_acces))
        .unwrap_or(false);
    par_temoin || par_entete
}

pub async fn garde(State(etat): State<EtatPartage>, requete: Request, suite: Next) -> Response {
    let chemin = requete.uri().path();
    let libre = !chemin.starts_with("/api/") || chemin == "/api/session" || chemin == "/api/connexion";
    if libre || connecte(&etat, requete.headers()) {
        suite.run(requete).await
    } else {
        (StatusCode::UNAUTHORIZED, Json(json!({"erreur": "code d'accès requis"}))).into_response()
    }
}

#[derive(Deserialize)]
pub struct Connexion {
    code: String,
}

pub async fn session(State(etat): State<EtatPartage>, entetes: HeaderMap) -> Json<serde_json::Value> {
    Json(json!({"connecte": connecte(&etat, &entetes)}))
}

pub async fn connexion(State(etat): State<EtatPartage>, Json(c): Json<Connexion>) -> Response {
    let saisi: String = c.code.trim().to_uppercase().chars().filter(|c| !c.is_whitespace()).collect();
    if saisi.replace('-', "") != etat.cfg.code_acces.replace('-', "") {
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        return (StatusCode::UNAUTHORIZED, Json(json!({"erreur": "code incorrect"}))).into_response();
    }
    let temoin = format!("{TEMOIN}={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=31536000", etat.cfg.jeton);
    ([(header::SET_COOKIE, temoin)], Json(json!({"connecte": true}))).into_response()
}
