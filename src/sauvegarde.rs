//! Sauvegarde d'un projet en zip téléchargé par le navigateur, suppression qui verse ses médias dans la bibliothèque
//! de son propriétaire, et réimport d'une sauvegarde. Le zip lui-même est l'affaire d'`outils/sauvegarde.py` : il
//! garde l'essentiel (rushs, médias, analyses, vidéo finale) et laisse ce qui se recalcule (vidéo de travail).

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{anyhow, bail, Context, Result};
use axum::body::Body;
use axum::http::{header, HeaderValue};
use axum::response::Response;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::process::Command;
use tokio_util::io::ReaderStream;

use crate::etat::EtatPartage;
use crate::modele::{maintenant, nouvel_id, EtatRush, Projet, Sauvegarde};
use crate::travaux::Genre;

fn script(etat: &EtatPartage) -> PathBuf {
    etat.cfg.racine.join("outils").join("sauvegarde.py")
}

/// Empreinte de ce qu'une sauvegarde contient : une sauvegarde dont l'empreinte diffère date d'avant des changements.
pub fn empreinte(p: &Projet) -> String {
    let v = json!([crate::rendu::empreinte_finale(p), p.rendu.as_ref().map(|r| &r.fichier), p.fiche, p.nom,
                   p.medias.iter().map(|m| &m.fichier).collect::<Vec<_>>()]);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.to_string().hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Ce que la fenêtre de sauvegarde affiche : taille du zip, dernière sauvegarde, est-elle à jour, médias à verser.
pub async fn infos(etat: &EtatPartage, id: &str) -> Result<Value> {
    let p = etat.projet(id).await?;
    let sortie = Command::new(&etat.cfg.python).arg(script(etat)).arg("taille").arg(etat.dossier_projet(id))
        .stdin(Stdio::null()).output().await.context("python introuvable")?;
    let taille: Value = serde_json::from_slice(&sortie.stdout).unwrap_or(json!({}));
    Ok(json!({
        "taille_zip": taille["octets"], "sauvegarde": p.sauvegarde,
        "a_jour": p.sauvegarde.as_ref().is_some_and(|s| s.empreinte == empreinte(&p)),
        "medias": p.medias.len(), "proprietaire": p.proprietaire,
    }))
}

fn nom_de_fichier(nom: &str) -> String {
    let n: String = nom.chars().map(|c| if c.is_alphanumeric() || " -_().".contains(c) { c } else { '_' }).collect();
    let n = n.trim().trim_start_matches('.').to_string();
    if n.is_empty() { "projet".into() } else { n }
}

/// Le zip, écrit à la volée vers le navigateur. Une fois envoyé en entier, la sauvegarde est notée dans le projet ;
/// un téléchargement interrompu arrête l'écriture et ne note rien.
pub async fn telecharger(etat: &EtatPartage, id: &str) -> Result<Response> {
    let p = etat.projet(id).await?;
    let mut enfant = Command::new(&etat.cfg.python).arg(script(etat)).arg("zip").arg(etat.dossier_projet(id))
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true)
        .spawn().context("python introuvable")?;
    let sortie = enfant.stdout.take().ok_or_else(|| anyhow!("sortie du zip indisponible"))?;
    let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<axum::body::Bytes>>(8);
    let (etat2, id2, empreinte) = (etat.clone(), id.to_string(), empreinte(&p));
    tokio::spawn(async move {
        let mut flux = ReaderStream::with_capacity(sortie, 1 << 20);
        let mut total = 0u64;
        while let Some(morceau) = flux.next().await {
            let fin = morceau.is_err();
            if let Ok(m) = &morceau {
                total += m.len() as u64;
            }
            if tx.send(morceau).await.is_err() || fin {
                // Navigateur parti, ou lecture en échec : on arrête tout, rien n'est noté.
                let _ = enfant.kill().await;
                return;
            }
        }
        let fini = enfant.wait_with_output().await;
        match fini {
            Ok(o) if o.status.success() => {
                let s = Sauvegarde { quand: maintenant(), taille: total, empreinte };
                let _ = etat2.modifier(&id2, |p| p.sauvegarde = Some(s)).await;
            }
            Ok(o) => {
                let e = String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("").to_string();
                tracing::warn!("sauvegarde de {id2} en échec : {e}");
                let _ = tx.send(Err(std::io::Error::other(e))).await;
            }
            Err(e) => {
                let _ = tx.send(Err(std::io::Error::other(e.to_string()))).await;
            }
        }
    });
    let nom = format!("{} — EDIVID.zip", nom_de_fichier(&p.nom));
    let encode: String = nom.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    let mut r = Response::new(Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx)));
    r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    r.headers_mut().insert(header::CONTENT_DISPOSITION,
                           HeaderValue::from_str(&format!("attachment; filename=\"projet.zip\"; filename*=UTF-8''{encode}"))?);
    Ok(r)
}

/// Supprime un projet ; ses médias (logos, vidéos, sons ajoutés à la main) rejoignent d'abord le rayon « medias »
/// de la bibliothèque de son propriétaire. Renvoie le nombre de médias versés.
pub async fn supprimer(etat: &EtatPartage, id: &str) -> Result<usize> {
    if etat.file.occupe(id) {
        bail!("un travail est en cours sur ce projet : attendez sa fin");
    }
    let p = etat.projet(id).await?;
    let biblio = crate::rayons::racine(&etat.cfg.donnees, &p.proprietaire);
    let medias = etat.dossier_projet(id).join("medias");
    let mut verses = 0;
    for m in &p.medias {
        let source = medias.join(&m.fichier);
        if source.is_file() {
            crate::rayons::verser(&biblio, &source, &m.nom).with_context(|| format!("média « {} » non versé : projet gardé", m.nom))?;
            verses += 1;
        }
    }
    etat.projets.lock().await.remove(id).ok_or_else(|| anyhow!("projet introuvable"))?;
    let _ = tokio::fs::remove_dir_all(etat.dossier_projet(id)).await;
    Ok(verses)
}

/// Réimporte une sauvegarde (zip déjà reçu dans `archive`) comme un nouveau projet ; la vidéo de travail de chaque rush
/// se refait sur le processeur, l'analyse (transcription) est reprise du zip. Renvoie l'identifiant du projet.
pub async fn importer(etat: &EtatPartage, archive: &Path) -> Result<String> {
    let id = nouvel_id();
    let dossier = etat.dossier_projet(&id);
    let deballe = Command::new(&etat.cfg.python).arg(script(etat)).arg("deballer").arg(archive).arg(&dossier)
        .stdin(Stdio::null()).output().await.context("python introuvable")?;
    if !deballe.status.success() {
        let _ = tokio::fs::remove_dir_all(&dossier).await;
        bail!("{}", String::from_utf8_lossy(&deballe.stderr).lines().last().unwrap_or("zip illisible"));
    }
    let lu = async {
        let mut p: Projet = serde_json::from_slice(&tokio::fs::read(dossier.join("projet.json")).await?)?;
        p.id = id.clone();
        p.apercu = None;
        p.exports.clear();
        p.sauvegarde = None;
        p.auto = None;
        p.depot_drive = None;
        if p.rendu.as_ref().is_some_and(|r| !dossier.join("sorties").join(&r.fichier).is_file()) {
            p.rendu = None;
        }
        if p.proprietaire.is_empty() {
            p.proprietaire = etat.proprietaire_par_defaut().await;
        }
        for r in p.rushs.iter_mut() {
            r.etat = EtatRush::Preparation;
            r.erreur = None;
        }
        Ok::<Projet, anyhow::Error>(p)
    }
    .await;
    let p = match lu {
        Ok(p) => p,
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&dossier).await;
            return Err(e.context("projet.json de la sauvegarde illisible"));
        }
    };
    etat.enregistrer(&p).await?;
    let rushs: Vec<String> = p.rushs.iter().map(|r| r.id.clone()).collect();
    etat.projets.lock().await.insert(id.clone(), p);
    for r in rushs {
        etat.file.ajouter(etat, &id, Genre::Restaurer(r));
    }
    Ok(id)
}
