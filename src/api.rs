//! API HTTP de l'interface.

use std::collections::HashMap;
use std::convert::Infallible;

use anyhow::anyhow;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{middleware, Json, Router};
use futures_util::{Stream, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::services::{ServeDir, ServeFile};

use crate::acces;
use crate::decoupe;
use crate::etat::EtatPartage;
use crate::llm;
use crate::modele::{maintenant, nouvel_id, Decision, Echange, EtatRush, Incrustation, Media, Montage, Projet, Rush};
use crate::reglages;
use crate::rendu;
use crate::travaux::Genre;

pub struct Erreur(StatusCode, String);

impl IntoResponse for Erreur {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"erreur": self.1}))).into_response()
    }
}

impl From<anyhow::Error> for Erreur {
    fn from(e: anyhow::Error) -> Self {
        Erreur(StatusCode::BAD_REQUEST, format!("{e:#}"))
    }
}

fn refus(message: impl Into<String>) -> Erreur {
    Erreur(StatusCode::BAD_REQUEST, message.into())
}

type Reponse = Result<Json<Value>, Erreur>;

pub fn routes(etat: EtatPartage) -> Router {
    let web = etat.cfg.racine.join("web");
    let televersement = Router::new()
        .route("/api/projets/{id}/rushs", put(televerser_rush))
        .route("/api/projets/{id}/medias", put(televerser_media))
        .layer(DefaultBodyLimit::disable());
    Router::new()
        .route("/api/session", get(acces::session))
        .route("/api/connexion", post(acces::connexion))
        .route("/api/catalogue", get(catalogue))
        .route("/api/etat", get(etat_general))
        .route("/api/projets", get(liste_projets).post(creer_projet))
        .route("/api/projets/{id}", get(voir_projet).delete(supprimer_projet).patch(renommer_projet))
        .route("/api/projets/{id}/rushs/ordre", post(ordonner_rushs))
        .route("/api/projets/{id}/rushs/{rid}", delete(supprimer_rush))
        .route("/api/projets/{id}/rushs/{rid}/relancer", post(relancer_rush))
        .route("/api/projets/{id}/medias/{mid}", get(fichier_media).delete(supprimer_media))
        .route("/api/projets/{id}/reglages", post(changer_reglages))
        .route("/api/projets/{id}/phrase", post(changer_phrase))
        .route("/api/projets/{id}/incrustations", post(ajouter_incrustation))
        .route("/api/projets/{id}/incrustations/{iid}", delete(retirer_incrustation))
        .route("/api/projets/{id}/monter", post(monter))
        .route("/api/projets/{id}/apercu", post(apercu))
        .route("/api/projets/{id}/rendu", post(rendu_final))
        .route("/api/projets/{id}/discussion", post(discuter))
        .route("/api/projets/{id}/sorties/{nom}", get(fichier_sortie))
        .route("/api/projets/{id}/evenements", get(evenements))
        .merge(televersement)
        .fallback_service(ServeDir::new(&web).fallback(ServeFile::new(web.join("index.html"))))
        .layer(middleware::from_fn_with_state(etat.clone(), acces::garde))
        .with_state(etat)
}

async fn catalogue() -> Json<Value> {
    Json(reglages::catalogue_json())
}

async fn etat_general(State(etat): State<EtatPartage>) -> Json<Value> {
    Json(json!({
        "file": etat.file.visibles(),
        "llm": etat.carte.etat_llm().await,
        "modele": etat.cfg.modele_llm,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn liste_projets(State(etat): State<EtatPartage>) -> Json<Value> {
    let projets = etat.projets.lock().await;
    let mut v: Vec<&Projet> = projets.values().collect();
    v.sort_by(|a, b| b.cree.cmp(&a.cree));
    Json(json!(v.iter().map(|p| json!({"id": p.id, "nom": p.nom, "cree": p.cree, "rushs": p.rushs.len(),
        "monte": p.montage.is_some(), "rendu": p.rendu.is_some()})).collect::<Vec<_>>()))
}

#[derive(Deserialize)]
struct Nom {
    nom: String,
}

async fn creer_projet(State(etat): State<EtatPartage>, Json(n): Json<Nom>) -> Reponse {
    let nom = n.nom.trim();
    let p = Projet {
        id: nouvel_id(),
        nom: if nom.is_empty() { "Nouveau projet".into() } else { nom.chars().take(80).collect() },
        cree: maintenant(),
        rushs: vec![],
        medias: vec![],
        reglages: reglages::defauts(),
        montage: None,
        incrustations: vec![],
        discussion: vec![],
        apercu: None,
        rendu: None,
    };
    etat.enregistrer(&p).await?;
    let id = p.id.clone();
    etat.projets.lock().await.insert(id.clone(), p);
    Ok(Json(json!({"id": id})))
}

async fn renommer_projet(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(n): Json<Nom>) -> Reponse {
    let nom: String = n.nom.trim().chars().take(80).collect();
    if nom.is_empty() {
        return Err(refus("le nom est vide"));
    }
    etat.modifier(&id, |p| p.nom = nom).await?;
    Ok(Json(json!({})))
}

async fn supprimer_projet(State(etat): State<EtatPartage>, Path(id): Path<String>) -> Reponse {
    if etat.file.occupe(&id) {
        return Err(refus("un travail est en cours sur ce projet : attendez sa fin"));
    }
    etat.projets.lock().await.remove(&id).ok_or_else(|| anyhow!("projet introuvable"))?;
    let _ = tokio::fs::remove_dir_all(etat.dossier_projet(&id)).await;
    Ok(Json(json!({})))
}

async fn voir_projet(State(etat): State<EtatPartage>, Path(id): Path<String>) -> Reponse {
    let p = etat.projet(&id).await?;
    let analyses = etat.analyses_du_projet(&p).await.unwrap_or_default();
    let ps = decoupe::phrases(&analyses);
    let decisions: HashMap<&str, &Decision> =
        p.montage.as_ref().map(|m| m.phrases.iter().map(|d| (d.cle.as_str(), d)).collect()).unwrap_or_default();
    let phrases: Vec<Value> = ps
        .iter()
        .enumerate()
        .map(|(i, ph)| json!({"n": i, "cle": ph.cle, "rush": ph.rush, "t0": ph.t0, "t1": ph.t1, "texte": ph.texte,
                              "decision": decisions.get(ph.cle.as_str())}))
        .collect();
    let tous_prets = !p.rushs.is_empty() && p.rushs.iter().all(|r| r.etat == EtatRush::Pret);
    Ok(Json(json!({
        "projet": p,
        "phrases": phrases,
        "pret_a_monter": tous_prets,
        "occupe": etat.file.occupe(&id),
    })))
}

#[derive(Deserialize)]
struct NomFichier {
    nom: String,
}

fn extension(nom: &str) -> String {
    nom.rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_else(|| "bin".into())
}

/// Écrit le corps de la requête dans un fichier, au fil de l'eau.
async fn ecrire_corps(corps: Body, chemin: &std::path::Path) -> anyhow::Result<u64> {
    tokio::fs::create_dir_all(chemin.parent().unwrap()).await?;
    let mut f = tokio::fs::File::create(chemin).await?;
    let mut flux = corps.into_data_stream();
    let mut taille = 0u64;
    while let Some(morceau) = flux.next().await {
        let morceau = morceau.map_err(|e| anyhow!("téléversement interrompu : {e}"))?;
        taille += morceau.len() as u64;
        f.write_all(&morceau).await?;
    }
    f.flush().await?;
    Ok(taille)
}

async fn televerser_rush(State(etat): State<EtatPartage>, Path(id): Path<String>, Query(q): Query<NomFichier>,
                         requete: Request) -> Reponse {
    etat.projet(&id).await?;
    let rid = nouvel_id();
    let fichier = format!("{rid}.{}", extension(&q.nom));
    let chemin = etat.dossier_projet(&id).join("rushs").join(&fichier);
    let taille = match ecrire_corps(requete.into_body(), &chemin).await {
        Ok(t) => t,
        Err(e) => {
            let _ = tokio::fs::remove_file(&chemin).await;
            return Err(e.into());
        }
    };
    let rush = Rush { id: rid.clone(), nom: q.nom.chars().take(120).collect(), fichier, taille, etat: EtatRush::Televerse,
                      erreur: None, source: None };
    etat.modifier(&id, |p| p.rushs.push(rush)).await?;
    etat.file.ajouter(&etat, &id, Genre::Preparer(rid.clone()));
    Ok(Json(json!({"id": rid})))
}

#[derive(Deserialize)]
struct Ordre {
    ids: Vec<String>,
}

async fn ordonner_rushs(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(o): Json<Ordre>) -> Reponse {
    etat.modifier(&id, |p| {
        p.rushs.sort_by_key(|r| o.ids.iter().position(|x| x == &r.id).unwrap_or(usize::MAX));
    })
    .await?;
    Ok(Json(json!({})))
}

async fn supprimer_rush(State(etat): State<EtatPartage>, Path((id, rid)): Path<(String, String)>) -> Reponse {
    if etat.file.occupe(&id) {
        return Err(refus("un travail est en cours sur ce projet : attendez sa fin"));
    }
    let fichier = etat
        .modifier(&id, |p| {
            let f = p.rushs.iter().find(|r| r.id == rid).map(|r| r.fichier.clone());
            p.rushs.retain(|r| r.id != rid);
            f
        })
        .await?;
    if let Some(f) = fichier {
        let _ = tokio::fs::remove_file(etat.dossier_projet(&id).join("rushs").join(f)).await;
    }
    let _ = tokio::fs::remove_dir_all(etat.dossier_travail(&id, &rid)).await;
    etat.oublier_analyse(&id, &rid).await;
    Ok(Json(json!({})))
}

async fn relancer_rush(State(etat): State<EtatPartage>, Path((id, rid)): Path<(String, String)>) -> Reponse {
    let p = etat.projet(&id).await?;
    let r = p.rushs.iter().find(|r| r.id == rid).ok_or_else(|| anyhow!("rush introuvable"))?;
    let prepare = etat.dossier_travail(&id, &rid).join("mezz.mkv").exists() && r.source.is_some();
    etat.file.ajouter(&etat, &id, if prepare { Genre::Analyser(rid) } else { Genre::Preparer(rid) });
    Ok(Json(json!({})))
}

async fn televerser_media(State(etat): State<EtatPartage>, Path(id): Path<String>, Query(q): Query<NomFichier>,
                          requete: Request) -> Reponse {
    etat.projet(&id).await?;
    let mid = nouvel_id();
    let fichier = format!("{mid}.{}", extension(&q.nom));
    let chemin = etat.dossier_projet(&id).join("medias").join(&fichier);
    ecrire_corps(requete.into_body(), &chemin).await?;
    let (genre, duree, a_son) = match rendu::sonder_media(&chemin, &q.nom).await {
        Ok(v) => v,
        Err(e) => {
            let _ = tokio::fs::remove_file(&chemin).await;
            return Err(refus(format!("média refusé : {e}")));
        }
    };
    let media = Media { id: mid.clone(), nom: q.nom.chars().take(120).collect(), fichier, genre, duree, a_son };
    etat.modifier(&id, |p| p.medias.push(media)).await?;
    Ok(Json(json!({"id": mid})))
}

async fn fichier_media(State(etat): State<EtatPartage>, Path((id, mid)): Path<(String, String)>, requete: Request)
                       -> Result<Response, Erreur> {
    let p = etat.projet(&id).await?;
    let m = p.medias.iter().find(|m| m.id == mid).ok_or_else(|| anyhow!("média introuvable"))?;
    let chemin = etat.dossier_projet(&id).join("medias").join(&m.fichier);
    Ok(ServeFile::new(chemin).try_call(requete).await.map_err(|e| anyhow!("{e}"))?.into_response())
}

async fn supprimer_media(State(etat): State<EtatPartage>, Path((id, mid)): Path<(String, String)>) -> Reponse {
    let fichier = etat
        .modifier(&id, |p| {
            let f = p.medias.iter().find(|m| m.id == mid).map(|m| m.fichier.clone());
            p.medias.retain(|m| m.id != mid);
            p.incrustations.retain(|i| i.media != mid);
            if reglages::texte(&p.reglages, "musique") == mid {
                p.reglages.insert("musique".into(), json!(""));
            }
            f
        })
        .await?;
    if let Some(f) = fichier {
        let _ = tokio::fs::remove_file(etat.dossier_projet(&id).join("medias").join(f)).await;
    }
    Ok(Json(json!({})))
}

/// Applique des réglages ; renvoie les libellés de ce qui a changé.
fn appliquer_reglages(p: &mut Projet, changements: &serde_json::Map<String, Value>) -> (Vec<String>, Vec<String>) {
    let mut faits = Vec::new();
    let mut refus = Vec::new();
    for (cle, valeur) in changements {
        let Some(r) = reglages::chercher(cle) else { continue };
        let mut valeur = valeur.clone();
        if matches!(r.genre, reglages::Genre::MediaSon) {
            // Le LLM donne un nom, l'interface un identifiant.
            let v = valeur.as_str().unwrap_or_default().to_string();
            let trouve = p.medias.iter().find(|m| m.genre == "son" && (m.id == v || m.nom.eq_ignore_ascii_case(&v)));
            valeur = match (v.is_empty(), trouve) {
                (true, _) => json!(""),
                (false, Some(m)) => json!(m.id),
                (false, None) => {
                    refus.push(format!("pas de son « {v} » dans la médiathèque"));
                    continue;
                }
            };
        }
        match reglages::valider(cle, &valeur) {
            Ok(v) if p.reglages.get(cle) != Some(&v) => {
                let libelle = match &r.genre {
                    reglages::Genre::Choix(o) => o.iter().find(|(k, _)| Some(*k) == v.as_str()).map(|(_, l)| l.to_string()).unwrap_or_default(),
                    reglages::Genre::Interrupteur => if v.as_bool() == Some(true) { "oui".into() } else { "non".into() },
                    reglages::Genre::Curseur { .. } => format!("{} %", v.as_f64().unwrap_or(0.0).round()),
                    reglages::Genre::MediaSon => p.medias.iter().find(|m| Some(m.id.as_str()) == v.as_str()).map(|m| m.nom.clone())
                        .unwrap_or_else(|| "aucune".into()),
                };
                faits.push(format!("{} → {libelle}", r.titre));
                p.reglages.insert(cle.clone(), v);
            }
            Ok(_) => {}
            Err(e) => refus.push(e),
        }
    }
    (faits, refus)
}

async fn changer_reglages(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(v): Json<Value>) -> Reponse {
    let changements = v.as_object().cloned().unwrap_or_default();
    let (faits, refus_) = etat.modifier(&id, |p| appliquer_reglages(p, &changements)).await?;
    if !refus_.is_empty() && faits.is_empty() {
        return Err(refus(refus_.join(" ; ")));
    }
    Ok(Json(json!({"faits": faits, "refus": refus_})))
}

#[derive(Deserialize)]
struct ChangementPhrase {
    cle: String,
    garder: Option<bool>,
    regime: Option<String>,
    cadre: Option<String>,
}

fn montage_par_defaut(ps: &[crate::modele::Phrase]) -> Montage {
    Montage {
        cree: maintenant(),
        phrases: ps.iter().map(|p| Decision { cle: p.cle.clone(), garder: true, regime: "rafale".into(), cadre: "moyen".into(),
                                               transition: None, mots_forts: vec![] }).collect(),
        accroche: vec![],
        chapitres: vec![],
        notes: vec![],
    }
}

fn changer(p: &mut Projet, ps: &[crate::modele::Phrase], c: &ChangementPhrase) -> Option<String> {
    let m = p.montage.get_or_insert_with(|| montage_par_defaut(ps));
    if !m.phrases.iter().any(|d| d.cle == c.cle) {
        m.phrases.push(Decision { cle: c.cle.clone(), garder: true, regime: "rafale".into(), cadre: "moyen".into(),
                                  transition: None, mots_forts: vec![] });
    }
    let d = m.phrases.iter_mut().find(|d| d.cle == c.cle)?;
    let mut faits = Vec::new();
    if let Some(g) = c.garder {
        if d.garder != g {
            d.garder = g;
            faits.push(if g { "rétablie" } else { "coupée" }.to_string());
        }
    }
    if let Some(r) = c.regime.as_ref().filter(|r| ["respiration", "rafale", "impact"].contains(&r.as_str())) {
        if &d.regime != r {
            d.regime = r.clone();
            faits.push(format!("régime {r}"));
        }
    }
    if let Some(k) = c.cadre.as_ref().filter(|k| ["large", "moyen", "visage", "yeux"].contains(&k.as_str())) {
        if &d.cadre != k {
            d.cadre = k.clone();
            faits.push(format!("cadre {k}"));
        }
    }
    (!faits.is_empty()).then(|| faits.join(", "))
}

async fn changer_phrase(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(c): Json<ChangementPhrase>) -> Reponse {
    let p = etat.projet(&id).await?;
    let ps = decoupe::phrases(&etat.analyses_du_projet(&p).await?);
    if !ps.iter().any(|ph| ph.cle == c.cle) {
        return Err(refus("phrase introuvable"));
    }
    let fait = etat.modifier(&id, |p| changer(p, &ps, &c)).await?;
    Ok(Json(json!({"fait": fait})))
}

#[derive(Deserialize)]
struct NouvelleIncrustation {
    #[serde(default)]
    media: String,
    #[serde(default)]
    effet: String,
    phrase: String,
    #[serde(default)]
    decalage: f64,
    duree: f64,
    position: String,
    taille: f64,
    #[serde(default)]
    fond_vert: bool,
    #[serde(default)]
    son: bool,
}

fn valider_incrustation(p: &Projet, n: NouvelleIncrustation) -> Result<Incrustation, String> {
    if n.effet.is_empty() && !p.medias.iter().any(|m| m.id == n.media && m.genre != "son") {
        return Err("choisissez une image, un GIF ou une vidéo de la médiathèque".into());
    }
    if !["", "secousse", "noir_blanc"].contains(&n.effet.as_str()) {
        return Err("effet inconnu".into());
    }
    let positions = ["plein", "centre", "haut_gauche", "haut_droite", "bas_gauche", "bas_droite"];
    Ok(Incrustation {
        id: nouvel_id(),
        media: if n.effet.is_empty() { n.media } else { String::new() },
        effet: n.effet,
        phrase: n.phrase,
        decalage: n.decalage.max(0.0),
        duree: n.duree.clamp(0.2, 30.0),
        position: if positions.contains(&n.position.as_str()) { n.position } else { "centre".into() },
        taille: n.taille.clamp(0.1, 1.0),
        fond_vert: n.fond_vert,
        son: n.son,
    })
}

async fn ajouter_incrustation(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(n): Json<NouvelleIncrustation>)
                              -> Reponse {
    let p = etat.projet(&id).await?;
    let inc = valider_incrustation(&p, n).map_err(refus)?;
    let iid = inc.id.clone();
    etat.modifier(&id, |p| p.incrustations.push(inc)).await?;
    Ok(Json(json!({"id": iid})))
}

async fn retirer_incrustation(State(etat): State<EtatPartage>, Path((id, iid)): Path<(String, String)>) -> Reponse {
    etat.modifier(&id, |p| p.incrustations.retain(|i| i.id != iid)).await?;
    Ok(Json(json!({})))
}

async fn monter(State(etat): State<EtatPartage>, Path(id): Path<String>) -> Reponse {
    let p = etat.projet(&id).await?;
    if p.rushs.is_empty() || p.rushs.iter().any(|r| r.etat != EtatRush::Pret) {
        return Err(refus("attendez que tous les rushs soient analysés"));
    }
    etat.file.ajouter(&etat, &id, Genre::Monter);
    Ok(Json(json!({})))
}

async fn apercu(State(etat): State<EtatPartage>, Path(id): Path<String>) -> Reponse {
    let p = etat.projet(&id).await?;
    if !p.rushs.iter().any(|r| r.etat == EtatRush::Pret) {
        return Err(refus("aucun rush analysé"));
    }
    etat.file.ajouter(&etat, &id, Genre::Apercu);
    Ok(Json(json!({})))
}

async fn rendu_final(State(etat): State<EtatPartage>, Path(id): Path<String>) -> Reponse {
    let p = etat.projet(&id).await?;
    if p.montage.is_none() {
        return Err(refus("proposez d'abord un montage"));
    }
    etat.file.ajouter(&etat, &id, Genre::Rendu);
    Ok(Json(json!({})))
}

#[derive(Deserialize)]
struct Message {
    message: String,
}

async fn discuter(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(m): Json<Message>) -> Reponse {
    let message = m.message.trim().to_string();
    if message.is_empty() {
        return Err(refus("message vide"));
    }
    let texte_moi = message.clone();
    etat.modifier(&id, |p| p.discussion.push(Echange { role: "moi".into(), texte: texte_moi, quand: maintenant(), actions: vec![] }))
        .await?;
    let p = etat.projet(&id).await?;
    let analyses = etat.analyses_du_projet(&p).await.unwrap_or_default();
    let ps = decoupe::phrases(&analyses);
    let v = match llm::discuter(&etat, &p, &ps, &message).await {
        Ok(v) => v,
        Err(e) => {
            let texte = format!("Je n'ai pas pu répondre : {e:#}");
            etat.modifier(&id, |p| p.discussion.push(Echange { role: "edivid".into(), texte: texte.clone(), quand: maintenant(),
                                                                 actions: vec![] })).await?;
            return Err(refus(texte));
        }
    };
    let cle_de = |n: &Value| n.as_u64().and_then(|n| ps.get(n as usize)).map(|ph| ph.cle.clone());
    let mut reglages_llm = v["reglages"].as_object().cloned().unwrap_or_default();
    // Un son posé en incrustation devient la musique de fond : c'est ce que la personne voulait.
    for i in v["ajouter_incrustations"].as_array().into_iter().flatten() {
        let nom = i["media"].as_str().unwrap_or("");
        if p.medias.iter().any(|m| m.genre == "son" && (m.nom.eq_ignore_ascii_case(nom) || m.id == nom)) {
            reglages_llm.insert("musique".into(), json!(nom));
        }
    }
    let changements: Vec<ChangementPhrase> = v["phrases"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| Some(ChangementPhrase {
            cle: cle_de(&c["id"])?,
            garder: c["garder"].as_bool(),
            regime: c["regime"].as_str().map(String::from),
            cadre: c["cadre"].as_str().map(String::from),
        }))
        .collect();
    let nouvelles: Vec<(NouvelleIncrustation, String)> = v["ajouter_incrustations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| {
            let effet = i["effet"].as_str().unwrap_or("").to_string();
            let nom_media = i["media"].as_str().unwrap_or("").to_string();
            let trouve = p.medias.iter().find(|m| m.nom.eq_ignore_ascii_case(&nom_media) || m.id == nom_media);
            if trouve.map(|m| m.genre == "son").unwrap_or(false) {
                return None;
            }
            let media = trouve.map(|m| m.id.clone()).unwrap_or_default();
            Some((NouvelleIncrustation {
                media,
                effet: effet.clone(),
                phrase: cle_de(&i["phrase"])?,
                decalage: 0.0,
                duree: i["duree"].as_f64().unwrap_or(2.0),
                position: i["position"].as_str().unwrap_or("centre").to_string(),
                taille: i["taille"].as_f64().unwrap_or(0.4),
                fond_vert: i["fond_vert"].as_bool().unwrap_or(false),
                son: i["son"].as_bool().unwrap_or(false),
            }, if effet.is_empty() { nom_media } else { effet }))
        })
        .collect();
    let a_retirer: Vec<String> = v["retirer_incrustations"].as_array().into_iter().flatten()
        .filter_map(|x| x.as_str().map(String::from)).collect();
    let remonter = v["remonter"].as_bool().unwrap_or(false);
    let reponse_llm = v["reponse"].as_str().unwrap_or("C'est noté.").to_string();
    let rang: HashMap<String, usize> = ps.iter().enumerate().map(|(i, ph)| (ph.cle.clone(), i)).collect();

    let ps2 = ps.clone();
    let actions = etat
        .modifier(&id, move |p| {
            let mut actions = Vec::new();
            let (faits, refus_) = appliquer_reglages(p, &reglages_llm);
            actions.extend(faits);
            actions.extend(refus_.into_iter().map(|r| format!("non appliqué : {r}")));
            for c in &changements {
                if let Some(f) = changer(p, &ps2, c) {
                    actions.push(format!("Phrase {} : {f}", rang.get(&c.cle).copied().unwrap_or(0)));
                }
            }
            for (n, nom) in nouvelles {
                let numero = rang.get(&n.phrase).copied().unwrap_or(0);
                match valider_incrustation(p, n) {
                    Ok(inc) => {
                        actions.push(format!("Incrustation « {nom} » sur la phrase {numero}"));
                        p.incrustations.push(inc);
                    }
                    Err(e) => actions.push(format!("non appliqué : {e}")),
                }
            }
            let avant = p.incrustations.len();
            p.incrustations.retain(|i| !a_retirer.contains(&i.id));
            if p.incrustations.len() < avant {
                actions.push(format!("{} incrustation(s) retirée(s)", avant - p.incrustations.len()));
            }
            if remonter {
                actions.push("Nouveau montage complet par le LLM".into());
            }
            let echecs: Vec<&str> = actions.iter().filter_map(|a| a.strip_prefix("non appliqué : ")).collect();
            let reponse = if echecs.is_empty() {
                reponse_llm
            } else {
                format!("{reponse_llm}\n\nAttention, je n'ai pas pu tout faire : {}.", echecs.join(" ; "))
            };
            p.discussion.push(Echange { role: "edivid".into(), texte: reponse.clone(), quand: maintenant(), actions: actions.clone() });
            (actions, reponse)
        })
        .await?;
    let (actions, reponse) = actions;
    // Règle de l'orchestrateur : tout changement appliqué donne un nouvel aperçu.
    let tous_prets = p.rushs.iter().all(|r| r.etat == EtatRush::Pret) && !p.rushs.is_empty();
    if remonter && tous_prets {
        etat.file.ajouter(&etat, &id, Genre::Monter);
    } else if actions.iter().any(|a| !a.starts_with("non appliqué")) && tous_prets {
        etat.file.ajouter(&etat, &id, Genre::Apercu);
    }
    Ok(Json(json!({"reponse": reponse, "actions": actions})))
}

async fn fichier_sortie(State(etat): State<EtatPartage>, Path((id, nom)): Path<(String, String)>, requete: Request)
                        -> Result<Response, Erreur> {
    if nom.contains('/') || nom.contains("..") {
        return Err(refus("nom invalide"));
    }
    let chemin = etat.dossier_projet(&id).join("sorties").join(&nom);
    if !chemin.exists() {
        return Err(Erreur(StatusCode::NOT_FOUND, "fichier introuvable".into()));
    }
    Ok(ServeFile::new(chemin).try_call(requete).await.map_err(|e| anyhow!("{e}"))?.into_response())
}

async fn evenements(State(etat): State<EtatPartage>, Path(id): Path<String>)
                    -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let flux = BroadcastStream::new(etat.evenements.subscribe()).filter_map(move |e| {
        let id = id.clone();
        async move {
            let e = e.ok()?;
            (e.projet == id || e.genre == "file").then(|| Ok(Event::default().event(e.genre.clone()).data(
                json!({"projet": e.projet, "donnees": e.donnees}).to_string())))
        }
    });
    Sse::new(flux).keep_alive(KeepAlive::default())
}
