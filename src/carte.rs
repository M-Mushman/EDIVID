//! Carte graphique : EDIVID ne s'en sert qu'avec un bail du contrôleur d'admission d'azothd.
//!
//! - Travail ponctuel (transcription, netteté IA) : bail pris avant, rendu après.
//! - LLM : bail pris au premier appel, gardé tant que le modèle reste chargé dans Ollama ; après
//!   cinq minutes sans appel, EDIVID décharge le modèle puis rend le bail. Un travail GPU qui
//!   attend la carte fait décharger tout de suite un LLM inactif.

use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::config::Config;
use crate::etat::EtatPartage;

pub const VRAM_LLM_MO: u64 = 13_000;
pub const VRAM_TRANSCRIPTION_MO: u64 = 5_000;
pub const VRAM_NETTETE_MO: u64 = 3_000;
/// Débruiteur IA de la voix : pointe mesurée à 2,2 Go sur la carte (1,5 Go alloués) pour un rush de 11 min.
/// Reconstruction du timbre (VoiceFixer) : pointe mesurée à 4,7 Go sur la carte (2,9 Go alloués), rush de 11 min.
pub const VRAM_RESTAURATION_MO: u64 = 5_500;
/// Voix séchée par MossFormer2 (ClearerVoice) : 254 Mo mesurés sur un rush de 11 min (2026-10-27).
pub const VRAM_SECHAGE_MO: u64 = 1_000;
pub const VRAM_ENCODAGE_MO: u64 = 1_500;
const INACTIVITE_LLM: Duration = Duration::from_secs(300);
/// Un bail expire de lui-même à la fin de la durée annoncée : celui du LLM est renouvelé avant.
const DUREE_BAIL_LLM_S: u64 = 1800;
const RENOUVELLEMENT_LLM: Duration = Duration::from_secs(1500);

pub struct Bail {
    pub id: String,
}

struct EtatLlm {
    bail: Option<(Bail, Instant)>,
    dernier_appel: Instant,
    en_vol: usize,
}

pub struct Carte {
    http: reqwest::Client,
    azothd: String,
    ollama: String,
    modele: String,
    llm: Mutex<EtatLlm>,
}

impl Carte {
    pub fn nouvelle(cfg: &Config, http: reqwest::Client) -> Self {
        Self {
            http,
            azothd: cfg.azothd.clone(),
            ollama: cfg.ollama.clone(),
            modele: cfg.modele_llm.clone(),
            llm: Mutex::new(EtatLlm { bail: None, dernier_appel: Instant::now(), en_vol: 0 }),
        }
    }

    async fn demander(&self, genre: &str, vram_mo: u64, duree_s: u64) -> Result<std::result::Result<Bail, String>> {
        let reponse: Value = self
            .http
            .post(format!("{}/api/v1/admission/requests", self.azothd))
            .json(&json!({"charge": "other:EDIVID", "kind": genre, "vram_mb": vram_mo, "ram_mb": 4000,
                          "latency_class": "interactive", "estimated_duration_s": duree_s, "preemptible": false}))
            .timeout(Duration::from_secs(10))
            .send()
            .await?
            .json()
            .await?;
        match reponse["outcome"].as_str() {
            Some("granted") => Ok(Ok(Bail { id: reponse["lease"]["id"].as_str().unwrap_or_default().to_string() })),
            Some("denied") => Ok(Err(reponse["reason"].as_str().unwrap_or("refus").to_string())),
            _ => Err(anyhow!("réponse inattendue du contrôleur d'admission : {reponse}")),
        }
    }

    /// Prend un bail ; attend (en le disant dans l'interface) tant que la carte est occupée.
    pub async fn prendre(&self, etat: &EtatPartage, projet: &str, genre: &str, vram_mo: u64, duree_s: u64) -> Result<Bail> {
        let mut annonce = false;
        loop {
            self.liberer_llm_si_inactif().await;
            match self.demander(genre, vram_mo, duree_s).await {
                Ok(Ok(bail)) => return Ok(bail),
                Ok(Err(raison)) => {
                    if !annonce {
                        tracing::info!("carte refusée ({genre}) : {raison}");
                        annonce = true;
                    }
                    etat.emettre(projet, "progression",
                                 json!({"etape": "En attente de la carte graphique", "pct": 0, "detail": raison}));
                }
                Err(e) => {
                    etat.emettre(projet, "progression",
                                 json!({"etape": "En attente du contrôleur d'admission d'azoth", "pct": 0,
                                        "detail": e.to_string()}));
                }
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }

    /// Une seule demande, sans attendre : pour un confort (encodage matériel), pas un besoin.
    pub async fn essayer(&self, genre: &str, vram_mo: u64, duree_s: u64) -> Option<Bail> {
        match self.demander(genre, vram_mo, duree_s).await {
            Ok(Ok(bail)) => Some(bail),
            _ => None,
        }
    }

    pub async fn rendre(&self, bail: Bail) {
        let url = format!("{}/api/v1/admission/leases/{}/release", self.azothd, bail.id);
        if let Err(e) = self.http.post(url).timeout(Duration::from_secs(10)).send().await {
            tracing::warn!("bail {} non rendu : {e}", bail.id);
        }
    }

    /// À appeler avant chaque appel au LLM.
    pub async fn llm_debut(&self, etat: &EtatPartage, projet: &str) -> Result<()> {
        loop {
            {
                let mut l = self.llm.lock().await;
                let perime = l.bail.as_ref().map(|(_, pris)| pris.elapsed() > RENOUVELLEMENT_LLM).unwrap_or(false);
                if perime && l.en_vol == 0 {
                    if let Some((ancien, _)) = l.bail.take() {
                        self.rendre(ancien).await;
                    }
                }
                if l.bail.is_some() {
                    l.en_vol += 1;
                    return Ok(());
                }
            }
            let bail = self.prendre(etat, projet, "model_load", VRAM_LLM_MO, DUREE_BAIL_LLM_S).await?;
            let mut l = self.llm.lock().await;
            if l.bail.is_none() {
                l.bail = Some((bail, Instant::now()));
            } else {
                drop(l);
                self.rendre(bail).await;
            }
        }
    }

    pub async fn llm_fin(&self) {
        let mut l = self.llm.lock().await;
        l.en_vol = l.en_vol.saturating_sub(1);
        l.dernier_appel = Instant::now();
    }

    async fn decharger(&self, bail: Bail) {
        let _ = self
            .http
            .post(format!("{}/api/generate", self.ollama))
            .json(&json!({"model": self.modele, "keep_alive": 0}))
            .timeout(Duration::from_secs(60))
            .send()
            .await;
        self.rendre(bail).await;
        tracing::info!("LLM déchargé, bail rendu");
    }

    pub async fn liberer_llm_si_inactif(&self) {
        let bail = {
            let mut l = self.llm.lock().await;
            if l.en_vol == 0 { l.bail.take() } else { None }
        };
        if let Some((bail, _)) = bail {
            self.decharger(bail).await;
        }
    }

    /// À l'arrêt du service : décharge le LLM et rend son bail, même en plein appel.
    pub async fn tout_rendre(&self) {
        let bail = self.llm.lock().await.bail.take();
        if let Some((bail, _)) = bail {
            self.decharger(bail).await;
        }
    }

    pub async fn etat_llm(&self) -> Value {
        let l = self.llm.lock().await;
        json!({"charge": l.bail.is_some(), "en_vol": l.en_vol})
    }
}

/// Baux laissés par une instance précédente d'EDIVID (arrêt brutal) : rendus au démarrage.
async fn rendre_orphelins(etat: &EtatPartage) {
    let url = format!("{}/api/v1/admission/state", etat.carte.azothd);
    let Ok(reponse) = etat.http.get(url).timeout(Duration::from_secs(10)).send().await else { return };
    let Ok(v) = reponse.json::<Value>().await else { return };
    for bail in v["active_leases"].as_array().into_iter().flatten() {
        if bail["charge"] == "other:EDIVID" {
            if let Some(id) = bail["id"].as_str() {
                tracing::info!("bail orphelin {id} rendu");
                etat.carte.rendre(Bail { id: id.to_string() }).await;
            }
        }
    }
}

/// Veille : décharge le LLM après cinq minutes sans appel.
pub fn demarrer_veille(etat: EtatPartage) {
    tokio::spawn(async move {
        rendre_orphelins(&etat).await;
        loop {
            tokio::time::sleep(Duration::from_secs(20)).await;
            let bail = {
                let mut l = etat.carte.llm.lock().await;
                if l.en_vol == 0 && l.bail.is_some() && l.dernier_appel.elapsed() > INACTIVITE_LLM {
                    l.bail.take()
                } else {
                    None
                }
            };
            if let Some((bail, _)) = bail {
                etat.carte.decharger(bail).await;
            }
        }
    });
}
