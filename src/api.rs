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
use crate::bibliotheque;
use crate::courriel;
use crate::decoupe;
use crate::drive;
use crate::google;
use crate::parametres;
use crate::postprod::{self, PostProd};
use crate::youtube;
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
        .route("/api/session", get(acces::voir_session))
        .route("/api/connexion", post(acces::connexion))
        .route("/api/deconnexion", post(acces::deconnexion))
        .route("/api/google/connexion", get(google::aller))
        .route("/api/google/retour", get(google::revenir))
        .route("/api/comptes", post(changer_compte))
        .route("/api/comptes/{email}", delete(retirer_compte))
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
        .route("/api/projets/{id}/incrustations/{iid}", delete(retirer_incrustation).post(modifier_incrustation))
        .route("/api/projets/{id}/commentaire", post(commenter))
        .route("/api/projets/{id}/commentaire/{pid}/valider", post(valider_commentaire))
        .route("/api/projets/{id}/commentaire/{pid}", delete(annuler_commentaire))
        .route("/api/projets/{id}/programme", get(programme))
        .route("/api/projets/{id}/rushs/{rid}/video", get(video_origine))
        .route("/api/projets/{id}/monter", post(monter))
        .route("/api/projets/{id}/apercu", post(apercu))
        .route("/api/projets/{id}/rendu", post(rendu_final))
        .route("/api/projets/{id}/discussion", post(discuter))
        .route("/api/projets/{id}/sorties/{nom}", get(fichier_sortie))
        .route("/api/projets/{id}/evenements", get(evenements))
        .route("/api/projets/{id}/consignes", post(changer_consignes))
        .route("/api/projets/{id}/coulisses", get(coulisses))
        .route("/api/projets/{id}/drive", post(envoyer_drive))
        .route("/api/parametres", get(voir_parametres).post(changer_parametres))
        .route("/api/parametres/courriel/essai", post(essai_courriel))
        .route("/api/parametres/drive/verifier", post(verifier_drive))
        .route("/api/parametres/drive/maintenant", post(passage_drive))
        .route("/api/postprod/looks", get(looks))
        .route("/api/projets/{id}/postprod", post(changer_postprod))
        .route("/api/projets/{id}/postprod/image", get(image_postprod))
        .route("/api/projets/{id}/correction", post(corriger_phrase))
        .route("/api/projets/{id}/remplacer", post(remplacer_partout))
        .route("/api/projets/{id}/export", post(lancer_export))
        .route("/api/projets/{id}/exports/{eid}", delete(retirer_export))
        .route("/api/projets/{id}/youtube/proposer", post(proposer_youtube))
        .route("/api/projets/{id}/youtube", post(publier_youtube))
        .route("/api/bibliotheque", get(voir_bibliotheque).post(ajouter_demande))
        .route("/api/bibliotheque/{did}", delete(retirer_demande))
        .merge(televersement)
        .nest_service("/polices", ServeDir::new(etat.cfg.racine.join("modeles").join("polices")))
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
        "monte": p.montage.is_some(), "rendu": p.rendu.is_some(), "origine": p.origine,
        "drive": p.depot_drive.is_some()})).collect::<Vec<_>>()))
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
        consignes: String::new(),
        auto: None,
        notifier_montage: false,
        depot_drive: None,
        origine: String::new(),
        compte: String::new(),
        postprod: Default::default(),
        corrections: Default::default(),
        exports: vec![],
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
    let ps = decoupe::phrases_projet(&p, &analyses);
    let decisions: HashMap<&str, &Decision> =
        p.montage.as_ref().map(|m| m.phrases.iter().map(|d| (d.cle.as_str(), d)).collect()).unwrap_or_default();
    let phrases: Vec<Value> = ps
        .iter()
        .enumerate()
        .map(|(i, ph)| json!({"n": i, "cle": ph.cle, "rush": ph.rush, "t0": ph.t0, "t1": ph.t1, "texte": ph.texte,
                              "decision": decisions.get(ph.cle.as_str())}))
        .collect();
    let tous_prets = !p.rushs.is_empty() && p.rushs.iter().all(|r| r.etat == EtatRush::Pret);
    let proxys: Vec<&str> = p.rushs.iter().filter(|r| etat.dossier_travail(&id, &r.id).join("proxy.mp4").exists())
        .map(|r| r.id.as_str()).collect();
    Ok(Json(json!({
        "proxys": proxys,
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
                    reglages::Genre::Curseur { unite, .. } => format!("{} {unite}", v.as_f64().unwrap_or(0.0).round()),
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
    } else if d.regime == "impact" && (d.cadre == "large" || d.cadre == "moyen") {
        // Règle du monteur : un impact se joue serré.
        d.cadre = "visage".into();
        faits.push("cadre visage".into());
    }
    (!faits.is_empty()).then(|| faits.join(", "))
}

async fn changer_phrase(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(c): Json<ChangementPhrase>) -> Reponse {
    let p = etat.projet(&id).await?;
    let ps = decoupe::phrases_projet(&p, &etat.analyses_du_projet(&p).await?);
    if !ps.iter().any(|ph| ph.cle == c.cle) {
        return Err(refus("phrase introuvable"));
    }
    let fait = etat.modifier(&id, |p| changer(p, &ps, &c)).await?;
    Ok(Json(json!({"fait": fait})))
}

#[derive(Deserialize, Default)]
struct NouvelleIncrustation {
    #[serde(default)]
    media: String,
    #[serde(default)]
    effet: String,
    #[serde(default)]
    texte: String,
    phrase: String,
    #[serde(default)]
    decalage: f64,
    duree: f64,
    #[serde(default)]
    position: String,
    taille: f64,
    #[serde(default)]
    fond_vert: bool,
    #[serde(default)]
    son: bool,
    #[serde(default)]
    couleur: String,
    #[serde(default)]
    fond: String,
    #[serde(default)]
    entree: String,
    #[serde(default)]
    x: Option<f64>,
    #[serde(default)]
    y: Option<f64>,
}

const ENTREES: &[&str] = &["", "aucune", "fondu", "splash", "glisse_gauche", "glisse_droite", "glisse_haut", "machine"];
const POSITIONS: &[&str] = &["plein", "centre", "haut", "bas", "haut_gauche", "haut_droite", "bas_gauche", "bas_droite"];

fn valider_incrustation(p: &Projet, n: NouvelleIncrustation) -> Result<Incrustation, String> {
    let texte: String = n.texte.trim().chars().take(300).collect();
    if texte.is_empty() && n.effet.is_empty() && !p.medias.iter().any(|m| m.id == n.media && m.genre != "son") {
        return Err("choisissez une image, un GIF ou une vidéo de la médiathèque, ou écrivez un texte".into());
    }
    if !["", "secousse", "noir_blanc"].contains(&n.effet.as_str()) {
        return Err("effet inconnu".into());
    }
    let est_texte = !texte.is_empty();
    Ok(Incrustation {
        id: nouvel_id(),
        media: if est_texte || !n.effet.is_empty() { String::new() } else { n.media },
        effet: if est_texte { String::new() } else { n.effet },
        phrase: n.phrase,
        decalage: n.decalage.max(0.0),
        duree: n.duree.clamp(0.2, 30.0),
        position: if POSITIONS.contains(&n.position.as_str()) { n.position } else { "centre".into() },
        taille: n.taille.clamp(0.1, 1.0),
        fond_vert: n.fond_vert,
        son: n.son,
        texte,
        couleur: if ["blanc", "jaune", "noir"].contains(&n.couleur.as_str()) { n.couleur } else { "blanc".into() },
        fond: if ["aucun", "boite", "bandeau"].contains(&n.fond.as_str()) { n.fond } else { "aucun".into() },
        entree: if ENTREES.contains(&n.entree.as_str()) { n.entree } else { String::new() },
        x: n.x.map(|v| v.clamp(0.0, 1.0)),
        y: n.y.map(|v| v.clamp(0.0, 1.0)),
    })
}

/// Déplacer, raccourcir, réécrire une incrustation (calque d'aperçu, module Texte).
async fn modifier_incrustation(State(etat): State<EtatPartage>, Path((id, iid)): Path<(String, String)>,
                               Json(v): Json<Value>) -> Reponse {
    let trouve = etat
        .modifier(&id, |p| {
            let Some(i) = p.incrustations.iter_mut().find(|i| i.id == iid) else { return false };
            if let (Some(x), Some(y)) = (v["x"].as_f64(), v["y"].as_f64()) {
                i.x = Some(x.clamp(0.0, 1.0));
                i.y = Some(y.clamp(0.0, 1.0));
            }
            if let Some(d) = v["duree"].as_f64() {
                i.duree = d.clamp(0.2, 30.0);
            }
            if let Some(d) = v["decalage"].as_f64() {
                i.decalage = d.max(0.0);
            }
            if let Some(t) = v["taille"].as_f64() {
                i.taille = t.clamp(0.1, 1.0);
            }
            if let Some(e) = v["entree"].as_str().filter(|e| ENTREES.contains(e)) {
                i.entree = e.to_string();
            }
            if let Some(t) = v["texte"].as_str().filter(|t| !t.trim().is_empty() && !i.texte.is_empty()) {
                i.texte = t.trim().chars().take(300).collect();
            }
            if let Some(c) = v["couleur"].as_str().filter(|c| ["blanc", "jaune", "noir"].contains(c)) {
                i.couleur = c.to_string();
            }
            if let Some(f) = v["fond"].as_str().filter(|f| ["aucun", "boite", "bandeau"].contains(f)) {
                i.fond = f.to_string();
            }
            true
        })
        .await?;
    if !trouve {
        return Err(refus("incrustation introuvable"));
    }
    Ok(Json(json!({})))
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

/// Applique les actions décrites par le LLM (conversation ou commentaire) ; renvoie ce qui a été fait.
fn appliquer_actions(p: &mut Projet, ps: &[crate::modele::Phrase], v: &Value) -> Vec<String> {
    let cle_de = |n: &Value| n.as_u64().and_then(|n| ps.get(n as usize)).map(|ph| ph.cle.clone());
    let rang: HashMap<String, usize> = ps.iter().enumerate().map(|(i, ph)| (ph.cle.clone(), i)).collect();
    let mut actions = Vec::new();
    let mut reglages_llm = v["reglages"].as_object().cloned().unwrap_or_default();
    // Un son posé en incrustation devient la musique de fond : c'est ce que la personne voulait.
    for i in v["ajouter_incrustations"].as_array().into_iter().flatten() {
        let nom = i["media"].as_str().unwrap_or("");
        if p.medias.iter().any(|m| m.genre == "son" && (m.nom.eq_ignore_ascii_case(nom) || m.id == nom)) {
            reglages_llm.insert("musique".into(), json!(nom));
        }
    }
    let (faits, refus_) = appliquer_reglages(p, &reglages_llm);
    actions.extend(faits);
    actions.extend(refus_.into_iter().map(|r| format!("non appliqué : {r}")));
    for c in v["phrases"].as_array().into_iter().flatten() {
        let Some(cle) = cle_de(&c["id"]) else { continue };
        let changement = ChangementPhrase {
            cle: cle.clone(),
            garder: c["garder"].as_bool(),
            regime: c["regime"].as_str().map(String::from),
            cadre: c["cadre"].as_str().map(String::from),
        };
        if let Some(f) = changer(p, ps, &changement) {
            actions.push(format!("Phrase {} : {f}", rang.get(&cle).copied().unwrap_or(0)));
        }
    }
    for i in v["ajouter_incrustations"].as_array().into_iter().flatten() {
        let texte = i["texte"].as_str().unwrap_or("").trim().to_string();
        // Un texte ne porte pas d'effet d'image ; une taille donnée en pourcentage revient entre 0 et 1.
        let effet = if texte.is_empty() { i["effet"].as_str().unwrap_or("").to_string() } else { String::new() };
        let taille = i["taille"].as_f64().map(|t| if t > 1.0 { t / 100.0 } else { t }).unwrap_or(0.4);
        let nom_media = i["media"].as_str().unwrap_or("").to_string();
        let trouve = p.medias.iter().find(|m| m.nom.eq_ignore_ascii_case(&nom_media) || m.id == nom_media);
        if texte.is_empty() && trouve.map(|m| m.genre == "son").unwrap_or(false) {
            continue;
        }
        let Some(phrase) = cle_de(&i["phrase"]) else { continue };
        let numero = rang.get(&phrase).copied().unwrap_or(0);
        let quoi = if !texte.is_empty() { format!("texte « {texte} »") } else if !effet.is_empty() { effet.clone() } else { format!("« {nom_media} »") };
        let n = NouvelleIncrustation {
            media: trouve.map(|m| m.id.clone()).unwrap_or_default(),
            effet,
            texte,
            phrase,
            decalage: i["decalage"].as_f64().unwrap_or(0.0),
            duree: i["duree"].as_f64().unwrap_or(2.0),
            position: i["position"].as_str().unwrap_or("centre").to_string(),
            taille,
            fond_vert: i["fond_vert"].as_bool().unwrap_or(false),
            son: i["son"].as_bool().unwrap_or(false),
            couleur: i["couleur"].as_str().unwrap_or("").to_string(),
            fond: i["fond"].as_str().unwrap_or("").to_string(),
            entree: i["entree"].as_str().unwrap_or("").to_string(),
            x: None,
            y: None,
        };
        match valider_incrustation(p, n) {
            Ok(inc) => {
                actions.push(format!("Incrustation {quoi} sur la phrase {numero}"));
                p.incrustations.push(inc);
            }
            Err(e) => actions.push(format!("non appliqué : {e}")),
        }
    }
    let a_retirer: Vec<String> = v["retirer_incrustations"].as_array().into_iter().flatten()
        .filter_map(|x| x.as_str().map(String::from)).collect();
    let avant = p.incrustations.len();
    p.incrustations.retain(|i| !a_retirer.contains(&i.id));
    if p.incrustations.len() < avant {
        actions.push(format!("{} incrustation(s) retirée(s)", avant - p.incrustations.len()));
    }
    actions
}

fn avec_echecs(texte: &str, actions: &[String]) -> String {
    let echecs: Vec<&str> = actions.iter().filter_map(|a| a.strip_prefix("non appliqué : ")).collect();
    if echecs.is_empty() {
        texte.to_string()
    } else {
        format!("{texte}\n\nAttention, je n'ai pas pu tout faire : {}.", echecs.join(" ; "))
    }
}

fn tous_prets(p: &Projet) -> bool {
    !p.rushs.is_empty() && p.rushs.iter().all(|r| r.etat == EtatRush::Pret)
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
    let ps = decoupe::phrases_projet(&p, &analyses);
    let v = match llm::discuter(&etat, &p, &ps, &message).await {
        Ok(v) => v,
        Err(e) => {
            let texte = format!("Je n'ai pas pu répondre : {e:#}");
            etat.modifier(&id, |p| p.discussion.push(Echange { role: "edivid".into(), texte: texte.clone(), quand: maintenant(),
                                                                 actions: vec![] })).await?;
            return Err(refus(texte));
        }
    };
    let remonter = v["remonter"].as_bool().unwrap_or(false);
    let reponse_llm = v["reponse"].as_str().unwrap_or("C'est noté.").to_string();
    let (actions, reponse) = etat
        .modifier(&id, move |p| {
            let mut actions = appliquer_actions(p, &ps, &v);
            if remonter {
                actions.push("Nouveau montage complet par le LLM".into());
            }
            let reponse = avec_echecs(&reponse_llm, &actions);
            p.discussion.push(Echange { role: "edivid".into(), texte: reponse.clone(), quand: maintenant(), actions: actions.clone() });
            (actions, reponse)
        })
        .await?;
    // Règle de l'orchestrateur : tout changement appliqué donne un nouvel aperçu.
    if remonter && tous_prets(&p) {
        etat.file.ajouter(&etat, &id, Genre::Monter);
    } else if actions.iter().any(|a| !a.starts_with("non appliqué")) && tous_prets(&p) {
        etat.file.ajouter(&etat, &id, Genre::Apercu);
    }
    Ok(Json(json!({"reponse": reponse, "actions": actions})))
}

// ---------------------------------------------------------------- commentaire sur un instant

#[derive(Deserialize)]
struct Commentaire {
    /// Instant commenté : dans la vidéo montée (apercu, rendu) ou dans un rush (origine).
    t: f64,
    vue: String,
    #[serde(default)]
    rush: String,
    texte: String,
    /// Proposition précédente, quand la personne corrige.
    #[serde(default)]
    precedente: Option<String>,
}

fn minutes(t: f64) -> String {
    let s = t.max(0.0).round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

async fn commenter(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(c): Json<Commentaire>) -> Reponse {
    let texte = c.texte.trim().to_string();
    if texte.is_empty() {
        return Err(refus("décrivez la correction voulue"));
    }
    let p = etat.projet(&id).await?;
    let analyses = etat.analyses_du_projet(&p).await?;
    let ps = decoupe::phrases_projet(&p, &analyses);
    if ps.is_empty() {
        return Err(refus("pas encore de transcription"));
    }
    // Quelle phrase passe à cet instant ?
    let (rush, src) = if c.vue == "origine" {
        (c.rush.clone(), c.t)
    } else {
        let sortie = if c.vue == "rendu" { p.rendu.as_ref() } else { p.apercu.as_ref() };
        let plans = sortie.and_then(|s| s.rapport["plans_detail"].as_array().cloned()).unwrap_or_default();
        let plan = plans.iter().find(|pl| {
            let (t0, d) = (pl["t"].as_f64().unwrap_or(0.0), pl["d"].as_f64().unwrap_or(0.0));
            c.t >= t0 && c.t < t0 + d
        });
        match plan {
            Some(pl) => (pl["rush"].as_str().unwrap_or("").to_string(),
                         pl["src"].as_f64().unwrap_or(0.0) + c.t - pl["t"].as_f64().unwrap_or(0.0)),
            None => return Err(refus("cet aperçu est ancien : mettez-le à jour avant de commenter")),
        }
    };
    let numero = ps
        .iter()
        .enumerate()
        .filter(|(_, ph)| ph.rush == rush)
        .min_by(|(_, a), (_, b)| {
            let da = if src < a.t0 { a.t0 - src } else if src > a.t1 { src - a.t1 } else { 0.0 };
            let db = if src < b.t0 { b.t0 - src } else if src > b.t1 { src - b.t1 } else { 0.0 };
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .ok_or_else(|| anyhow!("instant hors des rushs"))?;
    let precedente = c.precedente.as_ref().and_then(|pid| etat.propositions.lock().unwrap().get(pid).cloned());
    let v = llm::commenter(&etat, &p, &ps, numero, &minutes(c.t), &texte, precedente.as_ref()).await?;
    // Essai à blanc sur une copie : ce que la proposition changerait, et sur quelle plage.
    let mut copie = p.clone();
    let actions = appliquer_actions(&mut copie, &ps, &v);
    let de = v["de"].as_u64().map(|n| n as usize).unwrap_or(numero).min(ps.len() - 1);
    let a = v["a"].as_u64().map(|n| n as usize).unwrap_or(numero).clamp(de, ps.len() - 1);
    let cles: Vec<&str> = ps[de..=a].iter().map(|ph| ph.cle.as_str()).collect();
    let prog = decoupe::programme(&copie, &analyses, &ps, true);
    let plans: Vec<&decoupe::Plan> = prog.plans.iter().filter(|pl| !pl.accroche && cles.contains(&pl.phrase.as_str())).collect();
    let plage_sortie = (!plans.is_empty()).then(|| {
        json!([plans.first().unwrap().sortie_t0, plans.last().map(|pl| pl.sortie_t0 + pl.duree).unwrap_or(0.0)])
    });
    let pid = nouvel_id();
    let proposition = json!({
        "id": pid,
        "t": c.t, "vue": c.vue, "texte": texte,
        "phrase": numero, "de": de, "a": a,
        "plage_source": {"rush": ps[de].rush, "debut": ps[de].t0, "fin": ps[a].t1},
        "plage_sortie": plage_sortie,
        "reformulation": v["reformulation"], "explication": v["explication"],
        "possible": v["possible"].as_bool().unwrap_or(true),
        "actions": actions,
        "brut": v,
    });
    etat.propositions.lock().unwrap().insert(pid.clone(), proposition.clone());
    Ok(Json(proposition))
}

async fn valider_commentaire(State(etat): State<EtatPartage>, Path((id, pid)): Path<(String, String)>) -> Reponse {
    let proposition = etat.propositions.lock().unwrap().remove(&pid).ok_or_else(|| anyhow!("proposition expirée"))?;
    let p = etat.projet(&id).await?;
    let ps = decoupe::phrases_projet(&p, &etat.analyses_du_projet(&p).await?);
    let v = proposition["brut"].clone();
    let moi = format!("Commentaire à {} : {}", minutes(proposition["t"].as_f64().unwrap_or(0.0)),
                      proposition["texte"].as_str().unwrap_or(""));
    let reformulation = proposition["reformulation"].as_str().unwrap_or("").to_string();
    let actions = etat
        .modifier(&id, move |p| {
            let actions = appliquer_actions(p, &ps, &v);
            p.discussion.push(Echange { role: "moi".into(), texte: moi, quand: maintenant(), actions: vec![] });
            p.discussion.push(Echange { role: "edivid".into(), texte: avec_echecs(&format!("{reformulation} C'est appliqué."), &actions),
                                        quand: maintenant(), actions: actions.clone() });
            actions
        })
        .await?;
    if actions.iter().any(|a| !a.starts_with("non appliqué")) && tous_prets(&p) {
        etat.file.ajouter(&etat, &id, Genre::Apercu);
    }
    Ok(Json(json!({"actions": actions})))
}

async fn annuler_commentaire(State(etat): State<EtatPartage>, Path((_id, pid)): Path<(String, String)>) -> Reponse {
    etat.propositions.lock().unwrap().remove(&pid);
    Ok(Json(json!({})))
}

// ---------------------------------------------------------------- script courant, vidéo d'origine

async fn programme(State(etat): State<EtatPartage>, Path(id): Path<String>) -> Reponse {
    let p = etat.projet(&id).await?;
    let analyses = etat.analyses_du_projet(&p).await?;
    let ps = decoupe::phrases_projet(&p, &analyses);
    let prog = decoupe::programme(&p, &analyses, &ps, true);
    let mut v = decoupe::programme_json(&prog);
    v["rushs"] = json!(analyses.iter().map(|(rid, a)| json!({"id": rid, "duree": a.duree})).collect::<Vec<_>>());
    Ok(Json(v))
}

async fn video_origine(State(etat): State<EtatPartage>, Path((id, rid)): Path<(String, String)>, requete: Request)
                       -> Result<Response, Erreur> {
    let chemin = etat.dossier_travail(&id, &rid).join("proxy.mp4");
    if !chemin.exists() {
        return Err(Erreur(StatusCode::NOT_FOUND, "version d'origine en préparation".into()));
    }
    Ok(ServeFile::new(chemin).try_call(requete).await.map_err(|e| anyhow!("{e}"))?.into_response())
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

// ---------------------------------------------------------------- consignes, coulisses, Drive du projet

#[derive(Deserialize)]
struct Texte {
    texte: String,
}

async fn changer_consignes(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(t): Json<Texte>) -> Reponse {
    let texte: String = t.texte.trim().chars().take(4000).collect();
    etat.modifier(&id, |p| p.consignes = texte).await?;
    Ok(Json(json!({})))
}

/// Les consignes réellement envoyées au LLM, pour les lire et s'en inspirer.
async fn coulisses(State(etat): State<EtatPartage>, Path(id): Path<String>) -> Reponse {
    let p = etat.projet(&id).await?;
    Ok(Json(json!({
        "monteur": llm::consigne_monteur(&p, p.rushs.len() > 1),
        "critique": llm::CONSIGNE_CRITIQUE,
        "assistant": llm::CONSIGNE_DISCUSSION,
    })))
}

async fn envoyer_drive(State(etat): State<EtatPartage>, Path(id): Path<String>) -> Reponse {
    let p = etat.projet(&id).await?;
    if p.rendu.is_none() {
        return Err(refus("lancez d'abord le rendu final"));
    }
    let e = etat.clone();
    tokio::spawn(async move {
        match drive::deposer(&e, &id).await {
            Ok(chemin) => e.emettre(&id, "drive", json!({"chemin": chemin})),
            Err(err) => e.emettre(&id, "erreur", json!({"travail": "Envoi sur le Drive", "message": format!("{err:#}")})),
        }
    });
    Ok(Json(json!({})))
}

// ---------------------------------------------------------------- paramètres

async fn voir_parametres(State(etat): State<EtatPartage>) -> Json<Value> {
    let p = etat.parametres.read().await.clone();
    let mut v = parametres::vue(&p);
    v["etat_drive"] = drive::vue(&etat);
    if let Some(comptes) = v["comptes"].as_array_mut() {
        for c in comptes.iter_mut() {
            let email = c["email"].as_str().unwrap_or("").to_string();
            c["youtube"] = json!(youtube::autorise(&etat, &email));
        }
    }
    v["nvenc"] = json!(etat.nvenc);
    Json(v)
}

async fn changer_parametres(State(etat): State<EtatPartage>, Json(recu): Json<Value>) -> Reponse {
    let nouveau = {
        let actuels = etat.parametres.read().await.clone();
        parametres::fusionner(&actuels, &recu)?
    };
    parametres::enregistrer(&etat.cfg.donnees, &nouveau)?;
    *etat.parametres.write().await = nouveau.clone();
    etat.drive.reveil.notify_one();
    Ok(Json(parametres::vue(&nouveau)))
}

async fn essai_courriel(State(etat): State<EtatPartage>) -> Reponse {
    let p = etat.parametres.read().await.clone();
    courriel::envoyer(&p.courriel, "EDIVID — courriel d'essai",
                      &format!("Bonjour,\n\nCe courriel confirme qu'EDIVID sait vous prévenir. Vous recevrez un message \
                                quand un montage ou une vidéo finale sera prête.\n\n{}\n\n— EDIVID", p.url_publique))
        .await?;
    Ok(Json(json!({"message": format!("Courriel envoyé à {}", p.courriel.destinataires.join(", "))})))
}

async fn verifier_drive(State(etat): State<EtatPartage>) -> Reponse {
    let d = etat.parametres.read().await.drive.clone();
    let message = drive::verifier(&d).await?;
    Ok(Json(json!({"message": message})))
}

async fn passage_drive(State(etat): State<EtatPartage>) -> Reponse {
    let p = etat.parametres.read().await.clone();
    if !p.drive.actif && !p.comptes.iter().any(|c| c.veille) {
        return Err(refus("activez d'abord la veille d'un Drive"));
    }
    etat.drive.reveil.notify_one();
    Ok(Json(json!({})))
}

// ---------------------------------------------------------------- bibliothèque

async fn voir_bibliotheque(State(etat): State<EtatPartage>) -> Json<Value> {
    Json(json!({"integree": bibliotheque::integree(), "perso": bibliotheque::perso(&etat.cfg.donnees)}))
}

#[derive(Deserialize)]
struct NouvelleDemande {
    titre: String,
    texte: String,
}

async fn ajouter_demande(State(etat): State<EtatPartage>, Json(d): Json<NouvelleDemande>) -> Reponse {
    if d.texte.trim().is_empty() {
        return Err(refus("la demande est vide"));
    }
    let titre = if d.titre.trim().is_empty() { d.texte.chars().take(40).collect::<String>() } else { d.titre };
    Ok(Json(json!(bibliotheque::ajouter(&etat.cfg.donnees, &titre, &d.texte)?)))
}

async fn retirer_demande(State(etat): State<EtatPartage>, Path(did): Path<String>) -> Reponse {
    bibliotheque::retirer(&etat.cfg.donnees, &did)?;
    Ok(Json(json!({})))
}

// ---------------------------------------------------------------- comptes Google

#[derive(Deserialize)]
struct ChangementCompte {
    email: String,
    veille: bool,
}

async fn changer_compte(State(etat): State<EtatPartage>, Json(c): Json<ChangementCompte>) -> Reponse {
    let mut p = etat.parametres.write().await;
    let compte = p.comptes.iter_mut().find(|x| x.email.eq_ignore_ascii_case(&c.email)).ok_or_else(|| anyhow!("compte inconnu"))?;
    if c.veille && compte.distant.is_empty() {
        return Err(refus("le Drive de ce compte n'est pas relié : reconnectez-le avec Google"));
    }
    compte.veille = c.veille;
    parametres::enregistrer(&etat.cfg.donnees, &p)?;
    drop(p);
    etat.drive.reveil.notify_one();
    Ok(Json(json!({})))
}

async fn retirer_compte(State(etat): State<EtatPartage>, Path(email): Path<String>) -> Reponse {
    let distant = {
        let mut p = etat.parametres.write().await;
        let distant = p.comptes.iter().find(|c| c.email.eq_ignore_ascii_case(&email)).map(|c| c.distant.clone());
        p.comptes.retain(|c| !c.email.eq_ignore_ascii_case(&email));
        parametres::enregistrer(&etat.cfg.donnees, &p)?;
        distant
    };
    // Les jetons du Drive et de YouTube sont oubliés aussi.
    youtube::oublier(&etat, &email);
    if let Some(d) = distant.filter(|d| !d.is_empty()) {
        let _ = tokio::process::Command::new(drive::rclone()).args(["config", "delete", d.trim_end_matches(':')]).output().await;
    }
    Ok(Json(json!({})))
}

// ---------------------------------------------------------------- PostProd

async fn looks() -> Json<Value> {
    Json(json!({
        "looks": postprod::looks()["looks"],
        "couleurs": postprod::COULEURS.iter().map(|(c, n, _)| json!({"cle": c, "nom": n})).collect::<Vec<_>>(),
    }))
}

async fn changer_postprod(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(mut pp): Json<PostProd>) -> Reponse {
    pp.borner();
    etat.modifier(&id, |p| p.postprod = pp).await?;
    Ok(Json(json!({})))
}

#[derive(Deserialize)]
struct InstantImage {
    t: f64,
    #[serde(default)]
    vue: String,
    #[serde(default)]
    rush: String,
    #[serde(default)]
    avant: bool,
}

/// Image de la source à l'instant affiché, sans ou avec la PostProd du projet.
async fn image_postprod(State(etat): State<EtatPartage>, Path(id): Path<String>, Query(q): Query<InstantImage>)
                        -> Result<Response, Erreur> {
    let p = etat.projet(&id).await?;
    let (rush, src) = if q.vue == "origine" {
        (q.rush.clone(), q.t)
    } else {
        let sortie = if q.vue == "rendu" { p.rendu.as_ref() } else { p.apercu.as_ref() };
        let plans = sortie.and_then(|s| s.rapport["plans_detail"].as_array().cloned()).unwrap_or_default();
        let pl = plans.iter().find(|pl| {
            let (t0, d) = (pl["t"].as_f64().unwrap_or(0.0), pl["d"].as_f64().unwrap_or(0.0));
            q.t >= t0 && q.t < t0 + d
        }).ok_or_else(|| anyhow!("instant hors de l'aperçu"))?;
        (pl["rush"].as_str().unwrap_or("").to_string(), pl["src"].as_f64().unwrap_or(0.0) + q.t - pl["t"].as_f64().unwrap_or(0.0))
    };
    let rush = if rush.is_empty() { p.rushs.first().map(|r| r.id.clone()).unwrap_or_default() } else { rush };
    let mezz = etat.dossier_travail(&id, &rush).join("mezz.mkv");
    let jpeg = rendu::image_postprod(&mezz, src, &p.postprod, !q.avant).await?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/jpeg"), (axum::http::header::CACHE_CONTROL, "no-store")], jpeg).into_response())
}

// ---------------------------------------------------------------- corrections de transcription

#[derive(Deserialize)]
struct Correction {
    cle: String,
    texte: String,
}

async fn corriger_phrase(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(c): Json<Correction>) -> Reponse {
    let p = etat.projet(&id).await?;
    let analyses = etat.analyses_du_projet(&p).await?;
    let origine = decoupe::phrases(&analyses).into_iter().find(|ph| ph.cle == c.cle).ok_or_else(|| anyhow!("phrase introuvable"))?;
    let texte: String = c.texte.split_whitespace().collect::<Vec<_>>().join(" ");
    let cle = c.cle.clone();
    etat.modifier(&id, move |p| {
        if texte.is_empty() || texte == origine.texte {
            p.corrections.remove(&cle);
        } else {
            p.corrections.insert(cle, texte);
        }
    })
    .await?;
    Ok(Json(json!({})))
}

#[derive(Deserialize)]
struct Remplacement {
    cherche: String,
    remplace: String,
    #[serde(default)]
    casse: bool,
}

/// Remplace un mot ou une expression dans toute la transcription (erreur répétée de Whisper).
async fn remplacer_partout(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(r): Json<Remplacement>) -> Reponse {
    let cherche = r.cherche.trim().to_string();
    if cherche.is_empty() {
        return Err(refus("indiquez le texte à chercher"));
    }
    let p = etat.projet(&id).await?;
    let ps = decoupe::phrases_projet(&p, &etat.analyses_du_projet(&p).await?);
    let mut changements = Vec::new();
    for ph in &ps {
        let nouveau = if r.casse {
            ph.texte.replace(&cherche, r.remplace.trim())
        } else {
            remplacer_sans_casse(&ph.texte, &cherche, r.remplace.trim())
        };
        if nouveau != ph.texte {
            changements.push((ph.cle.clone(), nouveau));
        }
    }
    let n = changements.len();
    etat.modifier(&id, move |p| {
        for (cle, texte) in changements {
            p.corrections.insert(cle, texte);
        }
    })
    .await?;
    Ok(Json(json!({"phrases": n})))
}

fn remplacer_sans_casse(texte: &str, cherche: &str, remplace: &str) -> String {
    let bas = texte.to_lowercase();
    let cible = cherche.to_lowercase();
    let mut sortie = String::new();
    let mut i = 0;
    while let Some(pos) = bas[i..].find(&cible) {
        let debut = i + pos;
        // Les minuscules gardent ici la même longueur en octets (cas du français courant).
        if !texte.is_char_boundary(debut) || !texte.is_char_boundary(debut + cible.len()) {
            return texte.to_string();
        }
        sortie.push_str(&texte[i..debut]);
        sortie.push_str(remplace);
        i = debut + cible.len();
    }
    sortie.push_str(&texte[i..]);
    sortie
}

// ---------------------------------------------------------------- export et YouTube

#[derive(Deserialize)]
struct DemandeExport {
    preset: String,
}

async fn lancer_export(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(d): Json<DemandeExport>) -> Reponse {
    let p = etat.projet(&id).await?;
    if p.rendu.is_none() {
        return Err(refus("lancez d'abord le rendu final"));
    }
    if !rendu::PRESETS.iter().any(|(k, _)| *k == d.preset) {
        return Err(refus("préréglage inconnu"));
    }
    etat.file.ajouter(&etat, &id, Genre::Export(d.preset));
    Ok(Json(json!({})))
}

async fn retirer_export(State(etat): State<EtatPartage>, Path((id, eid)): Path<(String, String)>) -> Reponse {
    let fichier = etat
        .modifier(&id, |p| {
            let f = p.exports.iter().find(|e| e.id == eid).map(|e| e.fichier.clone());
            p.exports.retain(|e| e.id != eid);
            f
        })
        .await?;
    if let Some(f) = fichier {
        let _ = tokio::fs::remove_file(etat.dossier_projet(&id).join("sorties").join(f)).await;
    }
    Ok(Json(json!({})))
}

async fn proposer_youtube(State(etat): State<EtatPartage>, Path(id): Path<String>) -> Reponse {
    Ok(Json(youtube::proposer(&etat, &id).await?))
}

async fn publier_youtube(State(etat): State<EtatPartage>, Path(id): Path<String>, entetes: axum::http::HeaderMap,
                         Json(mut pub_): Json<youtube::Publication>) -> Reponse {
    // Le compte : celui demandé, sinon celui de la session, sinon le seul compte autorisé pour YouTube.
    if pub_.compte.is_empty() {
        let session = acces::session(&etat, &entetes).await.and_then(|s| s.email);
        let comptes = etat.parametres.read().await.comptes.clone();
        pub_.compte = session
            .filter(|e| youtube::autorise(&etat, e))
            .or_else(|| comptes.iter().map(|c| c.email.clone()).find(|e| youtube::autorise(&etat, e)))
            .unwrap_or_default();
    }
    if pub_.compte.is_empty() || !youtube::autorise(&etat, &pub_.compte) {
        return Err(refus("aucun compte Google n'a autorisé YouTube : connectez-vous avec Google (Paramètres) et acceptez l'accès à YouTube"));
    }
    let p = etat.projet(&id).await?;
    if !p.exports.iter().any(|e| e.id == pub_.export) {
        return Err(refus("export introuvable"));
    }
    youtube::lancer(&etat, &id, pub_);
    Ok(Json(json!({})))
}
