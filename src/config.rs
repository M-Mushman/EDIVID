//! Configuration : variables d'environnement, avec des valeurs par défaut adaptées à azoth.
//! Le code d'accès est tiré au premier lancement et gardé dans `<donnees>/acces.json`.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub struct Config {
    /// Dossier du dépôt : `capteurs/`, `modeles/`, `web/`, `.venv/`.
    pub racine: PathBuf,
    /// Projets, médias, rendus.
    pub donnees: PathBuf,
    pub ecoute: Vec<SocketAddr>,
    /// Écoute réservée au relais HTTPS du tailnet (`tailscale serve`) : la seule où l'identité Tailscale fait foi.
    pub ecoute_relais: Option<SocketAddr>,
    pub ollama: String,
    pub modele_llm: String,
    /// azothd, pour les baux de carte graphique.
    pub azothd: String,
    pub python: PathBuf,
    /// Python du débruiteur IA (environnement à part : le paquet fige d'anciennes versions).
    pub python_rehaussement: PathBuf,
    /// Python de la reconstruction du timbre (VoiceFixer, environnement à part).
    pub python_voicefixer: PathBuf,
    /// Python de la boîte à outils du son (séparation voix/réverbération, DNSMOS, égaliseur ; `.venv-son`).
    pub python_son: PathBuf,
    /// Python de la voix séchée par IA (MossFormer2, ClearerVoice ; `.venv-clearvoice`).
    pub python_clearvoice: PathBuf,
    /// Python du débruiteur DeepFilterNet3 (`.venv-deepfilter`, torch pour le processeur).
    pub python_deepfilter: PathBuf,
    /// Python de TF-GridNet (`.venv-gridnet`, espnet).
    pub python_gridnet: PathBuf,
    pub code_acces: String,
    /// Valeur du témoin de session, dérivée du code.
    pub jeton: String,
    /// Secret de signature des sessions ouvertes par Google.
    pub secret: String,
}

#[derive(Serialize, Deserialize)]
struct Acces {
    code: String,
    sel: String,
}

fn env(cle: &str, defaut: &str) -> String {
    std::env::var(cle).unwrap_or_else(|_| defaut.to_string())
}

fn tirer(alphabet: &[u8], n: usize) -> String {
    let mut rng = rand::rng();
    (0..n).map(|_| alphabet[rng.random_range(0..alphabet.len())] as char).collect()
}

impl Config {
    pub fn charger() -> Result<Self> {
        let racine = PathBuf::from(env("EDIVID_RACINE", &std::env::current_dir()?.to_string_lossy()));
        let donnees = PathBuf::from(env("EDIVID_DONNEES", &racine.join("donnees").to_string_lossy()));
        std::fs::create_dir_all(donnees.join("projets"))?;
        let ecoute = env("EDIVID_ECOUTE", "127.0.0.1:8030,100.124.48.19:8030")
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().parse().with_context(|| format!("adresse d'écoute invalide : {s}")))
            .collect::<Result<Vec<_>>>()?;
        let ecoute_relais: Option<SocketAddr> = match env("EDIVID_ECOUTE_RELAIS", "127.0.0.1:8031").trim() {
            "" => None,
            a => Some(a.parse().with_context(|| format!("adresse du relais invalide : {a}"))?),
        };
        let mut ecoute = ecoute;
        if let Some(r) = ecoute_relais.filter(|r| !ecoute.contains(r)) {
            ecoute.push(r);
        }
        let chemin_acces = donnees.join("acces.json");
        let acces: Acces = match std::fs::read_to_string(&chemin_acces) {
            Ok(t) => {
                // Créé avant les écritures réservées (droits 664) : ramené à 600.
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&chemin_acces, std::fs::Permissions::from_mode(0o600));
                serde_json::from_str(&t)?
            }
            Err(_) => {
                // Sans lettres ni chiffres qui se confondent (O/0, I/1/l).
                let alphabet = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
                let a = Acces {
                    code: format!("{}-{}", tirer(alphabet, 4), tirer(alphabet, 4)),
                    sel: tirer(b"abcdefghijklmnopqrstuvwxyz0123456789", 32),
                };
                crate::fichiers::ecrire(&chemin_acces, serde_json::to_string_pretty(&a)?.as_bytes(), true)?;
                a
            }
        };
        let jeton = format!("{:x}", Sha256::digest(format!("{}:{}", acces.sel, acces.code)));
        Ok(Self {
            python: racine.join(".venv/bin/python"),
            python_rehaussement: racine.join(".venv-rehaussement/bin/python"),
            python_voicefixer: racine.join(".venv-voicefixer/bin/python"),
            python_son: racine.join(".venv-son/bin/python"),
            python_clearvoice: racine.join(".venv-clearvoice/bin/python"),
            python_deepfilter: racine.join(".venv-deepfilter/bin/python"),
            python_gridnet: racine.join(".venv-gridnet/bin/python"),
            racine,
            donnees,
            ecoute,
            ecoute_relais,
            ollama: env("EDIVID_OLLAMA", "http://127.0.0.1:11434"),
            modele_llm: env("EDIVID_LLM", "qwen3.5:35b-a3b-q4_K_M"),
            azothd: env("EDIVID_AZOTHD", "http://127.0.0.1:8016"),
            code_acces: acces.code,
            jeton,
            secret: acces.sel,
        })
    }
}
