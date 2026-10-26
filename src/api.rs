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
use crate::jeux;
use crate::parametres;
use crate::postprod::{self, PostProd};
use crate::youtube;
use crate::etat::EtatPartage;
use crate::llm;
use crate::modele::{maintenant, nouvel_id, Decision, Echange, EtatRush, Generique, Habillage, Incrustation, Media, Montage, Projet,
                    Rush, Sequence};
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
        .route("/api/projets/{id}/medias/{mid}", get(fichier_media).put(remplacer_media).delete(supprimer_media))
        .route("/api/projets/{id}/reglages", post(changer_reglages))
        .route("/api/projets/{id}/phrase", post(changer_phrase))
        .route("/api/projets/{id}/accroche", post(changer_accroche))
        .route("/api/projets/{id}/chapitre", post(changer_chapitre))
        .route("/api/projets/{id}/fusion", post(fusionner))
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
        .route("/api/postprod/lut", get(table_couleurs))
        .route("/api/son/ambiances", get(ambiances))
        .route("/api/son/lieu/{nom}", get(lieu_son))
        .route("/api/projets/{id}/son/direct", get(son_direct))
        .route("/api/projets/{id}/son/niveau", get(son_niveau))
        .route("/api/projets/{id}/son/essai", get(essai_son))
        .route("/api/projets/{id}/postprod", post(changer_postprod))
        .route("/api/projets/{id}/postprod/image", get(image_postprod))
        .route("/api/projets/{id}/correction", post(corriger_phrase))
        .route("/api/projets/{id}/remplacer", post(remplacer_partout))
        .route("/api/projets/{id}/export", post(lancer_export))
        .route("/api/projets/{id}/exports/{eid}", delete(retirer_export))
        .route("/api/projets/{id}/youtube/proposer", post(proposer_youtube))
        .route("/api/projets/{id}/youtube", post(publier_youtube))
        .route("/api/bibliotheque", get(voir_bibliotheque).post(ajouter_demande))
        .route("/api/jeux", get(voir_jeux).post(ajouter_jeu))
        .route("/api/jeux/{jid}", delete(retirer_jeu))
        .route("/api/projets/{id}/jeux/{jid}", post(appliquer_jeu))
        .route("/api/projets/{id}/habillage", post(changer_habillage))
        .route("/api/projets/{id}/generique", post(changer_generique))
        .route("/api/drive/parcourir", get(parcourir_drive))
        .route("/api/projets/{id}/drive/importer", post(importer_drive))
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
        fusions: vec![],
        exports: vec![],
        habillage: vec![],
        generique: Default::default(),
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
                              "decision": decisions.get(ph.cle.as_str()), "fusionnees": ph.fusionnees,
                              // Minutage des mots (début, fin, mot) : le texte suit la lecture mot à mot.
                              "mots": ph.mots.iter().map(|m| json!([(m.t0 * 100.0).round() / 100.0, (m.t1 * 100.0).round() / 100.0, m.mot]))
                                  .collect::<Vec<_>>()}))
        .collect();
    let empreinte = crate::rendu::empreinte(&p);
    let tous_prets = !p.rushs.is_empty() && p.rushs.iter().all(|r| r.etat == EtatRush::Pret);
    let proxys: Vec<&str> = p.rushs.iter().filter(|r| etat.dossier_travail(&id, &r.id).join("proxy.mp4").exists())
        .map(|r| r.id.as_str()).collect();
    Ok(Json(json!({
        "proxys": proxys,
        "projet": p,
        "phrases": phrases,
        "pret_a_monter": tous_prets,
        "occupe": etat.file.occupe(&id),
        // Un aperçu fait avant le dernier changement n'est plus à jour (avant ce champ : null, inconnu).
        "apercu_a_jour": p.apercu.as_ref().and_then(|s| s.rapport["empreinte"].as_str()).map(|e| e == empreinte),
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

/// Remplace le fichier d'un média en gardant son identifiant : tout ce qui l'utilise (habillage, générique,
/// incrustations, musique) prend le nouveau fichier.
async fn remplacer_media(State(etat): State<EtatPartage>, Path((id, mid)): Path<(String, String)>, Query(q): Query<NomFichier>,
                         requete: Request) -> Reponse {
    let p = etat.projet(&id).await?;
    let ancien = p.medias.iter().find(|m| m.id == mid).ok_or_else(|| anyhow!("média introuvable"))?.clone();
    let fichier = format!("{mid}-{}.{}", nouvel_id(), extension(&q.nom));
    let chemin = etat.dossier_projet(&id).join("medias").join(&fichier);
    ecrire_corps(requete.into_body(), &chemin).await?;
    let (genre, duree, a_son) = match rendu::sonder_media(&chemin, &q.nom).await {
        Ok(v) => v,
        Err(e) => {
            let _ = tokio::fs::remove_file(&chemin).await;
            return Err(refus(format!("média refusé : {e}")));
        }
    };
    if genre != ancien.genre && !(["image", "gif"].contains(&genre.as_str()) && ["image", "gif"].contains(&ancien.genre.as_str())) {
        let _ = tokio::fs::remove_file(&chemin).await;
        return Err(refus(format!("remplacez une {} par un fichier du même genre", ancien.genre)));
    }
    let nom: String = q.nom.chars().take(120).collect();
    etat.modifier(&id, |p| {
        if let Some(m) = p.medias.iter_mut().find(|m| m.id == mid) {
            *m = Media { id: mid.clone(), nom, fichier, genre, duree, a_son };
        }
    })
    .await?;
    let _ = tokio::fs::remove_file(etat.dossier_projet(&id).join("medias").join(&ancien.fichier)).await;
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
            p.habillage.retain(|h| h.media != mid);
            for s in [&mut p.generique.debut, &mut p.generique.fin] {
                if s.as_ref().is_some_and(|s| s.media == mid && s.genre == "video") {
                    *s = None;
                } else if let Some(s) = s.as_mut().filter(|s| s.media == mid) {
                    s.media.clear();
                }
            }
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
    /// « auto » rend la transition au LLM et au réglage général ; « coupe » = aucune.
    #[serde(default)]
    transition: Option<String>,
    /// « auto » rend le mouvement au rythme ; sinon « fixe », « pousse » ou « punch ».
    #[serde(default)]
    mouvement: Option<String>,
    /// Rangs des mots retirés des sous-titres (liste complète).
    #[serde(default)]
    masques: Option<Vec<usize>>,
    /// Rangs des mots en emphase (liste complète, choisie à la main).
    #[serde(default)]
    emphase: Option<Vec<usize>>,
    /// Vrai : l'emphase revient au choix du LLM.
    #[serde(default)]
    emphase_auto: Option<bool>,
    /// Rangs des mots affichés à la main bien que coupés au montage (liste complète).
    #[serde(default)]
    forces: Option<Vec<usize>>,
}

fn montage_par_defaut(ps: &[crate::modele::Phrase]) -> Montage {
    Montage {
        cree: maintenant(),
        phrases: ps.iter().map(|p| Decision { cle: p.cle.clone(), garder: true, regime: "rafale".into(), cadre: "moyen".into(),
                                               transition: None, mots_forts: vec![], transition_choisie: None, mouvement: None, masques: vec![], emphase: None, forces: vec![] }).collect(),
        accroche: vec![],
        accroche_choisie: false,
        chapitres: vec![],
        notes: vec![],
    }
}

fn changer(p: &mut Projet, ps: &[crate::modele::Phrase], c: &ChangementPhrase) -> Option<String> {
    let m = p.montage.get_or_insert_with(|| montage_par_defaut(ps));
    if !m.phrases.iter().any(|d| d.cle == c.cle) {
        m.phrases.push(Decision { cle: c.cle.clone(), garder: true, regime: "rafale".into(), cadre: "moyen".into(),
                                  transition: None, mots_forts: vec![], transition_choisie: None, mouvement: None, masques: vec![], emphase: None, forces: vec![] });
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
    // Le régime ne touche jamais au cadre : changer l'un ne doit pas déplacer l'autre.
    if let Some(t) = c.transition.as_ref().filter(|t| ["auto", "coupe", "flash", "fondu", "zoom", "glisse", "glitch"].contains(&t.as_str())) {
        let t = (t != "auto").then(|| t.clone());
        if d.transition_choisie != t {
            faits.push(format!("transition {}", t.as_deref().unwrap_or("automatique")));
            d.transition_choisie = t;
        }
    }
    let nb_mots = ps.iter().find(|ph| ph.cle == c.cle).map(|ph| ph.mots.len()).unwrap_or(0);
    let propre = |v: &Vec<usize>| {
        let mut v: Vec<usize> = v.iter().copied().filter(|i| *i < nb_mots).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    if let Some(m) = c.masques.as_ref().map(propre) {
        if d.masques != m {
            d.masques = m;
            faits.push("sous-titres ajustés".into());
        }
    }
    if let Some(f) = c.forces.as_ref().map(propre) {
        if d.forces != f {
            d.forces = f;
            faits.push("sous-titres ajustés".into());
        }
    }
    if c.emphase_auto == Some(true) && d.emphase.is_some() {
        d.emphase = None;
        faits.push("emphase du LLM rétablie".into());
    } else if let Some(e) = c.emphase.as_ref().map(propre) {
        if d.emphase.as_ref() != Some(&e) {
            d.emphase = Some(e);
            faits.push("emphase ajustée".into());
        }
    }
    if let Some(m) = c.mouvement.as_ref().filter(|m| ["auto", "fixe", "pousse", "punch"].contains(&m.as_str())) {
        let m = (m != "auto").then(|| m.clone());
        if d.mouvement != m {
            faits.push(format!("mouvement {}", m.as_deref().unwrap_or("selon le rythme")));
            d.mouvement = m;
        }
    }
    (!faits.is_empty()).then(|| faits.join(", "))
}

#[derive(Deserialize)]
struct ChangementAccroche {
    cles: Vec<String>,
}

/// L'accroche choisie à la main, dans l'ordre (vide : pas d'accroche).
async fn changer_accroche(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(c): Json<ChangementAccroche>) -> Reponse {
    let p = etat.projet(&id).await?;
    let ps = decoupe::phrases_projet(&p, &etat.analyses_du_projet(&p).await?);
    let mut cles: Vec<String> = Vec::new();
    for cle in c.cles {
        if !ps.iter().any(|ph| ph.cle == cle) {
            return Err(refus("phrase introuvable"));
        }
        if !cles.contains(&cle) {
            cles.push(cle);
        }
    }
    etat.modifier(&id, |p| {
        if !cles.is_empty() {
            p.reglages.insert("accroche".into(), json!(true));
        }
        let m = p.montage.get_or_insert_with(|| montage_par_defaut(&ps));
        m.accroche = cles;
        m.accroche_choisie = true;
    })
    .await?;
    Ok(Json(json!({"fait": true})))
}

#[derive(Deserialize)]
struct ChangementChapitre {
    cle: String,
    /// Titre de la partie qui commence à cette phrase ; vide : plus de partie ici.
    titre: String,
}

async fn changer_chapitre(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(c): Json<ChangementChapitre>) -> Reponse {
    let p = etat.projet(&id).await?;
    let ps = decoupe::phrases_projet(&p, &etat.analyses_du_projet(&p).await?);
    if !ps.iter().any(|ph| ph.cle == c.cle) {
        return Err(refus("phrase introuvable"));
    }
    let titre = c.titre.trim().chars().take(80).collect::<String>();
    etat.modifier(&id, |p| {
        let m = p.montage.get_or_insert_with(|| montage_par_defaut(&ps));
        m.chapitres.retain(|x| x.cle != c.cle);
        if !titre.is_empty() {
            m.chapitres.push(crate::modele::Chapitre { cle: c.cle.clone(), titre: titre.clone() });
            // Dans l'ordre de la vidéo.
            let rang: HashMap<&str, usize> = ps.iter().enumerate().map(|(i, ph)| (ph.cle.as_str(), i)).collect();
            m.chapitres.sort_by_key(|x| rang.get(x.cle.as_str()).copied().unwrap_or(usize::MAX));
        }
    })
    .await?;
    Ok(Json(json!({"fait": true})))
}

#[derive(Deserialize)]
struct Fusion {
    cle: String,
    /// « suivante », « precedente » ou « separer ».
    sens: String,
}

/// Soude une phrase à sa voisine (la transcription l'avait coupée en deux), ou sépare une phrase soudée.
async fn fusionner(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(f): Json<Fusion>) -> Reponse {
    let p = etat.projet(&id).await?;
    let ps = decoupe::phrases_projet(&p, &etat.analyses_du_projet(&p).await?);
    let i = ps.iter().position(|ph| ph.cle == f.cle).ok_or_else(|| refus("phrase introuvable"))?;
    if f.sens == "separer" {
        let ph = ps[i].clone();
        if ph.fusionnees.is_empty() {
            return Err(refus("cette phrase n'est pas soudée"));
        }
        let mut chaine = vec![ph.cle.clone()];
        chaine.extend(ph.fusionnees[..ph.fusionnees.len() - 1].iter().cloned());
        etat.modifier(&id, |p| {
            p.fusions.retain(|c| !chaine.contains(c));
            // Le texte corrigé couvrait toute la phrase soudée : il ne vaut plus pour ses morceaux.
            p.corrections.remove(&ph.cle);
            if let Some(m) = p.montage.as_mut() {
                if let Some(d) = m.phrases.iter().find(|d| d.cle == ph.cle).cloned() {
                    for c in &ph.fusionnees {
                        m.phrases.retain(|x| &x.cle != c);
                        m.phrases.push(Decision { cle: c.clone(), ..d.clone() });
                    }
                }
            }
        })
        .await?;
        return Ok(Json(json!({"fait": true})));
    }
    let (a, b) = match f.sens.as_str() {
        "suivante" if i + 1 < ps.len() => (ps[i].clone(), ps[i + 1].clone()),
        "precedente" if i > 0 => (ps[i - 1].clone(), ps[i].clone()),
        _ => return Err(refus("pas de phrase voisine de ce côté")),
    };
    if a.rush != b.rush {
        return Err(refus("les deux phrases viennent de rushs différents"));
    }
    etat.modifier(&id, |p| {
        p.fusions.push(a.fusionnees.last().unwrap_or(&a.cle).clone());
        if p.corrections.contains_key(&a.cle) || p.corrections.contains_key(&b.cle) {
            p.corrections.insert(a.cle.clone(), format!("{} {}", a.texte, b.texte));
            p.corrections.remove(&b.cle);
        }
        for inc in p.incrustations.iter_mut().filter(|inc| inc.phrase == b.cle) {
            inc.phrase = a.cle.clone();
            inc.decalage += b.t0 - a.t0;
        }
        if let Some(m) = p.montage.as_mut() {
            // La phrase soudée est gardée si l'une des deux l'était, avec les choix de celle-là.
            let da = m.phrases.iter().find(|d| d.cle == a.cle).cloned();
            let db = m.phrases.iter().find(|d| d.cle == b.cle).cloned();
            if let (Some(da), Some(db)) = (da, db) {
                if !da.garder && db.garder {
                    m.phrases.retain(|x| x.cle != a.cle);
                    m.phrases.push(Decision { cle: a.cle.clone(), ..db });
                }
            }
            for c in m.accroche.iter_mut().filter(|c| **c == b.cle) {
                *c = a.cle.clone();
            }
            m.accroche.dedup();
            let a_chapitre = m.chapitres.iter().any(|c| c.cle == a.cle);
            m.chapitres.retain(|c| c.cle != b.cle || !a_chapitre);
            for c in m.chapitres.iter_mut().filter(|c| c.cle == b.cle) {
                c.cle = a.cle.clone();
            }
        }
    })
    .await?;
    Ok(Json(json!({"fait": true})))
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

const ENTREES: &[&str] = &["", "aucune", "fondu", "splash", "zoom", "secousse", "glisse_gauche", "glisse_droite", "glisse_haut",
                            "glisse_bas", "machine"];
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
    let medias: Vec<String> = etat.projet(&id).await?.medias.iter().filter(|m| m.genre != "son").map(|m| m.id.clone()).collect();
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
            // Une place nommée remplace la position libre posée à la main sur la vidéo.
            if let Some(pos) = v["position"].as_str().filter(|s| !s.is_empty()) {
                i.position = pos.to_string();
                i.x = None;
                i.y = None;
            }
            if let Some(m) = v["media"].as_str().filter(|m| !i.media.is_empty() && medias.contains(&m.to_string())) {
                i.media = m.to_string();
            }
            if let Some(b) = v["fond_vert"].as_bool() {
                i.fond_vert = b;
            }
            if let Some(b) = v["son"].as_bool() {
                i.son = b;
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
            transition: None,
            mouvement: None,
            masques: None,
            emphase: None,
            emphase_auto: None,
            forces: None,
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
    // Les sous-titres tels que le rendu les écrira, groupe par groupe : l'aperçu en direct les dessine.
    v["sous_titres_groupes"] = json!(rendu::groupes_sous_titres(&prog, &p.reglages).iter()
        .map(|g| json!([(g.debut * 1000.0).round() / 1000.0, (g.fin * 1000.0).round() / 1000.0, g.mots]))
        .collect::<Vec<_>>());
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
    let p = etat.parametres.read().await.clone();
    // Le Drive de chaque compte relié, et le Drive réglé à la main (rclone config) s'il sert encore.
    let mut drives: Vec<(String, parametres::Drive)> = p
        .comptes
        .iter()
        .filter(|c| !c.distant.is_empty())
        .map(|c| (c.email.clone(), parametres::Drive { distant: c.distant.clone(), ..p.drive.clone() }))
        .collect();
    if p.drive.actif && !drives.iter().any(|(_, d)| d.distant == p.drive.distant) {
        drives.push((p.drive.distant.clone(), p.drive.clone()));
    }
    if drives.is_empty() {
        return Err(refus("aucun Drive relié : reliez d'abord un compte Google (carte « Connexion avec Google »)"));
    }
    let mut lignes = Vec::new();
    let mut echec = false;
    for (qui, d) in drives {
        match drive::verifier(&d).await {
            Ok(m) => lignes.push(format!("{qui} : {m}")),
            Err(e) => {
                echec = true;
                lignes.push(format!("{qui} : {e:#}"));
            }
        }
    }
    if echec {
        return Err(refus(lignes.join(" — ")));
    }
    Ok(Json(json!({"message": lignes.join(" — ")})))
}

async fn passage_drive(State(etat): State<EtatPartage>) -> Reponse {
    let p = etat.parametres.read().await.clone();
    if !p.drive.actif && !p.comptes.iter().any(|c| c.veille) {
        return Err(refus("activez d'abord la veille d'un Drive"));
    }
    etat.drive.reveil.notify_one();
    Ok(Json(json!({})))
}

// ---------------------------------------------------------------- habillage et générique

const POSITIONS_HABILLAGE: &[&str] = &["haut_gauche", "haut_droite", "bas_gauche", "bas_droite", "haut", "bas"];

/// Remplace l'habillage du projet (liste entière), après vérification de chaque élément.
async fn changer_habillage(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(v): Json<Vec<Habillage>>) -> Reponse {
    let p = etat.projet(&id).await?;
    if v.len() > 6 {
        return Err(refus("six éléments d'habillage au plus"));
    }
    let mut propre = Vec::new();
    for mut h in v {
        match h.genre.as_str() {
            "logo" => {
                if !p.medias.iter().any(|m| m.id == h.media && (m.genre == "image" || m.genre == "gif")) {
                    return Err(refus("le logo doit être une image de la médiathèque du projet"));
                }
                h.texte.clear();
            }
            "texte" => {
                h.texte = h.texte.trim().chars().take(80).collect();
                if h.texte.is_empty() {
                    return Err(refus("le texte d'habillage est vide"));
                }
                h.media.clear();
                if !["blanc", "noir", "jaune"].contains(&h.couleur.as_str()) {
                    h.couleur = "blanc".into();
                }
            }
            _ => return Err(refus("un élément d'habillage est un logo ou un texte")),
        }
        if !POSITIONS_HABILLAGE.contains(&h.position.as_str()) {
            h.position = "haut_droite".into();
        }
        h.taille = if h.taille > 0.0 { h.taille.clamp(0.03, 0.6) } else if h.genre == "logo" { 0.18 } else { 0.04 };
        h.opacite = if h.opacite > 0.0 { h.opacite.clamp(0.1, 1.0) } else { 0.9 };
        h.ombre = h.ombre.clamp(0.0, 1.0);
        h.halo = h.halo.clamp(0.0, 1.0);
        if h.id.is_empty() {
            h.id = nouvel_id();
        }
        propre.push(h);
    }
    etat.modifier(&id, |p| p.habillage = propre.clone()).await?;
    Ok(Json(json!({"habillage": propre})))
}

/// Vérifie une séquence du générique : une vidéo de la médiathèque, ou un carton (titre, image facultative).
fn borner_sequence(p: &Projet, s: Option<Sequence>) -> Result<Option<Sequence>, String> {
    let Some(mut s) = s else { return Ok(None) };
    s.titre = s.titre.trim().chars().take(80).collect();
    s.sous_titre = s.sous_titre.trim().chars().take(120).collect();
    match s.genre.as_str() {
        "video" => {
            if !p.medias.iter().any(|m| m.id == s.media && m.genre == "video") {
                return Err("choisissez une vidéo de la médiathèque du projet".into());
            }
            s.duree = s.duree.clamp(0.0, 60.0);
        }
        "carton" => {
            if !s.media.is_empty() && !p.medias.iter().any(|m| m.id == s.media && (m.genre == "image" || m.genre == "gif")) {
                return Err("l'image du carton doit être une image de la médiathèque".into());
            }
            if s.titre.is_empty() && s.media.is_empty() {
                return Err("un carton a besoin d'un titre ou d'une image".into());
            }
            s.duree = if s.duree > 0.0 { s.duree.clamp(1.5, 10.0) } else { 3.0 };
        }
        _ => return Err("une séquence du générique est une vidéo ou un carton".into()),
    }
    Ok(Some(s))
}

async fn changer_generique(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(g): Json<Generique>) -> Reponse {
    let p = etat.projet(&id).await?;
    let g = Generique { debut: borner_sequence(&p, g.debut).map_err(refus)?, fin: borner_sequence(&p, g.fin).map_err(refus)? };
    etat.modifier(&id, |p| p.generique = g.clone()).await?;
    Ok(Json(json!({"generique": g})))
}

// ---------------------------------------------------------------- Google Drive : choisir des fichiers

#[derive(Deserialize)]
struct Parcours {
    #[serde(default)]
    compte: String,
    #[serde(default)]
    chemin: String,
    #[serde(default)]
    genre: String,
}

/// Le Drive relié d'un compte (le premier s'il n'est pas précisé).
async fn drive_du_compte(etat: &EtatPartage, compte: &str) -> Result<(String, String), Erreur> {
    let p = etat.parametres.read().await;
    p.comptes
        .iter()
        .filter(|c| !c.distant.is_empty())
        .find(|c| compte.is_empty() || c.email.eq_ignore_ascii_case(compte))
        .map(|c| (c.email.clone(), c.distant.clone()))
        .ok_or_else(|| refus("aucun Google Drive relié : connectez votre compte Google dans Paramètres"))
}

async fn parcourir_drive(State(etat): State<EtatPartage>, Query(q): Query<Parcours>) -> Reponse {
    let (email, distant) = drive_du_compte(&etat, &q.compte).await?;
    let comptes: Vec<Value> = etat.parametres.read().await.comptes.iter().filter(|c| !c.distant.is_empty())
        .map(|c| json!({"email": c.email, "nom": c.nom})).collect();
    let mut v = drive::parcourir(&distant, q.chemin.trim_matches('/'), &q.genre).await?;
    v["compte"] = json!(email);
    v["comptes"] = json!(comptes);
    Ok(Json(v))
}

#[derive(Deserialize)]
struct FichierDrive {
    chemin: String,
    nom: String,
    #[serde(default)]
    taille: u64,
}

#[derive(Deserialize)]
struct ImportDrive {
    #[serde(default)]
    compte: String,
    fichiers: Vec<FichierDrive>,
    /// « rushs » ou « medias ».
    cible: String,
}

/// Télécharge des fichiers du Drive dans le projet : les rushs apparaissent aussitôt (« Téléchargement »),
/// puis suivent le même chemin qu'un fichier déposé.
async fn importer_drive(State(etat): State<EtatPartage>, Path(id): Path<String>, Json(i): Json<ImportDrive>) -> Reponse {
    etat.projet(&id).await?;
    let (_, distant) = drive_du_compte(&etat, &i.compte).await?;
    if i.fichiers.is_empty() {
        return Err(refus("choisissez au moins un fichier"));
    }
    let rushs = i.cible == "rushs";
    let dossier = etat.dossier_projet(&id).join(if rushs { "rushs" } else { "medias" });
    tokio::fs::create_dir_all(&dossier).await.map_err(anyhow::Error::from)?;
    let mut a_faire = Vec::new();
    for f in &i.fichiers {
        let fid = nouvel_id();
        let fichier = format!("{fid}.{}", extension(&f.nom));
        a_faire.push((fid, fichier, f.chemin.clone(), f.nom.chars().take(120).collect::<String>(), f.taille));
    }
    if rushs {
        let nouveaux: Vec<Rush> = a_faire.iter().map(|(fid, fichier, _, nom, taille)| Rush {
            id: fid.clone(), nom: nom.clone(), fichier: fichier.clone(), taille: *taille, etat: EtatRush::Telechargement,
            erreur: None, source: None }).collect();
        etat.modifier(&id, |p| p.rushs.extend(nouveaux)).await?;
    }
    let n = a_faire.len();
    let etat2 = etat.clone();
    tokio::spawn(async move {
        for (k, (fid, fichier, chemin, nom, taille)) in a_faire.into_iter().enumerate() {
            let local = dossier.join(&fichier);
            let libelle = format!("Téléchargement depuis Google Drive ({}/{n}) : {nom}", k + 1);
            let res = drive::telecharger(&etat2, &id, &distant, &chemin, &local, taille, &libelle).await;
            if rushs {
                let echec = res.as_ref().err().map(|e| format!("téléchargement depuis Google Drive impossible : {e:#}"));
                let _ = etat2.modifier(&id, |p| {
                    if let Some(r) = p.rushs.iter_mut().find(|r| r.id == fid) {
                        r.etat = if echec.is_some() { EtatRush::Erreur } else { EtatRush::Televerse };
                        r.erreur = echec.clone();
                        if let Ok(t) = std::fs::metadata(&local).map(|m| m.len()) {
                            r.taille = t;
                        }
                    }
                }).await;
                if res.is_ok() {
                    etat2.file.ajouter(&etat2, &id, Genre::Preparer(fid));
                }
            } else {
                let media = match res {
                    Ok(()) => rendu::sonder_media(&local, &nom).await.map(|(genre, duree, a_son)| Media {
                        id: fid, nom: nom.clone(), fichier: fichier.clone(), genre, duree, a_son }),
                    Err(e) => Err(e),
                };
                match media {
                    Ok(m) => { let _ = etat2.modifier(&id, |p| p.medias.push(m)).await; }
                    Err(e) => {
                        let _ = tokio::fs::remove_file(&local).await;
                        etat2.emettre(&id, "erreur", json!({"travail": "Import depuis Google Drive", "message": format!("{nom} : {e:#}")}));
                    }
                }
            }
        }
        etat2.signaler(&id, "Téléchargement depuis Google Drive", 100.0);
    });
    Ok(Json(json!({"lances": n})))
}

// ---------------------------------------------------------------- jeux de réglages

async fn voir_jeux(State(etat): State<EtatPartage>) -> Json<Value> {
    Json(json!({"jeux": jeux::liste(&etat.cfg.donnees)}))
}

#[derive(Deserialize)]
struct NouveauJeu {
    nom: String,
    projet: String,
}

/// Enregistre les réglages et la PostProd du projet sous ce nom.
async fn ajouter_jeu(State(etat): State<EtatPartage>, Json(j): Json<NouveauJeu>) -> Reponse {
    let p = etat.projet(&j.projet).await?;
    let medias = etat.dossier_projet(&p.id).join("medias");
    Ok(Json(json!(jeux::ajouter(&etat.cfg.donnees, &j.nom, &p, &medias).await?)))
}

async fn retirer_jeu(State(etat): State<EtatPartage>, Path(jid): Path<String>) -> Reponse {
    jeux::retirer(&etat.cfg.donnees, &jid)?;
    Ok(Json(json!({})))
}

/// Recharge un jeu sur le projet : chaque réglage passe par la même validation qu'à la main.
async fn appliquer_jeu(State(etat): State<EtatPartage>, Path((id, jid)): Path<(String, String)>) -> Reponse {
    let jeu = jeux::trouver(&etat.cfg.donnees, &jid).ok_or_else(|| refus("ce jeu de réglages n'existe plus"))?;
    // Logos et vidéos du jeu : copiés dans la médiathèque du projet avant d'y faire référence.
    jeux::copier_medias(&jeu.medias, &jeux::dossier(&etat.cfg.donnees, &jid), &etat.dossier_projet(&id).join("medias")).await?;
    let changements: serde_json::Map<String, Value> = jeu.reglages.clone().into_iter().collect();
    let (faits, refus_) = etat
        .modifier(&id, |p| {
            // La musique d'un autre projet n'est pas dans cette médiathèque : on garde celle du projet.
            let mut changements = changements.clone();
            if let Some(m) = changements.get("musique").and_then(Value::as_str) {
                if !m.is_empty() && !p.medias.iter().any(|x| x.id == m) {
                    changements.remove("musique");
                }
            }
            let res = appliquer_reglages(p, &changements);
            p.postprod = jeu.postprod.clone();
            for m in &jeu.medias {
                if !p.medias.iter().any(|x| x.id == m.id) {
                    p.medias.push(m.clone());
                }
            }
            p.habillage = jeu.habillage.clone();
            p.generique = jeu.generique.clone();
            res
        })
        .await?;
    Ok(Json(json!({"faits": faits, "refus": refus_, "jeu": jeu.nom})))
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

/// Ce que le navigateur joue en direct sur l'aperçu : les filtres ffmpeg de la partie directe de la chaîne de la
/// voix, le lieu, les pistes à part de l'aperçu, et le gain final s'il est déjà mesuré pour ces réglages.
async fn son_direct(State(etat): State<EtatPartage>, Path(id): Path<String>) -> Reponse {
    let p = etat.projet(&id).await?;
    let espace = reglages::texte(&p.reglages, "espace");
    let lieu = rendu::lieu_direct(&etat, &espace).await?.map(|(_, part)| json!({"url": format!("/api/son/lieu/{espace}"), "part": part}));
    Ok(Json(json!({
        "filtres": rendu::filtres_direct(&p.reglages),
        "lieu": lieu,
        "pistes": p.apercu.as_ref().map(|a| a.rapport["son_direct"].clone()),
    })))
}

/// Gain final du son en direct pour les réglages actuels (quelques secondes la première fois, puis gardé).
async fn son_niveau(State(etat): State<EtatPartage>, Path(id): Path<String>) -> Reponse {
    let p = etat.projet(&id).await?;
    Ok(Json(json!({"gain": rendu::niveau_direct(&etat, &p).await?})))
}

async fn lieu_son(State(etat): State<EtatPartage>, Path(nom): Path<String>) -> Result<Response, Erreur> {
    let (f, _) = rendu::lieu_direct(&etat, &nom).await?.ok_or_else(|| refus("lieu inconnu"))?;
    let octets = tokio::fs::read(f).await.map_err(|e| anyhow!("{e}"))?;
    Ok(([(axum::http::header::CONTENT_TYPE, "audio/wav"), (axum::http::header::CACHE_CONTROL, "max-age=86400")], octets).into_response())
}

const AMBIANCES: &str = include_str!("../bibliotheque/ambiances.json");

async fn ambiances() -> Json<Value> {
    Json(serde_json::from_str(AMBIANCES).expect("bibliotheque/ambiances.json invalide"))
}

#[derive(Deserialize)]
struct Essai {
    /// Ambiance de la bibliothèque à essayer ; absente : les réglages actuels du projet.
    #[serde(default)]
    ambiance: String,
}

/// Vingt secondes du premier rush, traitées avec une ambiance (ou les réglages du projet) : on compare à l'oreille.
async fn essai_son(State(etat): State<EtatPartage>, Path(id): Path<String>, Query(q): Query<Essai>) -> Result<Response, Erreur> {
    let p = etat.projet(&id).await?;
    let analyses = etat.analyses_du_projet(&p).await?;
    let (rush, _) = analyses.first().ok_or_else(|| refus("aucun rush prêt"))?;
    let mut r = p.reglages.clone();
    if !q.ambiance.is_empty() {
        let tout: Value = serde_json::from_str(AMBIANCES).expect("bibliotheque/ambiances.json invalide");
        let a = tout["ambiances"].as_array().into_iter().flatten().find(|a| a["cle"] == q.ambiance.as_str())
            .ok_or_else(|| refus("ambiance inconnue"))?;
        for (k, v) in a["reglages"].as_object().into_iter().flatten() {
            r.insert(k.clone(), v.clone());
        }
    }
    // Un passage parlé : la première phrase gardée du rush, et ce qui suit.
    let ps = decoupe::phrases_projet(&p, &analyses);
    let gardee = |cle: &str| p.montage.as_ref().and_then(|m| m.phrases.iter().find(|d| d.cle == cle)).map(|d| d.garder).unwrap_or(true);
    let debut = ps.iter().find(|ph| &ph.rush == rush && gardee(&ph.cle)).map(|ph| (ph.t0 - 0.3).max(0.0)).unwrap_or(0.0);
    let octets = rendu::essai_son(&etat, &id, rush, debut, 20.0, &r).await?;
    Ok(([(axum::http::header::CONTENT_TYPE, "audio/mpeg")], octets).into_response())
}

async fn looks() -> Json<Value> {
    Json(json!({
        "looks": postprod::looks()["looks"],
        "couleurs": postprod::COULEURS.iter().map(|(c, n, _)| json!({"cle": c, "nom": n})).collect::<Vec<_>>(),
    }))
}

/// Table de couleurs d'une PostProd donnée dans l'adresse (pas celle enregistrée) : pendant qu'on tire un
/// curseur, l'aperçu en direct suit sans attendre l'enregistrement.
async fn table_couleurs(State(etat): State<EtatPartage>, Query(mut pp): Query<PostProd>) -> Result<Response, Erreur> {
    pp.borner();
    let octets = rendu::table_couleurs(&etat, &pp).await?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/png"), (axum::http::header::CACHE_CONTROL, "max-age=86400")], octets).into_response())
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
    let avant = decoupe::phrases_projet(&p, &analyses).into_iter().find(|ph| ph.cle == c.cle).map(|ph| ph.mots.len()).unwrap_or(0);
    let apres = if texte.is_empty() { 0 } else { texte.split(' ').count() };
    etat.modifier(&id, move |p| {
        // Le nombre de mots change : les mots retirés des sous-titres et l'emphase, notés par rang, ne valent plus.
        if apres != avant {
            if let Some(d) = p.montage.as_mut().and_then(|m| m.phrases.iter_mut().find(|d| d.cle == cle)) {
                d.masques.clear();
                d.emphase = None;
                d.forces.clear();
            }
        }
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
