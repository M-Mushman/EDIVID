//! File unique de travaux, exécutés un par un : la carte et le processeur ne sont jamais disputés
//! entre deux travaux d'EDIVID.

use std::sync::Mutex as MutexStd;

use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::json;
use tokio::sync::{mpsc, Mutex};

use crate::carte;
use crate::courriel;
use crate::decoupe;
use crate::drive;
use crate::etat::EtatPartage;
use crate::llm;
use crate::modele::{nouvel_id, EtatRush};
use crate::rendu;

#[derive(Clone, Debug, PartialEq)]
pub enum Genre {
    Preparer(String),
    Analyser(String),
    /// Version légère du rush pour le lecteur de l'interface.
    Proxy(String),
    Monter,
    Apercu,
    Rendu,
    /// Version exportée du rendu final (youtube, leger, max).
    Export(String),
    /// Quatre propositions de vignette (demande en JSON).
    Vignettes(String),
}

impl Genre {
    fn libelle(&self) -> &'static str {
        match self {
            Genre::Preparer(_) => "Préparation d'un rush",
            Genre::Analyser(_) => "Analyse d'un rush",
            Genre::Proxy(_) => "Vidéo d'origine pour le lecteur",
            Genre::Monter => "Montage par le LLM",
            Genre::Apercu => "Aperçu rapide",
            Genre::Rendu => "Vidéo finale",
            Genre::Export(_) => "Export",
            Genre::Vignettes(_) => "Vignettes YouTube",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Travail {
    pub id: String,
    pub projet: String,
    pub genre: Genre,
}

#[derive(Clone, Debug, Serialize)]
pub struct TravailVisible {
    pub id: String,
    pub projet: String,
    pub libelle: String,
    pub en_cours: bool,
}

pub struct File {
    tx: mpsc::UnboundedSender<Travail>,
    rx: Mutex<Option<mpsc::UnboundedReceiver<Travail>>>,
    attente: MutexStd<Vec<Travail>>,
    en_cours: MutexStd<Option<Travail>>,
}

impl File {
    pub fn nouvelle() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self { tx, rx: Mutex::new(Some(rx)), attente: MutexStd::new(vec![]), en_cours: MutexStd::new(None) }
    }

    /// Ajoute un travail ; un aperçu, un rendu ou un montage déjà en attente pour ce projet n'est pas doublé.
    pub fn ajouter(&self, etat: &EtatPartage, projet: &str, genre: Genre) {
        {
            let mut attente = self.attente.lock().unwrap();
            let unique = matches!(genre, Genre::Apercu | Genre::Rendu | Genre::Monter | Genre::Proxy(_) | Genre::Export(_) | Genre::Vignettes(_));
            if unique && attente.iter().any(|t| t.projet == projet && t.genre == genre) {
                return;
            }
            let t = Travail { id: nouvel_id(), projet: projet.into(), genre };
            attente.push(t.clone());
            let _ = self.tx.send(t);
        }
        etat.emettre(projet, "file", json!({}));
    }

    pub fn visibles(&self) -> Vec<TravailVisible> {
        let mut v = Vec::new();
        if let Some(t) = self.en_cours.lock().unwrap().as_ref() {
            v.push(TravailVisible { id: t.id.clone(), projet: t.projet.clone(), libelle: t.genre.libelle().into(), en_cours: true });
        }
        for t in self.attente.lock().unwrap().iter() {
            v.push(TravailVisible { id: t.id.clone(), projet: t.projet.clone(), libelle: t.genre.libelle().into(), en_cours: false });
        }
        v
    }

    /// Un travail de ce genre est-il en attente ou en cours pour ce projet ?
    pub fn occupe(&self, projet: &str) -> bool {
        self.en_cours.lock().unwrap().as_ref().map(|t| t.projet == projet).unwrap_or(false)
            || self.attente.lock().unwrap().iter().any(|t| t.projet == projet)
    }
}

pub fn demarrer(etat: EtatPartage) {
    tokio::spawn(async move {
        let mut rx = etat.file.rx.lock().await.take().expect("file déjà démarrée");
        while let Some(t) = rx.recv().await {
            {
                let mut attente = etat.file.attente.lock().unwrap();
                attente.retain(|x| x.id != t.id);
                *etat.file.en_cours.lock().unwrap() = Some(t.clone());
            }
            etat.emettre(&t.projet, "file", json!({}));
            let projet_existe = etat.projets.lock().await.contains_key(&t.projet);
            if projet_existe {
                if let Err(e) = executer(&etat, &t).await {
                    tracing::warn!("{} en échec : {e:#}", t.genre.libelle());
                    echec(&etat, &t, &format!("{e:#}")).await;
                }
            }
            *etat.file.en_cours.lock().unwrap() = None;
            etat.emettre(&t.projet, "file", json!({}));
        }
    });
}

async fn echec(etat: &EtatPartage, t: &Travail, message: &str) {
    if let Genre::Preparer(r) | Genre::Analyser(r) = &t.genre {
        let (r, m) = (r.clone(), message.to_string());
        let _ = etat
            .modifier(&t.projet, |p| {
                if let Some(rush) = p.rushs.iter_mut().find(|x| x.id == r) {
                    rush.etat = EtatRush::Erreur;
                    rush.erreur = Some(m);
                }
            })
            .await;
    }
    etat.emettre(&t.projet, "erreur", json!({"travail": t.genre.libelle(), "message": message}));
    if let Ok(p) = etat.projet(&t.projet).await {
        courriel::prevenir(etat, "erreur", &p, &format!("{} : {message}", t.genre.libelle())).await;
    }
}

/// Dépose le rendu sur le Drive (si demandé), puis prévient par courriel ; sans bloquer la file.
pub fn apres_rendu(etat: &EtatPartage, projet: &str, deposer: bool) {
    let (etat, projet) = (etat.clone(), projet.to_string());
    tokio::spawn(async move {
        if deposer {
            match drive::deposer(&etat, &projet).await {
                Ok(chemin) => etat.emettre(&projet, "drive", json!({"chemin": chemin})),
                Err(e) => {
                    let message = format!("dépôt sur le Google Drive : {e:#}");
                    etat.emettre(&projet, "erreur", json!({"travail": "Dépôt sur le Drive", "message": message}));
                    if let Ok(p) = etat.projet(&projet).await {
                        courriel::prevenir(&etat, "erreur", &p, &message).await;
                    }
                }
            }
        }
        if let Ok(p) = etat.projet(&projet).await {
            courriel::prevenir(&etat, "rendu", &p, "").await;
        }
    });
}

async fn etat_rush(etat: &EtatPartage, projet: &str, rush: &str, e: EtatRush) -> Result<()> {
    let r = rush.to_string();
    etat.modifier(projet, |p| {
        if let Some(x) = p.rushs.iter_mut().find(|x| x.id == r) {
            x.etat = e;
            x.erreur = None;
        }
    })
    .await
}

async fn executer(etat: &EtatPartage, t: &Travail) -> Result<()> {
    let projet = t.projet.as_str();
    match &t.genre {
        Genre::Vignettes(demande) => {
            crate::vignette::fabriquer(etat, projet, &serde_json::from_str(demande)?).await?;
        }
        Genre::Preparer(rush) => {
            let p = etat.projet(projet).await?;
            let Some(r) = p.rushs.iter().find(|x| &x.id == rush).cloned() else { return Ok(()) };
            etat_rush(etat, projet, rush, EtatRush::Preparation).await?;
            let source = etat.dossier_projet(projet).join("rushs").join(&r.fichier);
            let infos = rendu::sonder_rush(&source).await?;
            let dossier = etat.dossier_travail(projet, rush);
            tokio::fs::create_dir_all(&dossier).await?;
            rendu::preparer(etat, projet, &source, &infos, &dossier.join("mezz.mkv")).await?;
            let (id, infos2) = (rush.clone(), infos.clone());
            etat.modifier(projet, |p| {
                if let Some(x) = p.rushs.iter_mut().find(|x| x.id == id) {
                    x.etat = EtatRush::Prepare;
                    x.source = Some(infos2);
                }
            })
            .await?;
            etat.file.ajouter(etat, projet, Genre::Analyser(rush.clone()));
            etat.file.ajouter(etat, projet, Genre::Proxy(rush.clone()));
        }
        Genre::Proxy(rush) => {
            let p = etat.projet(projet).await?;
            let Some(r) = p.rushs.iter().find(|x| &x.id == rush) else { return Ok(()) };
            let dossier = etat.dossier_travail(projet, rush);
            let duree = r.source.as_ref().map(|s| s.duree).unwrap_or(0.0);
            if !dossier.join("proxy.mp4").exists() && dossier.join("mezz.mkv").exists() {
                rendu::proxy(etat, projet, &dossier.join("mezz.mkv"), &dossier.join("proxy.mp4"), duree).await?;
                etat.emettre(projet, "projet", json!({}));
            }
        }
        Genre::Analyser(rush) => {
            let p = etat.projet(projet).await?;
            if !p.rushs.iter().any(|x| &x.id == rush) {
                return Ok(());
            }
            etat_rush(etat, projet, rush, EtatRush::Analyse).await?;
            let dossier = etat.dossier_travail(projet, rush);
            let duree = p.rushs.iter().find(|x| &x.id == rush).and_then(|x| x.source.as_ref()).map(|s| s.duree).unwrap_or(600.0);
            let bail = etat
                .carte
                .prendre(etat, projet, "other:transcription", carte::VRAM_TRANSCRIPTION_MO, (duree * 0.5 + 180.0) as u64)
                .await?;
            let res = rendu::capteur(etat, projet, "analyse.py",
                                     &[dossier.join("mezz.mkv").to_string_lossy().into(), dossier.to_string_lossy().into()])
                .await;
            etat.carte.rendre(bail).await;
            res?;
            etat.oublier_analyse(projet, rush).await;
            etat_rush(etat, projet, rush, EtatRush::Pret).await?;
            // Chaîne automatique (projet venu du Drive) : le montage part dès que tout est analysé.
            let p = etat.projet(projet).await?;
            if p.auto.is_some() && p.montage.is_none() && p.rushs.iter().all(|r| r.etat == EtatRush::Pret) {
                etat.file.ajouter(etat, projet, Genre::Monter);
            }
        }
        Genre::Monter => {
            let p = etat.projet(projet).await?;
            if p.rushs.is_empty() {
                bail!("ajoutez au moins un rush");
            }
            if p.rushs.iter().any(|r| r.etat != EtatRush::Pret) {
                bail!("tous les rushs doivent être analysés avant le montage");
            }
            let analyses = etat.analyses_du_projet(&p).await?;
            let ps = decoupe::phrases_projet(&p, &analyses);
            if ps.is_empty() {
                bail!("aucune parole trouvée dans les rushs");
            }
            let montage = llm::monter(etat, &p, &ps).await?;
            etat.modifier(projet, |p| {
                p.montage = Some(montage);
                p.notifier_montage = true;
            })
            .await?;
            etat.file.ajouter(etat, projet, Genre::Apercu);
        }
        Genre::Export(preset) => {
            let ex = rendu::exporter(etat, projet, preset).await?;
            etat.modifier(projet, |p| p.exports.push(ex)).await?;
            etat.emettre(projet, "fini", json!({"genre": "export"}));
        }
        Genre::Apercu | Genre::Rendu => {
            let apercu = t.genre == Genre::Apercu;
            let sortie = rendu::rendre(etat, projet, apercu).await?;
            let ancien = etat
                .modifier(projet, |p| {
                    if apercu {
                        p.apercu = Some(sortie);
                        None
                    } else {
                        let ancien = p.rendu.replace(sortie).map(|s| s.fichier);
                        // La nouvelle vidéo finale remplace l'ancienne, sauf si un export est ce fichier même.
                        ancien.filter(|f| !p.exports.iter().any(|e| &e.fichier == f) && p.rendu.as_ref().is_some_and(|s| &s.fichier != f))
                    }
                })
                .await?;
            if let Some(f) = ancien {
                let _ = tokio::fs::remove_file(etat.dossier_projet(projet).join("sorties").join(f)).await;
            }
            etat.emettre(projet, "fini", json!({"genre": if apercu { "apercu" } else { "rendu" }}));
            let p = etat.projet(projet).await?;
            if apercu && p.notifier_montage {
                etat.modifier(projet, |p| p.notifier_montage = false).await?;
                match &p.auto {
                    Some(a) if a.rendu_final => etat.file.ajouter(etat, projet, Genre::Rendu),
                    _ => courriel::prevenir(etat, "montage", &p, "").await,
                }
            } else if !apercu {
                let deposer = p.auto.as_ref().map(|a| a.deposer_drive).unwrap_or(false);
                apres_rendu(etat, projet, deposer);
            }
        }
    }
    Ok(())
}

/// Au démarrage : les rushs préparés avant la v0.3 reçoivent leur version pour le lecteur.
pub async fn rattraper_proxys(etat: &EtatPartage) {
    let projets: Vec<(String, Vec<String>)> = etat
        .projets
        .lock()
        .await
        .values()
        .map(|p| (p.id.clone(), p.rushs.iter().map(|r| r.id.clone()).collect()))
        .collect();
    for (pid, rushs) in projets {
        for rid in rushs {
            let d = etat.dossier_travail(&pid, &rid);
            if d.join("mezz.mkv").exists() && !d.join("proxy.mp4").exists() {
                etat.file.ajouter(etat, &pid, Genre::Proxy(rid));
            }
        }
    }
}
