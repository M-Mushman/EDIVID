//! Accès : appareil du tailnet reconnu par Tailscale (sans rien saisir), sinon code d'accès (demandé une
//! fois par navigateur) ou compte Google autorisé. Les ACL du tailnet restent la première porte.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::etat::EtatPartage;

/// Écoute par laquelle arrive la requête : seule celle réservée au relais HTTPS (`tailscale serve`) fait foi pour
/// l'identité du tailnet. Avant, toute connexion locale l'était, donc tout programme local pouvait l'écrire.
#[derive(Clone, Copy)]
pub struct Ecoute {
    pub relais: bool,
}

/// En-tête exigé sur toute requête qui modifie (POST, PUT, DELETE…) : un site tiers ne peut pas le poser sans
/// l'accord du serveur (pré-vol CORS jamais accordé), ce qui ferme les attaques par requête forgée (CSRF).
pub const ENTETE_CSRF: &str = "x-edivid";

const ESSAIS_MAX: u32 = 10;
const FENETRE_ESSAIS: Duration = Duration::from_secs(15 * 60);
static ECHECS: Mutex<Option<HashMap<IpAddr, (u32, Instant)>>> = Mutex::new(None);

/// Égalité en temps constant : la durée de la comparaison ne dit rien du secret.
fn egal(a: &str, b: &str) -> bool {
    a.len() == b.len() && bool::from(a.as_bytes().ct_eq(b.as_bytes()))
}

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
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepte toute clé");
    mac.update(format!("{email}|{expire}").as_bytes());
    mac.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect()
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
    format!("{TEMOIN}={valeur}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={DUREE_SESSION_GOOGLE_S}")
}

pub async fn session(etat: &EtatPartage, entetes: &HeaderMap) -> Option<Session> {
    for v in temoins(entetes) {
        if egal(&v, &etat.cfg.jeton) {
            return Some(Session { email: None });
        }
        let morceaux: Vec<&str> = v.split('.').collect();
        if let ["g", email_hex, expire, sig] = morceaux.as_slice() {
            let (Some(email), Ok(expire)) = (dehex(email_hex), expire.parse::<i64>()) else { continue };
            let autorise = etat.parametres.read().await.comptes.iter().any(|c| c.email.eq_ignore_ascii_case(&email));
            if autorise && expire > chrono::Utc::now().timestamp() && egal(&signature(&etat.cfg.secret, &email, expire), sig) {
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
        .map(|code| egal(&code.trim().to_uppercase(), &etat.cfg.code_acces.to_uppercase()))
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

/// Un segment de chemin, décodé, qui sortirait de son dossier (« .. », barre, octet nul) : refusé partout.
fn chemin_sur(chemin: &str) -> bool {
    chemin.split('/').all(|segment| {
        let d = decoder(segment);
        d != "." && d != ".." && !d.contains(['/', '\\', '\0'])
    })
}

fn decoder(s: &str) -> String {
    let o = s.as_bytes();
    let mut sortie = Vec::with_capacity(o.len());
    let mut i = 0;
    while i < o.len() {
        let chiffre = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
        if o[i] == b'%' && i + 2 < o.len() {
            if let (Some(a), Some(b)) = (chiffre(o[i + 1]), chiffre(o[i + 2])) {
                sortie.push(a * 16 + b);
                i += 3;
                continue;
            }
        }
        sortie.push(o[i]);
        i += 1;
    }
    String::from_utf8_lossy(&sortie).into_owned()
}

/// Requête qui modifie : en-tête anti-CSRF présent, et origine (si le navigateur la donne) égale à l'hôte.
fn requete_legitime(requete: &Request) -> bool {
    use axum::http::Method;
    if matches!(*requete.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        return true;
    }
    let h = requete.headers();
    if h.get(ENTETE_CSRF).and_then(|v| v.to_str().ok()) != Some("1") {
        return false;
    }
    match (h.get(header::ORIGIN).and_then(|v| v.to_str().ok()), h.get(header::HOST).and_then(|v| v.to_str().ok())) {
        (Some(origine), Some(hote)) => origine == format!("https://{hote}") || origine == format!("http://{hote}"),
        _ => true,
    }
}

pub async fn garde(State(etat): State<EtatPartage>, mut requete: Request, suite: Next) -> Response {
    if !chemin_sur(requete.uri().path()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"erreur": "chemin invalide"}))).into_response();
    }
    // L'identité du tailnet n'est crue que par l'écoute réservée au relais HTTPS, en local.
    let par_relais = requete.extensions().get::<Ecoute>().is_some_and(|e| e.relais)
        && requete.extensions().get::<ConnectInfo<SocketAddr>>().is_some_and(|c| c.0.ip().is_loopback());
    if !par_relais {
        requete.headers_mut().remove(IDENTITE_TAILNET);
    }
    if requete.uri().path().starts_with("/api/") && !requete_legitime(&requete) {
        return (StatusCode::FORBIDDEN, Json(json!({"erreur": "requête refusée (origine ou en-tête manquant)"}))).into_response();
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

pub async fn connexion(State(etat): State<EtatPartage>, ConnectInfo(client): ConnectInfo<SocketAddr>, Json(c): Json<Connexion>) -> Response {
    let ip = client.ip();
    let bloque = {
        let mut g = ECHECS.lock().unwrap_or_else(|e| e.into_inner());
        let table = g.get_or_insert_with(HashMap::new);
        table.retain(|_, (_, depuis)| depuis.elapsed() < FENETRE_ESSAIS);
        table.get(&ip).is_some_and(|(n, _)| *n >= ESSAIS_MAX)
    };
    if bloque {
        return (StatusCode::TOO_MANY_REQUESTS, Json(json!({"erreur": "trop d'essais : réessayez dans un quart d'heure"}))).into_response();
    }
    let saisi: String = c.code.trim().to_uppercase().chars().filter(|c| !c.is_whitespace()).collect();
    if !egal(&saisi.replace('-', ""), &etat.cfg.code_acces.replace('-', "")) {
        {
            let mut g = ECHECS.lock().unwrap_or_else(|e| e.into_inner());
            let e = g.get_or_insert_with(HashMap::new).entry(ip).or_insert((0, Instant::now()));
            e.0 += 1;
        }
        tokio::time::sleep(Duration::from_millis(800)).await;
        return (StatusCode::UNAUTHORIZED, Json(json!({"erreur": "code incorrect"}))).into_response();
    }
    let temoin = format!("{TEMOIN}={}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=31536000", etat.cfg.jeton);
    ([(header::SET_COOKIE, temoin)], Json(json!({"connecte": true}))).into_response()
}

pub async fn deconnexion() -> Response {
    let temoin = format!("{TEMOIN}=; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0");
    ([(header::SET_COOKIE, temoin)], Json(json!({"connecte": false}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chemins() {
        assert!(chemin_sur("/api/projets/9c8892ad3890/sorties/rendu-20261027.mp4"));
        assert!(chemin_sur("/api/comptes/lea%40exemple.fr"));
        for mauvais in ["/api/jeux/..", "/api/jeux/%2E%2E", "/api/jeux/%2e%2E", "/api/jeux/..%2F..", "/api/x/a%5Cb", "/api/x/%00"] {
            assert!(!chemin_sur(mauvais), "{mauvais} accepté");
        }
    }

    #[test]
    fn signatures() {
        let s = signature("sel", "lea@exemple.fr", 42);
        assert_eq!(s.len(), 64);
        assert_ne!(s, signature("sel", "lea@exemple.fr", 43));
        assert!(egal(&s, &signature("sel", "lea@exemple.fr", 42)));
        assert!(!egal(&s, &s[..63]));
    }
}
