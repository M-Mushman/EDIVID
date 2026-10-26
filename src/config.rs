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
    pub ollama: String,
    pub modele_llm: String,
    /// azothd, pour les baux de carte graphique.
    pub azothd: String,
    pub python: PathBuf,
    /// Python du débruiteur IA (environnement à part : le paquet fige d'anciennes versions).
    pub python_rehaussement: PathBuf,
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
        let chemin_acces = donnees.join("acces.json");
        let acces: Acces = match std::fs::read_to_string(&chemin_acces) {
            Ok(t) => serde_json::from_str(&t)?,
            Err(_) => {
                // Sans lettres ni chiffres qui se confondent (O/0, I/1/l).
                let alphabet = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
                let a = Acces {
                    code: format!("{}-{}", tirer(alphabet, 4), tirer(alphabet, 4)),
                    sel: tirer(b"abcdefghijklmnopqrstuvwxyz0123456789", 32),
                };
                std::fs::write(&chemin_acces, serde_json::to_string_pretty(&a)?)?;
                a
            }
        };
        let jeton = format!("{:x}", Sha256::digest(format!("{}:{}", acces.sel, acces.code)));
        Ok(Self {
            python: racine.join(".venv/bin/python"),
            python_rehaussement: racine.join(".venv-rehaussement/bin/python"),
            racine,
            donnees,
            ecoute,
            ollama: env("EDIVID_OLLAMA", "http://127.0.0.1:11434"),
            modele_llm: env("EDIVID_LLM", "qwen3.5:35b-a3b-q4_K_M"),
            azothd: env("EDIVID_AZOTHD", "http://127.0.0.1:8016"),
            code_acces: acces.code,
            jeton,
            secret: acces.sel,
        })
    }
}
