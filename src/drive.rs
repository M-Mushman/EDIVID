//! Google Drive, par rclone : veille du dossier d'entrée (un fichier ou un sous-dossier = un projet),
//! puis dépôt du rendu final dans le dossier de sortie.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::sync::Notify;

use crate::etat::EtatPartage;
use crate::modele::{maintenant, nouvel_id, Auto, EtatRush, Projet, Rush};
use crate::parametres::Drive;
use crate::reglages;
use crate::travaux::Genre;

pub const PRIS_EN_CHARGE: &str = "Pris en charge";
const EXTENSIONS: &[&str] = &["mov", "mp4", "m4v", "mkv", "avi", "hevc", "3gp", "webm", "mts"];

#[derive(Default, Serialize, Clone)]
pub struct EtatDrive {
    pub dernier_passage: Option<String>,
    pub message: String,
    pub erreur: Option<String>,
    pub en_cours: Option<String>,
}

pub struct Veille {
    pub etat: std::sync::Mutex<EtatDrive>,
    pub reveil: Notify,
    /// Tailles vues au passage précédent : un envoi est fini quand elles ne bougent plus.
    vus: std::sync::Mutex<HashMap<String, u64>>,
}

impl Veille {
    pub fn nouvelle() -> Self {
        Self { etat: Default::default(), reveil: Notify::new(), vus: Default::default() }
    }

    fn dire(&self, message: impl Into<String>, erreur: Option<String>) {
        let mut e = self.etat.lock().unwrap();
        e.message = message.into();
        e.erreur = erreur;
    }

    fn occupe(&self, quoi: Option<String>) {
        self.etat.lock().unwrap().en_cours = quoi;
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Registre {
    /// Fichiers déjà pris (chemin + taille), au cas où leur déplacement aurait échoué.
    pris: HashSet<String>,
}

pub fn rclone() -> PathBuf {
    if let Ok(r) = std::env::var("EDIVID_RCLONE") {
        return PathBuf::from(r);
    }
    let local = std::env::var("HOME").map(|h| PathBuf::from(h).join(".local/bin/rclone")).unwrap_or_default();
    if local.exists() { local } else { PathBuf::from("rclone") }
}

/// Chemin rclone d'un dossier : « gdrive:EDIVID/À monter », ou un dossier local pour les essais.
pub fn chemin(distant: &str, dossier: &str) -> String {
    if distant.ends_with(':') {
        format!("{distant}{dossier}")
    } else {
        format!("{}/{}", distant.trim_end_matches('/'), dossier)
    }
}

async fn rclone_cmd(args: &[&str]) -> Result<String> {
    let sortie = Command::new(rclone())
        .args(args)
        .output()
        .await
        .context("rclone introuvable : installez-le (voir Paramètres)")?;
    if !sortie.status.success() {
        let err = String::from_utf8_lossy(&sortie.stderr);
        let ligne = err.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("échec").to_string();
        bail!("{ligne}");
    }
    Ok(String::from_utf8_lossy(&sortie.stdout).to_string())
}

/// Vérifie la liaison et crée les dossiers s'il le faut.
pub async fn verifier(d: &Drive) -> Result<String> {
    if d.distant.ends_with(':') {
        let distants = rclone_cmd(&["listremotes"]).await?;
        if !distants.lines().any(|l| l.trim() == d.distant) {
            bail!("le Drive « {} » n'est pas relié : reliez à nouveau le compte Google dans Paramètres", d.distant);
        }
    }
    for dossier in [&d.entree, &d.sortie] {
        rclone_cmd(&["mkdir", &chemin(&d.distant, dossier)]).await?;
    }
    rclone_cmd(&["mkdir", &chemin(&d.distant, &format!("{}/{PRIS_EN_CHARGE}", d.entree))]).await?;
    let liste = lister(d).await?;
    Ok(format!("Liaison correcte. Dossier « {} » : {} vidéo(s) en attente.", d.entree, liste.len()))
}

#[derive(Debug, Clone)]
struct Fichier {
    chemin: String,
    taille: u64,
}

async fn lister(d: &Drive) -> Result<Vec<Fichier>> {
    let brut = rclone_cmd(&["lsjson", "--files-only", "-R", "--max-depth", "2", &chemin(&d.distant, &d.entree)]).await?;
    let v: Vec<Value> = serde_json::from_str(&brut)?;
    Ok(v.iter()
        .filter_map(|f| {
            let chemin = f["Path"].as_str()?.to_string();
            if chemin.starts_with(PRIS_EN_CHARGE) {
                return None;
            }
            let ext = chemin.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
            EXTENSIONS.contains(&ext.as_str()).then(|| Fichier { chemin, taille: f["Size"].as_u64().unwrap_or(0) })
        })
        .collect())
}

fn charger_registre(etat: &EtatPartage) -> Registre {
    std::fs::read_to_string(etat.cfg.donnees.join("drive.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn enregistrer_registre(etat: &EtatPartage, r: &Registre) {
    let _ = std::fs::write(etat.cfg.donnees.join("drive.json"), serde_json::to_vec_pretty(r).unwrap_or_default());
}

/// Les Drive à surveiller : celui de chaque compte relié qui l'a demandé, et le Drive réglé à la main.
async fn sources(etat: &EtatPartage) -> Vec<(Drive, String)> {
    let p = etat.parametres.read().await.clone();
    let mut v: Vec<(Drive, String)> = p
        .comptes
        .iter()
        .filter(|c| c.veille && !c.distant.is_empty())
        .map(|c| (Drive { distant: c.distant.clone(), ..p.drive.clone() }, c.email.clone()))
        .collect();
    if p.drive.actif && !v.iter().any(|(d, _)| d.distant == p.drive.distant) {
        v.push((p.drive.clone(), String::new()));
    }
    v
}

/// Un passage de veille sur tous les Drive surveillés.
pub async fn passage(etat: &EtatPartage) -> Result<()> {
    let mut derniere_erreur = None;
    for (d, compte) in sources(etat).await {
        if let Err(e) = passage_source(etat, &d, &compte).await {
            derniere_erreur = Some(e);
        }
    }
    match derniere_erreur {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Un passage sur un Drive : prend les envois terminés et crée leurs projets.
async fn passage_source(etat: &EtatPartage, d: &Drive, compte: &str) -> Result<()> {
    let d = d.clone();
    let fichiers = match lister(&d).await {
        Ok(f) => f,
        // Dossier d'entrée absent (premier usage, ou supprimé) : on le recrée, il est vide.
        Err(e) if e.to_string().contains("directory not found") => {
            rclone_cmd(&["mkdir", &chemin(&d.distant, &format!("{}/{PRIS_EN_CHARGE}", d.entree))]).await?;
            Vec::new()
        }
        Err(e) => return Err(e),
    };
    let mut registre = charger_registre(etat);
    // Tailles vues au passage précédent, pour ce Drive seulement.
    let prefixe = format!("{}#", d.distant);
    let precedents: HashMap<String, u64> = {
        let mut vus = etat.drive.vus.lock().unwrap();
        let (miens, autres): (HashMap<_, _>, HashMap<_, _>) = vus.drain().partition(|(k, _)| k.starts_with(&prefixe));
        *vus = autres;
        miens.into_iter().map(|(k, v)| (k[prefixe.len()..].to_string(), v)).collect()
    };
    let mut groupes: BTreeMap<String, Vec<Fichier>> = BTreeMap::new();
    let mut maintenant_vus = HashMap::new();
    for f in fichiers {
        maintenant_vus.insert(format!("{prefixe}{}", f.chemin), f.taille);
        if registre.pris.contains(&format!("{}#{}", f.chemin, f.taille)) || registre.pris.contains(&format!("{prefixe}{}#{}", f.chemin, f.taille)) {
            continue;
        }
        let groupe = match f.chemin.split_once('/') {
            Some((dossier, _)) => dossier.to_string(),
            None => f.chemin.clone(),
        };
        groupes.entry(groupe).or_default().push(f);
    }
    etat.drive.vus.lock().unwrap().extend(maintenant_vus);
    let mut attente = 0;
    for (groupe, fichiers) in groupes {
        let stable = fichiers.iter().all(|f| precedents.get(&f.chemin) == Some(&f.taille) && f.taille > 0);
        if !stable {
            attente += 1;
            continue;
        }
        etat.drive.occupe(Some(format!("Téléchargement de « {groupe} »")));
        let res = prendre(etat, &d, compte, &groupe, &fichiers, &mut registre).await;
        etat.drive.occupe(None);
        enregistrer_registre(etat, &registre);
        if let Err(e) = res {
            etat.drive.dire(format!("« {groupe} » non pris"), Some(format!("{e:#}")));
            return Err(e);
        }
    }
    etat.drive.dire(
        if attente > 0 { format!("{attente} envoi(s) en cours de dépôt sur le Drive : ils seront pris au prochain passage.") }
        else { "Rien de nouveau dans le dossier d'entrée.".to_string() },
        None,
    );
    Ok(())
}

fn nom_de_groupe(groupe: &str) -> String {
    let nom = groupe.rsplit_once('.').filter(|(_, e)| EXTENSIONS.contains(&e.to_lowercase().as_str())).map(|(n, _)| n).unwrap_or(groupe);
    nom.replace(['_', '-'], " ").trim().to_string()
}

async fn prendre(etat: &EtatPartage, d: &Drive, compte: &str, groupe: &str, fichiers: &[Fichier], registre: &mut Registre)
                 -> Result<()> {
    let modele = if d.modele.is_empty() { None } else { etat.projet(&d.modele).await.ok() };
    let id = nouvel_id();
    let mut p = Projet {
        id: id.clone(),
        nom: nom_de_groupe(groupe),
        cree: maintenant(),
        rushs: vec![],
        medias: vec![],
        reglages: modele.as_ref().map(|m| m.reglages.clone()).unwrap_or_else(reglages::defauts),
        montage: None,
        incrustations: vec![],
        discussion: vec![],
        apercu: None,
        rendu: None,
        consignes: modele.as_ref().map(|m| m.consignes.clone()).unwrap_or_default(),
        auto: Some(Auto { rendu_final: d.rendu_final, deposer_drive: d.deposer }),
        notifier_montage: false,
        depot_drive: None,
        origine: "Google Drive".into(),
        compte: compte.to_string(),
        postprod: modele.as_ref().map(|m| m.postprod.clone()).unwrap_or_default(),
        corrections: Default::default(),
        exports: vec![],
    };
    let mut tries: Vec<&Fichier> = fichiers.iter().collect();
    tries.sort_by(|a, b| a.chemin.cmp(&b.chemin));
    let dossier_rushs = etat.dossier_projet(&id).join("rushs");
    tokio::fs::create_dir_all(&dossier_rushs).await?;
    for f in &tries {
        let rid = nouvel_id();
        let nom = f.chemin.rsplit('/').next().unwrap_or(&f.chemin).to_string();
        let ext = nom.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_else(|| "mov".into());
        let fichier = format!("{rid}.{ext}");
        let local = dossier_rushs.join(&fichier);
        let source = chemin(&d.distant, &format!("{}/{}", d.entree, f.chemin));
        rclone_cmd(&["copyto", &source, &local.to_string_lossy()]).await.with_context(|| format!("téléchargement de {nom}"))?;
        p.rushs.push(Rush { id: rid, nom, fichier, taille: f.taille, etat: EtatRush::Televerse, erreur: None, source: None });
    }
    // Le projet existe ; les fichiers du Drive sont rangés pour ne jamais être repris.
    etat.enregistrer(&p).await?;
    let rushs: Vec<String> = p.rushs.iter().map(|r| r.id.clone()).collect();
    etat.projets.lock().await.insert(id.clone(), p);
    for r in rushs {
        etat.file.ajouter(etat, &id, Genre::Preparer(r));
    }
    for f in &tries {
        registre.pris.insert(format!("{}#{}#{}", d.distant, f.chemin, f.taille));
        let source = chemin(&d.distant, &format!("{}/{}", d.entree, f.chemin));
        let cible = chemin(&d.distant, &format!("{}/{PRIS_EN_CHARGE}/{}", d.entree, f.chemin));
        if let Err(e) = rclone_cmd(&["moveto", &source, &cible]).await {
            tracing::warn!("{} non déplacé sur le Drive : {e:#}", f.chemin);
        }
    }
    tracing::info!("Drive : projet {id} créé depuis « {groupe} »");
    etat.drive.dire(format!("« {groupe} » pris en charge"), None);
    Ok(())
}

/// Dépose le rendu final dans le dossier de sortie ; renvoie son chemin sur le Drive.
pub async fn deposer(etat: &EtatPartage, projet: &str) -> Result<String> {
    let p = etat.projet(projet).await?;
    // Le rendu retourne au Drive d'où vient le projet ; sinon, au Drive réglé à la main.
    let d = {
        let params = etat.parametres.read().await;
        let mut d = params.drive.clone();
        if let Some(c) = params.comptes.iter().find(|c| !p.compte.is_empty() && c.email == p.compte && !c.distant.is_empty()) {
            d.distant = c.distant.clone();
        }
        d
    };
    let rendu = p.rendu.as_ref().ok_or_else(|| anyhow!("pas encore de rendu final"))?;
    let local = etat.dossier_projet(projet).join("sorties").join(&rendu.fichier);
    let nom: String = p.nom.chars().map(|c| if matches!(c, '/' | '\\' | ':') { '-' } else { c }).collect();
    let relatif = format!("{}/{} — EDIVID.mp4", d.sortie, nom.trim());
    etat.drive.occupe(Some(format!("Envoi de « {} » sur le Drive", p.nom)));
    let res = rclone_cmd(&["copyto", &local.to_string_lossy(), &chemin(&d.distant, &relatif)]).await;
    etat.drive.occupe(None);
    res?;
    let r = relatif.clone();
    etat.modifier(projet, |p| p.depot_drive = Some(r)).await?;
    Ok(relatif)
}

pub fn demarrer(etat: EtatPartage) {
    tokio::spawn(async move {
        loop {
            let d = etat.parametres.read().await.drive.clone();
            let attente = Duration::from_secs(d.intervalle_s.max(30));
            if !sources(&etat).await.is_empty() {
                if let Err(e) = passage(&etat).await {
                    tracing::warn!("veille du Drive : {e:#}");
                    etat.drive.dire("Le dossier d'entrée n'a pas pu être lu", Some(format!("{e:#}")));
                }
                etat.drive.etat.lock().unwrap().dernier_passage = Some(maintenant());
            }
            tokio::select! {
                _ = tokio::time::sleep(attente) => {}
                _ = etat.drive.reveil.notified() => {}
            }
        }
    });
}

pub fn vue(etat: &EtatPartage) -> Value {
    let e = etat.drive.etat.lock().unwrap().clone();
    json!({"rclone": rclone().exists() || which("rclone"), "etat": e})
}

fn which(nom: &str) -> bool {
    std::env::var("PATH").unwrap_or_default().split(':').any(|d| Path::new(d).join(nom).exists())
}
