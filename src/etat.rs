//! État partagé du serveur : projets en mémoire et sur disque, analyses en cache, événements.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::{broadcast, Mutex};

use crate::carte::Carte;
use crate::config::Config;
use crate::drive::Veille;
use crate::parametres::{self, Parametres};
use crate::modele::{Analyse, Projet};
use crate::reglages;
use crate::travaux::File;

#[derive(Clone, Debug, Serialize)]
pub struct Evenement {
    pub projet: String,
    /// progression, projet, erreur, file.
    pub genre: String,
    pub donnees: Value,
}

pub struct Etat {
    pub cfg: Config,
    pub projets: Mutex<HashMap<String, Projet>>,
    analyses: Mutex<HashMap<String, Arc<Analyse>>>,
    pub evenements: broadcast::Sender<Evenement>,
    pub file: File,
    pub carte: Carte,
    pub http: reqwest::Client,
    pub parametres: tokio::sync::RwLock<Parametres>,
    pub drive: Veille,
    /// Encodeur matériel NVIDIA utilisable (paquet libnvidia-encode présent).
    pub nvenc: bool,
    /// Propositions de correction en attente de validation (commentaires sur un instant).
    pub propositions: std::sync::Mutex<HashMap<String, Value>>,
    /// Allers-retours en cours avec Google (connexion).
    pub google: crate::google::Demandes,
}

pub type EtatPartage = Arc<Etat>;

impl Etat {
    pub async fn nouveau(cfg: Config, nvenc: bool) -> Result<EtatPartage> {
        let mut projets = HashMap::new();
        let dossier = cfg.donnees.join("projets");
        for entree in std::fs::read_dir(&dossier)? {
            let chemin = entree?.path().join("projet.json");
            if let Ok(texte) = std::fs::read_to_string(&chemin) {
                match serde_json::from_str::<Projet>(&texte) {
                    Ok(mut p) => {
                        reglages::completer(&mut p.reglages);
                        projets.insert(p.id.clone(), p);
                    }
                    Err(e) => tracing::warn!("projet illisible {}: {e}", chemin.display()),
                }
            }
        }
        let (evenements, _) = broadcast::channel(512);
        let donnees = cfg.donnees.clone();
        let http = reqwest::Client::builder().build()?;
        Ok(Arc::new(Self {
            carte: Carte::nouvelle(&cfg, http.clone()),
            cfg,
            projets: Mutex::new(projets),
            analyses: Mutex::new(HashMap::new()),
            evenements,
            file: File::nouvelle(),
            http,
            parametres: tokio::sync::RwLock::new(parametres::charger(&donnees)),
            drive: Veille::nouvelle(),
            nvenc,
            propositions: Default::default(),
            google: Default::default(),
        }))
    }

    pub fn dossier_projet(&self, id: &str) -> PathBuf {
        self.cfg.donnees.join("projets").join(id)
    }

    /// Fichier de travail, analyse et enveloppe d'un rush.
    pub fn dossier_travail(&self, projet: &str, rush: &str) -> PathBuf {
        self.dossier_projet(projet).join("travail").join(rush)
    }

    pub async fn projet(&self, id: &str) -> Result<Projet> {
        self.projets
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow!("projet introuvable"))
    }

    pub async fn enregistrer(&self, p: &Projet) -> Result<()> {
        let dossier = self.dossier_projet(&p.id);
        tokio::fs::create_dir_all(&dossier).await?;
        let tmp = dossier.join("projet.json.tmp");
        tokio::fs::write(&tmp, serde_json::to_vec_pretty(p)?).await?;
        tokio::fs::rename(&tmp, dossier.join("projet.json")).await?;
        Ok(())
    }

    /// Modifie un projet, l'enregistre et prévient les navigateurs ouverts dessus.
    pub async fn modifier<R>(&self, id: &str, f: impl FnOnce(&mut Projet) -> R) -> Result<R> {
        let (resultat, copie) = {
            let mut projets = self.projets.lock().await;
            let p = projets.get_mut(id).ok_or_else(|| anyhow!("projet introuvable"))?;
            let r = f(p);
            (r, p.clone())
        };
        self.enregistrer(&copie).await?;
        self.emettre(id, "projet", json!({}));
        Ok(resultat)
    }

    pub fn emettre(&self, projet: &str, genre: &str, donnees: Value) {
        let _ = self.evenements.send(Evenement { projet: projet.into(), genre: genre.into(), donnees });
    }

    pub fn signaler(&self, projet: &str, etape: &str, pct: f64) {
        self.emettre(projet, "progression", json!({"etape": etape, "pct": (pct * 10.0).round() / 10.0}));
    }

    pub async fn analyse(&self, projet: &str, rush: &str) -> Result<Arc<Analyse>> {
        let cle = format!("{projet}/{rush}");
        if let Some(a) = self.analyses.lock().await.get(&cle) {
            return Ok(a.clone());
        }
        let dossier = self.dossier_travail(projet, rush);
        let texte = tokio::fs::read_to_string(dossier.join("analyse.json"))
            .await
            .context("analyse absente : le rush n'est pas encore analysé")?;
        let mut a: Analyse = serde_json::from_str(&texte)?;
        let octets = tokio::fs::read(dossier.join("enveloppe.f32")).await.unwrap_or_default();
        a.enveloppe = octets.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let a = Arc::new(a);
        self.analyses.lock().await.insert(cle, a.clone());
        Ok(a)
    }

    pub async fn oublier_analyse(&self, projet: &str, rush: &str) {
        self.analyses.lock().await.remove(&format!("{projet}/{rush}"));
    }

    /// Analyses de tous les rushs prêts, dans l'ordre du projet.
    pub async fn analyses_du_projet(&self, p: &Projet) -> Result<Vec<(String, Arc<Analyse>)>> {
        let mut res = Vec::new();
        for r in &p.rushs {
            if r.etat == crate::modele::EtatRush::Pret {
                res.push((r.id.clone(), self.analyse(&p.id, &r.id).await?));
            }
        }
        Ok(res)
    }
}
