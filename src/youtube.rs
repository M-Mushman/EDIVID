//! Publication sur YouTube (API YouTube Data v3, envoi reprenable). Les jetons de rafraîchissement
//! Google sont gardés sur azoth (`<donnees>/comptes.json`, droits 600).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_util::io::ReaderStream;

use crate::courriel;
use crate::decoupe;
use crate::etat::EtatPartage;
use crate::llm;
use crate::modele::{maintenant, Echange};

pub const PORTEE: &str = "https://www.googleapis.com/auth/youtube.upload";

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Jetons {
    pub refresh_token: String,
    pub portees: String,
}

fn chemin(etat: &EtatPartage) -> std::path::PathBuf {
    etat.cfg.donnees.join("comptes.json")
}

pub fn charger(etat: &EtatPartage) -> HashMap<String, Jetons> {
    std::fs::read_to_string(chemin(etat)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn garder(etat: &EtatPartage, email: &str, jetons: Jetons) -> Result<()> {
    let mut tous = charger(etat);
    tous.insert(email.to_lowercase(), jetons);
    crate::fichiers::ecrire(&chemin(etat), &serde_json::to_vec_pretty(&tous)?, true)
}

/// Accès accordés par Google pour ce compte (vide s'il n'est pas relié).
pub fn portees(etat: &EtatPartage, email: &str) -> String {
    charger(etat).get(&email.to_lowercase()).map(|j| j.portees.clone()).unwrap_or_default()
}

pub fn oublier(etat: &EtatPartage, email: &str) {
    let mut tous = charger(etat);
    if tous.remove(&email.to_lowercase()).is_some() {
        if let Err(e) = crate::fichiers::ecrire(&chemin(etat), &serde_json::to_vec_pretty(&tous).unwrap_or_default(), true) {
            tracing::warn!("jetons Google non effacés du disque : {e}");
        }
    }
}

/// Nom de la chaîne YouTube du compte connecté, pour montrer où partiront les vidéos.
pub async fn nom_chaine(etat: &EtatPartage, jeton_acces: &str) -> Option<String> {
    let v: Value = etat
        .http
        .get("https://www.googleapis.com/youtube/v3/channels?part=snippet&mine=true")
        .bearer_auth(jeton_acces)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    v["items"][0]["snippet"]["title"].as_str().map(str::to_string)
}

/// Ce compte a-t-il autorisé l'envoi sur YouTube ?
pub fn autorise(etat: &EtatPartage, email: &str) -> bool {
    charger(etat).get(&email.to_lowercase()).map(|j| j.portees.contains(PORTEE) && !j.refresh_token.is_empty()).unwrap_or(false)
}

async fn jeton_acces(etat: &EtatPartage, email: &str) -> Result<String> {
    let j = charger(etat).get(&email.to_lowercase()).cloned().ok_or_else(|| anyhow!("compte Google non relié"))?;
    let g = etat.parametres.read().await.google.clone();
    let v: Value = etat
        .http
        .post("https://oauth2.googleapis.com/token")
        .form(&[("client_id", g.client_id.as_str()), ("client_secret", g.client_secret.as_str()),
                ("refresh_token", j.refresh_token.as_str()), ("grant_type", "refresh_token")])
        .timeout(Duration::from_secs(20))
        .send()
        .await?
        .json()
        .await?;
    v["access_token"].as_str().map(String::from).ok_or_else(|| anyhow!("Google refuse le renouvellement du jeton : {v}"))
}

#[derive(Deserialize)]
pub struct Publication {
    pub export: String,
    #[serde(default)]
    pub compte: String,
    pub titre: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub mots_cles: Vec<String>,
    /// private, unlisted ou public.
    #[serde(default)]
    pub visibilite: String,
    /// 22 (personnes et blogs), 26 (vie pratique et style), 27 (éducation)…
    #[serde(default)]
    pub categorie: String,
    /// Vignette du projet (fichier de `sorties/vignettes/`), envoyée après la vidéo.
    #[serde(default)]
    pub vignette: String,
}

/// Envoie un export sur YouTube ; renvoie le lien de la vidéo.
pub async fn publier(etat: &EtatPartage, projet: &str, pub_: &Publication) -> Result<String> {
    let p = etat.projet(projet).await?;
    let ex = p.exports.iter().find(|e| e.id == pub_.export).ok_or_else(|| anyhow!("export introuvable"))?;
    let fichier = etat.dossier_projet(projet).join("sorties").join(&ex.fichier);
    let taille = tokio::fs::metadata(&fichier).await?.len();
    let jeton = jeton_acces(etat, &pub_.compte).await?;
    let visibilite = if ["private", "unlisted", "public"].contains(&pub_.visibilite.as_str()) { pub_.visibilite.as_str() } else { "private" };
    let titre: String = pub_.titre.trim().chars().take(100).collect();
    if titre.is_empty() {
        bail!("le titre est obligatoire");
    }
    let meta = json!({
        "snippet": {
            "title": titre,
            "description": pub_.description.chars().take(5000).collect::<String>(),
            "tags": pub_.mots_cles.iter().take(30).collect::<Vec<_>>(),
            "categoryId": if pub_.categorie.is_empty() { "22" } else { pub_.categorie.as_str() },
            "defaultLanguage": "fr", "defaultAudioLanguage": "fr",
        },
        "status": {"privacyStatus": visibilite, "selfDeclaredMadeForKids": false},
    });
    let session = etat
        .http
        .post("https://www.googleapis.com/upload/youtube/v3/videos?uploadType=resumable&part=snippet,status")
        .bearer_auth(&jeton)
        .header("X-Upload-Content-Type", "video/mp4")
        .header("X-Upload-Content-Length", taille.to_string())
        .json(&meta)
        .timeout(Duration::from_secs(30))
        .send()
        .await?;
    if !session.status().is_success() {
        let texte = session.text().await.unwrap_or_default();
        bail!("YouTube refuse l'envoi : {}", court(&texte));
    }
    let adresse = session.headers().get("location").and_then(|v| v.to_str().ok()).ok_or_else(|| anyhow!("YouTube n'a pas ouvert d'envoi"))?.to_string();
    // Envoi du fichier, avec la progression dans l'interface.
    let f = tokio::fs::File::open(&fichier).await?;
    let (e, pid) = (etat.clone(), projet.to_string());
    let mut envoye = 0u64;
    let mut dernier = Instant::now();
    let flux = ReaderStream::with_capacity(f, 1 << 20).map(move |morceau| {
        if let Ok(m) = &morceau {
            envoye += m.len() as u64;
            if dernier.elapsed().as_millis() > 700 {
                e.signaler(&pid, "Envoi sur YouTube", 100.0 * envoye as f64 / taille.max(1) as f64);
                dernier = Instant::now();
            }
        }
        morceau
    });
    let reponse = etat
        .http
        .put(&adresse)
        .bearer_auth(&jeton)
        .header("Content-Type", "video/mp4")
        .header("Content-Length", taille.to_string())
        .body(reqwest::Body::wrap_stream(flux))
        .timeout(Duration::from_secs(6 * 3600))
        .send()
        .await
        .context("envoi interrompu")?;
    let v: Value = reponse.json().await?;
    let id = v["id"].as_str().ok_or_else(|| anyhow!("YouTube n'a pas confirmé l'envoi : {}", court(&v.to_string())))?;
    if !pub_.vignette.is_empty() {
        // Une vignette refusée (chaîne pas encore validée par téléphone, en général) n'annule pas la publication.
        if let Err(e) = envoyer_vignette(etat, projet, &jeton, id, &pub_.vignette).await {
            etat.emettre(projet, "erreur", json!({"travail": "Vignette YouTube",
                "message": format!("la vidéo est en ligne, mais pas sa vignette : {e:#}. La chaîne doit être validée par téléphone pour accepter une vignette personnalisée.")}));
        }
    }
    Ok(format!("https://youtu.be/{id}"))
}

/// Pose la vignette choisie sur une vidéo déjà envoyée (`thumbnails.set`).
async fn envoyer_vignette(etat: &EtatPartage, projet: &str, jeton: &str, video: &str, nom: &str) -> Result<()> {
    let f = crate::vignette::chemin(etat, projet, nom)?;
    let octets = tokio::fs::read(&f).await?;
    let r = etat
        .http
        .post(format!("https://www.googleapis.com/upload/youtube/v3/thumbnails/set?videoId={video}&uploadType=media"))
        .bearer_auth(jeton)
        .header("Content-Type", "image/jpeg")
        .body(octets)
        .timeout(Duration::from_secs(60))
        .send()
        .await?;
    if !r.status().is_success() {
        bail!("{}", court(&r.text().await.unwrap_or_default()));
    }
    Ok(())
}

fn court(t: &str) -> String {
    t.chars().take(300).collect()
}

/// Lance la publication sans bloquer ; consigne le résultat et prévient par courriel.
pub fn lancer(etat: &EtatPartage, projet: &str, pub_: Publication) {
    let (etat, projet) = (etat.clone(), projet.to_string());
    tokio::spawn(async move {
        match publier(&etat, &projet, &pub_).await {
            Ok(lien) => {
                let (l, eid, titre, vis) = (lien.clone(), pub_.export.clone(), pub_.titre.clone(), pub_.visibilite.clone());
                let _ = etat
                    .modifier(&projet, move |p| {
                        if let Some(e) = p.exports.iter_mut().find(|e| e.id == eid) {
                            e.youtube = Some(l.clone());
                        }
                        p.discussion.push(Echange { role: "edivid".into(), quand: maintenant(), actions: vec![],
                            texte: format!("« {titre} » est en ligne sur YouTube ({}) : {l}",
                                           match vis.as_str() { "public" => "publique", "unlisted" => "non répertoriée", _ => "privée" }) });
                    })
                    .await;
                etat.emettre(&projet, "youtube", json!({"lien": lien}));
                if let Ok(p) = etat.projet(&projet).await {
                    courriel::prevenir(&etat, "rendu", &p, "").await;
                }
            }
            Err(err) => {
                let message = format!("{err:#}");
                etat.emettre(&projet, "erreur", json!({"travail": "Publication sur YouTube", "message": message}));
                if let Ok(p) = etat.projet(&projet).await {
                    courriel::prevenir(&etat, "erreur", &p, &format!("publication sur YouTube : {message}")).await;
                }
            }
        }
    });
}

/// Le texte que dit la vidéo montée (phrases gardées, accroche exclue).
pub async fn texte_du_montage(etat: &EtatPartage, p: &crate::modele::Projet) -> Result<String> {
    let analyses = etat.analyses_du_projet(p).await?;
    let ps = decoupe::phrases_projet(p, &analyses);
    let prog = decoupe::programme(p, &analyses, &ps, false);
    let gardees: Vec<String> = prog.plans.iter().filter(|pl| !pl.accroche).map(|pl| pl.phrase.clone()).collect();
    Ok(ps.iter().filter(|ph| gardees.contains(&ph.cle)).map(|ph| ph.texte.as_str()).collect::<Vec<_>>().join(" "))
}

/// Titre, description (avec chapitres horodatés) et mots-clés proposés par le LLM.
pub async fn proposer(etat: &EtatPartage, projet: &str) -> Result<Value> {
    let p = etat.projet(projet).await?;
    let analyses = etat.analyses_du_projet(&p).await?;
    let ps = decoupe::phrases_projet(&p, &analyses);
    let prog = decoupe::programme(&p, &analyses, &ps, false);
    let texte = texte_du_montage(etat, &p).await?;
    // Chapitres YouTube : « 0:00 » en premier, au moins trois, dix secondes chacun au minimum.
    // Les titres sont au temps du montage : la séquence d'ouverture du générique les décale d'autant.
    let (ouverture, _) = crate::rendu::durees_generique(&p);
    let mut chapitres: Vec<(f64, String)> = Vec::new();
    for (t, titre) in &prog.titres {
        let t = t + ouverture;
        if chapitres.last().map(|(d, _)| t - d >= 10.0).unwrap_or(true) {
            chapitres.push((t, titre.clone()));
        }
    }
    if chapitres.first().map(|(t, _)| *t > 0.5).unwrap_or(false) {
        chapitres.insert(0, (0.0, "Introduction".into()));
    }
    let liste_chapitres = if chapitres.len() >= 3 {
        chapitres.iter().map(|(t, ti)| format!("{}:{:02} {ti}", (*t as u64) / 60, (*t as u64) % 60)).collect::<Vec<_>>().join("\n")
    } else {
        String::new()
    };
    let schema = json!({"type": "object", "properties": {
        "titre": {"type": "string"}, "description": {"type": "string"},
        "mots_cles": {"type": "array", "items": {"type": "string"}}},
        "required": ["titre", "description", "mots_cles"]});
    let consigne = "Tu prépares la publication YouTube d'une vidéo face caméra en français. À partir de son texte, propose : \
un titre accrocheur et honnête (60 caractères au plus, sans majuscules partout ni promesse mensongère) ; une description de \
trois à cinq phrases courtes qui donnent envie de regarder et disent ce qu'on y apprend (sans les chapitres, ajoutés à part) ; \
huit à douze mots-clés. Réponds en JSON.";
    let mut v = llm::appeler(etat, projet, consigne, &texte.chars().take(12000).collect::<String>(), &schema, false).await?;
    if !liste_chapitres.is_empty() {
        let d = v["description"].as_str().unwrap_or("").to_string();
        v["description"] = json!(format!("{d}\n\nChapitres :\n{liste_chapitres}"));
    }
    Ok(v)
}
