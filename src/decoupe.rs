//! Du montage décidé par le LLM aux plans à rendre : points de coupe calés sur le son, cadrage
//! sur le visage, transitions, sous-titres et incrustations replacés sur le temps de sortie.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::{json, Value};

use crate::modele::{Analyse, Decision, Mot, Phrase, Projet};
use crate::reglages::{nombre, texte, vrai, Reglages};

pub const IPS: f64 = 30.0;
const PAUSE_PHRASE: f64 = 0.45;
const PAS_ENVELOPPE: f64 = 0.01;
const HESITATIONS: &[&str] = &["euh", "heu", "hum", "hmm", "euhm", "bah", "mmh"];
/// Au-delà de cet agrandissement, la netteté IA est appliquée au plan (rendu final).
pub const SEUIL_NETTETE: f64 = 1.6;
const ZOOM_TRANSITION: f64 = 1.4;
const DUREE_MIN_PLAN: f64 = 0.3;
/// Agrandissement le plus fort permis (pixels de sortie par pixel de la source). Au-delà, même la
/// reconstruction IA invente les traits : yeux redessinés, visage changé (mesuré le 2026-10-18 sur une
/// source de 576 px : propre jusqu'à ×3, yeux abîmés dès ×3,8).
pub const AGRANDISSEMENT_MAX: f64 = 3.0;
/// Écart minimal entre deux cadrages pour qu'un changement se lise comme voulu : en dessous, l'œil le prend
/// pour un sursaut de caméra.
pub const PAS_MIN: f64 = 1.2;
/// Durée minimale d'un cadrage avant qu'un changement automatique (punch, recentrage) le modifie
/// (secondes). Le cadre demandé pour une phrase s'applique, lui, dès sa première image.
const MAINTIEN_MIN: f64 = 2.5;
/// Le visage peut s'écarter de l'ancrage du cadre de 10 % de sa taille avant qu'on recadre.
const TOLERANCE_ANCRAGE: f64 = 0.10;

pub fn normaliser(mot: &str) -> String {
    let mut s = String::new();
    for c in mot.to_lowercase().chars() {
        let c = match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        };
        if c.is_alphanumeric() || c == '\'' {
            s.push(c);
        }
    }
    s
}

fn est_hesitation(mot: &str) -> bool {
    HESITATIONS.contains(&normaliser(mot).as_str())
}

/// Phrases de tous les rushs prêts, dans l'ordre du projet.
pub fn phrases(analyses: &[(String, Arc<Analyse>)]) -> Vec<Phrase> {
    let mut res = Vec::new();
    for (rush, a) in analyses {
        let mut groupes: Vec<Vec<Mot>> = Vec::new();
        let mut courant: Vec<Mot> = Vec::new();
        for m in &a.mots {
            if let Some(dernier) = courant.last() {
                if m.t0 - dernier.t1 > PAUSE_PHRASE {
                    groupes.push(std::mem::take(&mut courant));
                }
            }
            courant.push(m.clone());
            if m.mot.ends_with(['.', '?', '!']) {
                groupes.push(std::mem::take(&mut courant));
            }
        }
        if !courant.is_empty() {
            groupes.push(courant);
        }
        let mut fin_prec = 0.0;
        for (i, g) in groupes.into_iter().enumerate() {
            let t0 = g[0].t0;
            let t1 = g[g.len() - 1].t1;
            res.push(Phrase {
                cle: format!("{rush}:{i}"),
                rush: rush.clone(),
                t0,
                t1,
                silence_avant: ((t0 - fin_prec) * 100.0).round() / 100.0,
                db: g.iter().map(|m| m.db).fold(f64::MIN, f64::max),
                texte: recoller(&g.iter().map(|m| m.mot.as_str()).collect::<Vec<_>>()),
                mots: g,
                fusionnees: vec![],
            });
            fin_prec = t1;
        }
    }
    res
}

/// Phrases du projet : les phrases soudées à la main sont réunies (sous la clé de la première), puis les
/// corrections de texte sont appliquées.
pub fn phrases_projet(p: &Projet, analyses: &[(String, Arc<Analyse>)]) -> Vec<Phrase> {
    let mut ps: Vec<Phrase> = Vec::new();
    let mut souder = false;
    for ph in phrases(analyses) {
        match ps.last_mut() {
            Some(prec) if souder && prec.rush == ph.rush => {
                prec.t1 = ph.t1;
                prec.db = prec.db.max(ph.db);
                prec.texte = format!("{} {}", prec.texte, ph.texte);
                prec.mots.extend(ph.mots);
                prec.fusionnees.push(ph.cle);
            }
            _ => ps.push(ph),
        }
        let derniere = ps.last().map(|x| x.fusionnees.last().unwrap_or(&x.cle).clone()).unwrap_or_default();
        souder = p.fusions.contains(&derniere);
    }
    for ph in ps.iter_mut() {
        if let Some(texte) = p.corrections.get(&ph.cle) {
            corriger(ph, texte);
        }
    }
    ps
}

/// Remplace les mots d'une phrase par le texte corrigé, en gardant le minutage d'origine : un mot
/// pour un mot si leur nombre est égal, sinon réparti sur la durée de la phrase selon la longueur.
fn corriger(ph: &mut Phrase, texte: &str) {
    let nouveaux: Vec<&str> = texte.split_whitespace().collect();
    if nouveaux.is_empty() || ph.mots.is_empty() {
        return;
    }
    let db = ph.mots.iter().map(|m| m.db).fold(f64::MIN, f64::max);
    if nouveaux.len() == ph.mots.len() {
        for (m, n) in ph.mots.iter_mut().zip(&nouveaux) {
            m.mot = n.to_string();
        }
    } else {
        let (t0, t1) = (ph.mots[0].t0, ph.mots[ph.mots.len() - 1].t1);
        let total: usize = nouveaux.iter().map(|n| n.chars().count() + 1).sum();
        let mut t = t0;
        ph.mots = nouveaux
            .iter()
            .map(|n| {
                let d = (t1 - t0) * (n.chars().count() + 1) as f64 / total as f64;
                let m = Mot { t0: t, t1: t + d * 0.92, mot: n.to_string(), db };
                t += d;
                m
            })
            .collect();
    }
    ph.texte = texte.trim().to_string();
}

/// Whisper découpe « c'est » en « c » + « 'est » et « bien-être » en « bien » + « -être ».
pub fn recoller(mots: &[&str]) -> String {
    let mut s = String::new();
    for m in mots {
        if !s.is_empty() && !m.starts_with(['\'', '-']) {
            s.push(' ');
        }
        s.push_str(m);
    }
    s
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub rush: String,
    pub phrase: String,
    /// Début dans le rush et durée, arrondie à l'image.
    pub t0: f64,
    pub duree: f64,
    pub sortie_t0: f64,
    pub cadre: String,
    pub regime: String,
    pub accroche: bool,
    /// Zone de la source qui contient tout le plan (le cadre le plus large).
    pub zone: Rect,
    /// Zoom dans la zone à la fin du plan ; il part de 1 (la zone entière).
    pub z_fin: f64,
    /// Centre du cadre le plus serré, en coordonnées 0–1 de la zone.
    pub centre_serre: (f64, f64),
    pub transition: Option<String>,
    /// Effet de sortie quand le plan suivant entre en fondu ou en glissé.
    pub sortie: Option<String>,
    /// Agrandissement au moment le plus serré (sortie / pixels source).
    pub agrandissement: f64,
    pub nettete: bool,
    /// Largeur du visage à l'écran (part de la largeur de l'image) au début et à la fin du plan ; 0 sans visage.
    pub visage_debut: f64,
    pub visage_fin: f64,
    /// Coupe à l'intérieur d'une phrase en rafale : un cran plus serré que le reste de la phrase.
    pub punch: bool,
    /// Phrase forte : le cadre avance d'un cran pendant le plan.
    pub pousse: bool,
    /// Morceau d'une poussée répartie sur plusieurs plans : avance régulière, sans ralenti à la fin du plan.
    pub lineaire: bool,
}

#[derive(Clone, Debug)]
pub struct MotSortie {
    pub t0: f64,
    pub t1: f64,
    pub mot: String,
    pub fort: bool,
}

#[derive(Clone, Debug)]
pub struct IncrustationPlacee {
    pub id: String,
    pub texte: String,
    pub couleur: String,
    pub fond: String,
    pub entree: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub media: String,
    pub effet: String,
    pub debut: f64,
    pub fin: f64,
    pub position: String,
    pub taille: f64,
    pub fond_vert: bool,
    pub son: bool,
}

pub struct Programme {
    pub plans: Vec<Plan>,
    pub largeur: u32,
    pub hauteur: u32,
    pub mots: Vec<MotSortie>,
    pub titres: Vec<(f64, String)>,
    pub transitions: Vec<f64>,
    pub incrustations: Vec<IncrustationPlacee>,
    pub duree: f64,
    pub avertissements: Vec<String>,
    pub coupes_internes: usize,
}

/// Dimensions de sortie : format choisi, ou celui du premier rush.
pub fn dimensions(reglages: &Reglages, premier: Option<&Analyse>, apercu: bool) -> (u32, u32) {
    let format = match texte(reglages, "format").as_str() {
        "auto" => match premier {
            Some(a) if a.largeur > a.hauteur => "horizontal",
            Some(a) if a.largeur == a.hauteur => "carre",
            _ => "vertical",
        }
        .to_string(),
        f => f.to_string(),
    };
    let k = if texte(reglages, "definition") == "2160" { 2 } else { 1 };
    let (l, h) = match format.as_str() {
        "horizontal" => (1920 * k, 1080 * k),
        "carre" => (1080 * k, 1080 * k),
        _ => (1080 * k, 1920 * k),
    };
    // L'aperçu reste léger, quelle que soit la définition finale.
    let (l, h) = if apercu && k == 2 { (l / 2, h / 2) } else { (l, h) };
    if apercu {
        (l / 2, h / 2)
    } else {
        (l, h)
    }
}

struct Son<'a> {
    env: &'a [f32],
    seuil: f32,
}

impl Son<'_> {
    fn nouveau(env: &[f32]) -> Son<'_> {
        if env.is_empty() {
            return Son { env, seuil: f32::MIN };
        }
        let mut tri = env.to_vec();
        tri.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let bas = tri[tri.len() / 10];
        let haut = tri[tri.len() * 9 / 10];
        let seuil = (bas + 0.35 * (haut - bas)).clamp(bas + 6.0, (haut - 3.0).max(bas + 6.0));
        Son { env, seuil }
    }

    fn parle(&self, t: f64) -> bool {
        let i = (t / PAS_ENVELOPPE).floor();
        if i < 0.0 || self.env.is_empty() {
            return false;
        }
        self.env.get(i as usize).map(|v| *v > self.seuil).unwrap_or(false)
    }

    /// Recule le début jusqu'au vrai départ de la voix (0,15 s au plus).
    fn debut(&self, t: f64) -> f64 {
        let mut s = t;
        while s > t - 0.15 && self.parle(s - PAS_ENVELOPPE) {
            s -= PAS_ENVELOPPE;
        }
        s
    }

    /// Avance la fin jusqu'au vrai silence (0,45 s au plus) : Whisper finit les mots trop tôt.
    fn fin(&self, t: f64) -> f64 {
        let mut e = t;
        while e < t + 0.45 && self.parle(e) {
            e += PAS_ENVELOPPE;
        }
        e
    }

    /// Silences internes d'au moins `min` secondes entre a et b.
    fn silences(&self, a: f64, b: f64, min: f64) -> Vec<(f64, f64)> {
        let mut res = Vec::new();
        let mut t = a + 0.1;
        let mut debut: Option<f64> = None;
        while t < b - 0.1 {
            if self.parle(t) {
                if let Some(d) = debut.take() {
                    if t - d >= min {
                        res.push((d, t));
                    }
                }
            } else if debut.is_none() {
                debut = Some(t);
            }
            t += PAS_ENVELOPPE;
        }
        res
    }
}

fn arrondi_image(t: f64) -> f64 {
    (t * IPS).round() / IPS
}

fn visage_median(a: &Analyse, t0: f64, t1: f64, yeux: bool) -> Option<[f64; 4]> {
    let choisir = |v: &crate::modele::Visage| if yeux { v.yeux } else { v.visage };
    let mut dedans: Vec<[f64; 4]> = a.visages.iter().filter(|v| v.t >= t0 && v.t <= t1).map(choisir).collect();
    if dedans.is_empty() {
        let milieu = (t0 + t1) / 2.0;
        let proche = a
            .visages
            .iter()
            .min_by(|x, y| (x.t - milieu).abs().partial_cmp(&(y.t - milieu).abs()).unwrap())?;
        dedans.push(choisir(proche));
    }
    let mut res = [0.0; 4];
    for (k, r) in res.iter_mut().enumerate() {
        let mut col: Vec<f64> = dedans.iter().map(|b| b[k]).collect();
        col.sort_by(|x, y| x.partial_cmp(y).unwrap());
        *r = col[col.len() / 2];
    }
    Some(res)
}

/// Où placer le visage dans le cadre (part de la hauteur, depuis le haut) : au tiers haut en plan large,
/// un peu plus bas en gros plan.
fn hauteur_visage(z: f64) -> f64 {
    0.35 + 0.12 * (z.ln() / 3.3f64.ln()).clamp(0.0, 1.0)
}

/// Grille de cadrage d'un rush : quelques zooms fixes, du plus large (l'image entière, sous la bande
/// rognée) au plus serré que la source supporte sans perte, séparés d'un pas franc.
struct Grille {
    source: (f64, f64),
    haut: f64,
    base: (f64, f64),
    niveaux: Vec<f64>,
    plafond: f64,
}

impl Grille {
    fn nouvelle(a: &Analyse, sortie: (u32, u32), rognage: f64, pas: f64) -> Grille {
        let source = (a.largeur as f64, a.hauteur as f64);
        let haut = (source.1 * rognage.clamp(0.0, 0.3)).round();
        let utile = (source.0, source.1 - haut);
        let aspect = sortie.0 as f64 / sortie.1 as f64;
        let base = if utile.0 / utile.1 > aspect { (utile.1 * aspect, utile.1) } else { (utile.0, utile.0 / aspect) };
        // Seule borne : l'agrandissement que la source supporte (une source 4K se resserre deux fois plus
        // qu'une source 1080p).
        let plafond = (AGRANDISSEMENT_MAX * base.0 / sortie.0 as f64).max(1.0);
        // De l'image entière au plafond, en pas égaux : le nombre de pas qui approche le mieux le pas voulu,
        // sans jamais descendre sous PAS_MIN (exemple : plafond ×1,47, pas voulu 30 % → deux pas de 21 %).
        let etendue = plafond.ln();
        let pas_retenu = (1..=12)
            .map(|n| (etendue / n as f64).exp())
            .take_while(|s| *s >= PAS_MIN - 1e-9)
            .min_by(|a, b| (a.ln() - pas.ln()).abs().partial_cmp(&(b.ln() - pas.ln()).abs()).unwrap());
        let niveaux = match pas_retenu {
            Some(s) => {
                let n = (etendue / s.ln()).round() as i32;
                (0..=n).map(|k| s.powi(k)).collect()
            }
            None => vec![1.0],
        };
        Grille { source, haut, base, niveaux, plafond }
    }

    fn plus_serre(&self) -> usize {
        self.niveaux.len() - 1
    }

    /// Le cran le plus proche d'un zoom voulu.
    fn cran(&self, z: f64) -> usize {
        let ecart = |i: usize| (self.niveaux[i].ln() - z.max(1.0).ln()).abs();
        (0..self.niveaux.len()).min_by(|&i, &j| ecart(i).partial_cmp(&ecart(j)).unwrap()).unwrap_or(0)
    }

    /// Cadre au zoom z autour de l'ancrage (centre du visage), borné à la partie utile de la source.
    fn cadre(&self, z: f64, ancre: (f64, f64)) -> Rect {
        let (w, h) = (self.base.0 / z, self.base.1 / z);
        let cy = ancre.1 + (0.5 - hauteur_visage(z)) * h;
        let x = (ancre.0 - w / 2.0).clamp(0.0, (self.source.0 - w).max(0.0));
        let y = (cy - h / 2.0).clamp(self.haut, (self.source.1 - h).max(self.haut));
        Rect { x, y, w, h }
    }
}

/// Visage d'un plan (centre et largeur, en pixels de la source), mesuré sur le plan élargi d'une seconde
/// et demie de chaque côté, pour ne pas suivre le tremblement du repérage. Pour le cadre « yeux », le
/// centre est celui des yeux, la largeur reste celle du visage.
fn visage_plan(a: &Analyse, t0: f64, t1: f64, yeux: bool) -> Option<(f64, f64, f64)> {
    let (l, h) = (a.largeur as f64, a.hauteur as f64);
    let v = visage_median(a, t0 - 1.5, t1 + 1.5, false)?;
    let c = if yeux { visage_median(a, t0 - 1.5, t1 + 1.5, true).filter(|y| y[2] > y[0]).unwrap_or(v) } else { v };
    Some((((c[0] + c[2]) / 2.0) * l, ((c[1] + c[3]) / 2.0) * h, (v[2] - v[0]) * l))
}

/// Cran de chaque cadre (large, moyen, visage, yeux) pour un visage occupant `part` de la largeur de l'image
/// entière. Chaque cadre vise une taille de visage à l'écran ; quand la source ne permet pas de l'atteindre,
/// les cadres restent tout de même distincts et dans l'ordre tant que la grille a assez de crans : « yeux »
/// est toujours le plus serré possible, « large » toujours l'image entière.
fn crans_cadres(g: &Grille, part: f64, intensite: f64) -> [usize; 4] {
    if part <= 0.0 {
        return [0; 4];
    }
    let serre = g.plus_serre();
    let mut c = [0, g.cran(0.17 * intensite / part), g.cran(0.30 * intensite / part), g.cran(0.46 * intensite / part)];
    for i in 1..4 {
        c[i] = c[i].max(c[i - 1] + 1).min(serre);
    }
    for i in (1..3).rev() {
        c[i] = c[i].min(c[i + 1].saturating_sub(1)).max(1.min(serre));
    }
    c
}

fn indice_cadre(cadre: &str) -> usize {
    match cadre {
        "moyen" => 1,
        "visage" => 2,
        "yeux" => 3,
        _ => 0,
    }
}

/// Cadre tous les plans, dans l'ordre du montage. Un cadrage tenu ne bouge pas d'un pixel. Chaque phrase reçoit
/// directement le cran de son cadre (un plan large revient toujours à l'image entière), indépendamment de ce
/// qui précède : changer une phrase ne décale pas les cadres des suivantes. Les changements automatiques
/// (punch d'une rafale, recentrage sur un visage qui s'est déplacé) attendent que le cadrage ait tenu
/// MAINTIEN_MIN, et se font d'un cran entier, jamais d'un micro-recadrage. Une transition remet tout à plat.
fn cadrer_plans(plans: &mut [Plan], analyses: &HashMap<&str, &Analyse>, r: &Reglages, finale: (u32, u32), sortie: (u32, u32)) {
    let pas = (1.0 + nombre(r, "ecart_cadrage") / 100.0).max(PAS_MIN);
    let rognage = nombre(r, "rognage_haut") / 100.0;
    let intensite = match texte(r, "zooms").as_str() {
        "doux" => 0.8,
        "fort" => 1.2,
        _ => 1.0,
    };
    let mut grilles: HashMap<String, Grille> = HashMap::new();
    // Cadrage en cours : rush, phrase, cran atteint à la fin du plan précédent, ancrage, durée déjà tenue.
    let mut courant: Option<(String, String, usize, (f64, f64), f64)> = None;
    // Une poussée couvre toute sa phrase, d'une coupe à l'autre : chaque plan en joue sa part (début et fin,
    // en fraction de la durée de la phrase), au lieu de rejouer un zoom avant à chaque coupe.
    let mut parts: Vec<Option<(f64, f64)>> = vec![None; plans.len()];
    let mut i = 0;
    while i < plans.len() {
        let meme = |a: &Plan, b: &Plan| b.pousse && b.phrase == a.phrase && b.accroche == a.accroche && b.rush == a.rush && b.transition.is_none();
        let mut j = i + 1;
        if plans[i].pousse {
            while j < plans.len() && meme(&plans[i], &plans[j]) {
                j += 1;
            }
            let total: f64 = plans[i..j].iter().map(|p| p.duree).sum();
            let mut ecoule = 0.0;
            for k in i..j {
                parts[k] = Some((ecoule / total, (ecoule + plans[k].duree) / total));
                ecoule += plans[k].duree;
            }
        }
        i = j;
    }
    let mut poussee: Option<(usize, (f64, f64))> = None;
    for (i, pl) in plans.iter_mut().enumerate() {
        let Some(a) = analyses.get(pl.rush.as_str()) else { continue };
        let g = grilles.entry(pl.rush.clone()).or_insert_with(|| Grille::nouvelle(a, finale, rognage, pas));
        let serre_max = g.plus_serre();
        let (fx, fy, fw) = visage_plan(a, pl.t0, pl.t0 + pl.duree, pl.cadre == "yeux")
            .unwrap_or((g.source.0 / 2.0, g.source.1 / 2.0, 0.0));
        let part = if fw > 0.0 { fw / g.base.0 } else { 0.0 };
        let base = crans_cadres(g, part, intensite)[indice_cadre(&pl.cadre)];
        let mut voulu = base;
        // Le punch resserre d'un cran un plan sur deux d'une rafale ; un plan large reste l'image entière.
        if pl.punch && base > 0 {
            voulu = if voulu < serre_max { voulu + 1 } else { voulu.saturating_sub(1) };
        }
        let suite = courant.as_ref().filter(|c| c.0 == pl.rush && pl.transition.is_none());
        let (cran, ancre) = match suite {
            Some((_, phrase, prec, ancre, tenu)) => {
                let prec = *prec;
                // Le cadre d'une nouvelle phrase s'applique tout de suite ; dans une phrase, un changement
                // automatique attend que le cadrage ait tenu.
                let nouvelle = *phrase != pl.phrase;
                // Une poussée (impact) repart toujours de son cadre : sinon elle s'empilerait de plan en plan.
                let mut n = if voulu != prec && !nouvelle && !pl.pousse && *tenu < MAINTIEN_MIN { prec } else { voulu };
                let z = g.niveaux[n];
                let deplace = (fx - ancre.0).abs() > TOLERANCE_ANCRAGE * g.base.0 / z
                    || (fy - ancre.1).abs() > TOLERANCE_ANCRAGE * g.base.1 / z;
                // Le visage s'est déplacé : on le recentre, avec un changement de cran franc. Inutile en plan
                // large, qui montre déjà toute l'image.
                if deplace && n == prec && n > 0 && *tenu >= MAINTIEN_MIN {
                    n = if n < serre_max && n <= base { n + 1 } else { n - 1 };
                }
                // Un changement de cran recentre sur le visage ; un cadrage tenu garde son ancrage.
                (n, if n != prec { (fx, fy) } else { *ancre })
            }
            None => (voulu, (fx, fy)),
        };
        // La suite d'une poussée garde le cran et l'ancrage de son début : le mouvement continue.
        let (f0, f1) = parts[i].unwrap_or((0.0, 1.0));
        let (cran, ancre) = match poussee {
            Some(p) if parts[i].is_some() && f0 > 0.0 => p,
            _ => (cran, ancre),
        };
        poussee = parts[i].map(|_| (cran, ancre));
        let fin = if pl.pousse && cran < serre_max { cran + 1 } else { cran };
        let pente = g.niveaux[fin] / g.niveaux[cran];
        let (za, zb) = (g.niveaux[cran] * pente.powf(f0), g.niveaux[cran] * pente.powf(f1));
        let z_trans = if pl.transition.as_deref() == Some("zoom") { ZOOM_TRANSITION } else { 1.0 };
        let zone = g.cadre(za, ancre);
        let serre = g.cadre((zb * z_trans).min(g.plafond.max(zb)), ancre);
        pl.zone = zone;
        pl.z_fin = zb / za;
        pl.lineaire = parts[i].is_some() && (f0 > 0.0 || f1 < 1.0);
        pl.centre_serre = (
            ((serre.x + serre.w / 2.0 - zone.x) / zone.w).clamp(0.0, 1.0),
            ((serre.y + serre.h / 2.0 - zone.y) / zone.h).clamp(0.0, 1.0),
        );
        pl.agrandissement = sortie.0 as f64 / serre.w;
        pl.visage_debut = za * part;
        pl.visage_fin = zb * part;
        let tenu = match suite {
            Some((_, _, prec, _, t)) if *prec == cran => t + pl.duree,
            _ => pl.duree,
        };
        courant = Some((pl.rush.clone(), pl.phrase.clone(), fin, ancre, if fin != cran { 0.0 } else { tenu }));
    }
}

struct Entree<'a> {
    phrase: &'a Phrase,
    decision: Decision,
    accroche: bool,
}

/// Construit le programme de rendu.
pub fn programme(p: &Projet, analyses: &[(String, Arc<Analyse>)], ps: &[Phrase], apercu: bool) -> Programme {
    let r = &p.reglages;
    let montage = p.montage.as_ref();
    let par_cle: HashMap<&str, &Phrase> = ps.iter().map(|ph| (ph.cle.as_str(), ph)).collect();
    let analyses_par_rush: HashMap<&str, &Analyse> = analyses.iter().map(|(id, a)| (id.as_str(), a.as_ref())).collect();
    let decisions: HashMap<&str, &Decision> =
        montage.map(|m| m.phrases.iter().map(|d| (d.cle.as_str(), d)).collect()).unwrap_or_default();
    let (largeur, hauteur) = dimensions(r, analyses.first().map(|(_, a)| a.as_ref()), apercu);
    let zooms = texte(r, "zooms");
    let style = texte(r, "transition");
    let nettete = texte(r, "nettete_ia");
    let rythme_max = match texte(r, "rythme").as_str() {
        "ample" => 0.55,
        "nerveux" => 0.18,
        _ => 0.30,
    };
    let mut avertissements = Vec::new();

    let defaut = |cle: &str| Decision {
        cle: cle.into(),
        garder: true,
        regime: "rafale".into(),
        cadre: "moyen".into(),
        transition: None,
        mots_forts: vec![],
        transition_choisie: None,
        mouvement: None,
    };
    // Corps : phrases gardées, dans l'ordre.
    let corps: Vec<&Phrase> = ps
        .iter()
        .filter(|ph| decisions.get(ph.cle.as_str()).map(|d| d.garder).unwrap_or(true))
        .collect();
    let debut_corps: HashSet<&str> = corps.iter().take(3).map(|ph| ph.cle.as_str()).collect();
    let mut entrees: Vec<Entree> = Vec::new();
    let choisie = montage.map(|m| m.accroche_choisie).unwrap_or(false);
    if vrai(r, "accroche") {
        let mut duree = 0.0;
        for cle in montage.map(|m| m.accroche.clone()).unwrap_or_default() {
            // Garde : l'accroche du LLM ne redit pas l'introduction (celle choisie à la main est prise telle quelle).
            if debut_corps.contains(cle.as_str()) && !choisie {
                continue;
            }
            if let Some(ph) = par_cle.get(cle.as_str()) {
                if duree + (ph.t1 - ph.t0) > 10.0 && !choisie {
                    break;
                }
                duree += ph.t1 - ph.t0;
                let mut d = decisions.get(cle.as_str()).map(|d| (*d).clone()).unwrap_or_else(|| defaut(&cle));
                d.regime = "impact".into();
                if d.cadre != "yeux" {
                    d.cadre = "visage".into();
                }
                entrees.push(Entree { phrase: ph, decision: d, accroche: true });
            }
        }
        // Repli : si le choix du LLM redisait l'introduction, on prend ses phrases « impact » les plus
        // engagées, situées après la première minute, courtes, jusqu'à huit secondes au total.
        if entrees.is_empty() && !choisie {
            let mut candidates: Vec<&&Phrase> = corps
                .iter()
                .filter(|ph| ph.t0 > 60.0 && ph.t1 - ph.t0 <= 6.0 && !debut_corps.contains(ph.cle.as_str()))
                .filter(|ph| decisions.get(ph.cle.as_str()).map(|d| d.regime == "impact").unwrap_or(false))
                .collect();
            candidates.sort_by(|a, b| b.db.partial_cmp(&a.db).unwrap_or(std::cmp::Ordering::Equal));
            let mut duree = 0.0;
            for ph in candidates.into_iter().take(2) {
                if duree + (ph.t1 - ph.t0) > 8.0 {
                    break;
                }
                duree += ph.t1 - ph.t0;
                let mut d = decisions.get(ph.cle.as_str()).map(|d| (*d).clone()).unwrap_or_else(|| defaut(&ph.cle));
                d.regime = "impact".into();
                if d.cadre != "yeux" {
                    d.cadre = "visage".into();
                }
                entrees.push(Entree { phrase: ph, decision: d, accroche: true });
            }
            if !entrees.is_empty() {
                avertissements.push("accroche du LLM écartée (elle redisait l'introduction) : phrases fortes choisies à la place".into());
            }
        }
    }
    let chapitres: HashMap<String, String> = montage
        .map(|m| m.chapitres.iter().map(|c| (c.cle.clone(), c.titre.clone())).collect())
        .unwrap_or_default();
    // Une partie qui commence sur une phrase coupée commence à la phrase gardée suivante.
    let mut debut_partie: HashMap<String, String> = HashMap::new();
    let mut en_attente: Vec<String> = Vec::new();
    for ph in ps {
        if let Some(t) = chapitres.get(&ph.cle) {
            en_attente.push(t.clone());
        }
        if decisions.get(ph.cle.as_str()).map(|d| d.garder).unwrap_or(true) && !en_attente.is_empty() {
            debut_partie.insert(ph.cle.clone(), en_attente.remove(0));
            en_attente.clear();
        }
    }
    for ph in &corps {
        let d = decisions.get(ph.cle.as_str()).map(|d| (*d).clone()).unwrap_or_else(|| defaut(&ph.cle));
        entrees.push(Entree { phrase: ph, decision: d, accroche: false });
    }

    let mut plans: Vec<Plan> = Vec::new();
    let mut mots_sortie: Vec<MotSortie> = Vec::new();
    let mut titres = Vec::new();
    let mut transitions = Vec::new();
    let mut sortie_t = 0.0;
    let mut coupes_internes = 0;
    let mut premier_du_corps = true;
    let mut sons: HashMap<&str, Son> = HashMap::new();
    for (id, a) in analyses {
        sons.insert(id.as_str(), Son::nouveau(&a.enveloppe));
    }

    for e in &entrees {
        let ph = e.phrase;
        if !analyses_par_rush.contains_key(ph.rush.as_str()) {
            continue;
        }
        let son = &sons[ph.rush.as_str()];
        let mut d = e.decision.clone();
        if zooms == "aucun" {
            d.cadre = "large".into();
        } else if d.cadre == "yeux" && !vrai(r, "yeux") {
            d.cadre = "visage".into();
        }
        // Transition au début de la phrase.
        let mut transition = None;
        let partie = (!e.accroche).then(|| debut_partie.get(&ph.cle)).flatten();
        let veut = !e.accroche
            && !plans.is_empty()
            && (premier_du_corps || partie.is_some() || d.transition.is_some() || d.transition_choisie.is_some());
        if veut {
            // Le choix fait à la main prime ; « coupe » = aucune transition.
            transition = match d.transition_choisie.as_deref() {
                Some("coupe") => None,
                Some(t) => Some(t.to_string()),
                None if style == "coupe" => None,
                None if style == "auto" => Some(d.transition.clone().unwrap_or_else(|| "flash".into())),
                None => Some(style.clone()),
            };
        }
        if !e.accroche {
            premier_du_corps = false;
        }

        // Séries de mots, coupées aux hésitations.
        let mut series: Vec<Vec<&Mot>> = vec![vec![]];
        for m in &ph.mots {
            if vrai(r, "coupe_hesitations") && est_hesitation(&m.mot) {
                series.push(vec![]);
            } else {
                series.last_mut().unwrap().push(m);
            }
        }
        series.retain(|s| !s.is_empty());
        let respiration = d.regime == "respiration";
        let silence_max = if respiration { (rythme_max * 2.0f64).min(1.0) } else { rythme_max };
        let traine = if respiration { 0.18 } else { 0.08 };
        let mut segments: Vec<(f64, f64)> = Vec::new();
        for (k, s) in series.iter().enumerate() {
            let mut debut = son.debut(s[0].t0) - 0.03;
            let fin = son.fin(s[s.len() - 1].t1) + traine;
            if k == 0 && (respiration || !vrai(r, "coupe_silences")) {
                debut -= ph.silence_avant.min(if respiration { 0.4 } else { 1.5 });
            }
            let mut courant = debut.max(0.0);
            if vrai(r, "coupe_silences") {
                for (s0, s1) in son.silences(courant, fin, silence_max) {
                    if s0 + 0.05 - courant >= DUREE_MIN_PLAN && fin - (s1 - 0.03) >= DUREE_MIN_PLAN {
                        segments.push((courant, s0 + 0.05));
                        courant = s1 - 0.03;
                        coupes_internes += 1;
                    }
                }
            }
            segments.push((courant, fin));
        }
        // Pas de chevauchement avec le plan précédent du même rush.
        if let Some(prec) = plans.last() {
            if prec.rush == ph.rush && !e.accroche && !prec.accroche {
                let fin_prec = prec.t0 + prec.duree;
                for seg in segments.iter_mut() {
                    seg.0 = seg.0.max(fin_prec);
                }
            }
        }
        segments.retain(|(a0, a1)| a1 - a0 >= DUREE_MIN_PLAN * 0.7);

        for (k, (s0, s1)) in segments.iter().enumerate() {
            let t0 = arrondi_image(*s0);
            let duree = arrondi_image(s1 - s0).max(1.0 / IPS);
            // Mouvement choisi à la main, sinon celui du rythme (si le réglage général le permet).
            let (punch, pousse) = match d.mouvement.as_deref() {
                _ if zooms == "aucun" => (false, false),
                Some("punch") => (k % 2 == 1, false),
                Some("pousse") => (false, true),
                Some(_) => (false, false),
                None => (d.regime == "rafale" && k % 2 == 1 && vrai(r, "punch_jumpcut"), d.regime == "impact" && vrai(r, "zoom_anime")),
            };
            let trans = if k == 0 { transition.clone() } else { None };
            if k == 0 {
                if let Some(t) = &trans {
                    transitions.push(sortie_t);
                    if t == "fondu" || t == "glisse" {
                        if let Some(prec) = plans.last_mut() {
                            prec.sortie = Some(t.clone());
                        }
                    }
                }
                if !e.accroche && vrai(r, "titres_chapitres") {
                    if let Some(t) = partie {
                        titres.push((sortie_t, t.clone()));
                    }
                }
            }
            let mots_forts: HashSet<String> = d.mots_forts.iter().map(|m| normaliser(m)).collect();
            for m in &ph.mots {
                let milieu = (m.t0 + m.t1) / 2.0;
                if milieu < t0 || milieu > t0 + duree || (vrai(r, "coupe_hesitations") && est_hesitation(&m.mot)) {
                    continue;
                }
                mots_sortie.push(MotSortie {
                    t0: sortie_t + (m.t0 - t0).max(0.0),
                    t1: sortie_t + (m.t1 - t0).min(duree),
                    mot: m.mot.clone(),
                    fort: mots_forts.contains(&normaliser(&m.mot)),
                });
            }
            plans.push(Plan {
                rush: ph.rush.clone(),
                phrase: ph.cle.clone(),
                t0,
                duree,
                sortie_t0: sortie_t,
                cadre: d.cadre.clone(),
                regime: d.regime.clone(),
                accroche: e.accroche,
                // Le cadrage se décide ensuite, sur toute la suite des plans (cadrer_plans).
                zone: Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 },
                z_fin: 1.0,
                centre_serre: (0.5, 0.5),
                transition: trans,
                sortie: None,
                agrandissement: 1.0,
                nettete: false,
                visage_debut: 0.0,
                visage_fin: 0.0,
                punch,
                pousse,
                lineaire: false,
            });
            sortie_t += duree;
        }
    }

    // Cadrage d'ensemble, sur la grille de la définition finale : l'aperçu montre les mêmes cadres.
    let finale = dimensions(r, analyses.first().map(|(_, a)| a.as_ref()), false);
    cadrer_plans(&mut plans, &analyses_par_rush, r, finale, (largeur, hauteur));
    let veut_nettete = !apercu && nettete != "non";
    for pl in plans.iter_mut() {
        pl.nettete = veut_nettete && pl.agrandissement > SEUIL_NETTETE;
    }

    // Incrustations, replacées sur le temps de sortie.
    let mut incrustations = Vec::new();
    for inc in &p.incrustations {
        let Some(ph) = par_cle.get(inc.phrase.as_str()) else {
            avertissements.push("une incrustation vise une phrase qui n'existe plus".into());
            continue;
        };
        let t = ph.t0 + inc.decalage;
        let plan = plans
            .iter()
            .filter(|pl| !pl.accroche && pl.phrase == ph.cle)
            .find(|pl| t >= pl.t0 && t < pl.t0 + pl.duree)
            .or_else(|| plans.iter().find(|pl| !pl.accroche && pl.phrase == ph.cle));
        let Some(plan) = plan else {
            avertissements.push(format!("incrustation ignorée : la phrase « {} » est coupée", court(&ph.texte)));
            continue;
        };
        let debut = plan.sortie_t0 + (t - plan.t0).clamp(0.0, plan.duree);
        incrustations.push(IncrustationPlacee {
            id: inc.id.clone(),
            texte: inc.texte.clone(),
            couleur: inc.couleur.clone(),
            fond: inc.fond.clone(),
            entree: inc.entree.clone(),
            x: inc.x,
            y: inc.y,
            media: inc.media.clone(),
            effet: inc.effet.clone(),
            debut,
            fin: (debut + inc.duree.max(0.2)).min(sortie_t),
            position: inc.position.clone(),
            taille: inc.taille.clamp(0.1, 1.0),
            fond_vert: inc.fond_vert,
            son: inc.son,
        });
    }
    if nombre(r, "volume_musique") > 0.0 && !texte(r, "musique").is_empty()
        && !p.medias.iter().any(|m| m.id == texte(r, "musique"))
    {
        avertissements.push("la musique choisie n'est plus dans la médiathèque".into());
    }

    Programme {
        plans,
        largeur,
        hauteur,
        mots: mots_sortie,
        titres,
        transitions,
        incrustations,
        duree: sortie_t,
        avertissements,
        coupes_internes,
    }
}

pub fn court(t: &str) -> String {
    let t: String = t.chars().take(60).collect();
    t
}

fn arr(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn arr3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Cadre à la fin d'un plan (après un zoom qui avance).
fn cadre_fin(p: &Plan) -> Rect {
    let (w, h) = (p.zone.w / p.z_fin, p.zone.h / p.z_fin);
    let (cx, cy) = (p.zone.x + p.centre_serre.0 * p.zone.w, p.zone.y + p.centre_serre.1 * p.zone.h);
    Rect { x: cx - w / 2.0, y: cy - h / 2.0, w, h }
}

/// Coefficient de saut de cadrage, à chaque coupe franche : rapport de taille entre le cadre qui finit et
/// celui qui commence, et déplacement de leur centre. Une coupe est « identique » (même cadre au pixel près),
/// « franche » (au moins PAS_MIN), ou un « micro-recadrage » (entre les deux : l'œil y voit un sursaut).
pub fn sauts_cadrage(prog: &Programme) -> Value {
    let (mut identiques, mut micro) = (0usize, Vec::new());
    let mut francs: Vec<f64> = Vec::new();
    for p in prog.plans.windows(2) {
        if p[1].transition.is_some() || p[0].rush != p[1].rush {
            continue;
        }
        let (a, b) = (cadre_fin(&p[0]), p[1].zone);
        let rapport = a.w.max(b.w) / a.w.min(b.w);
        let decalage = (a.x + a.w / 2.0 - b.x - b.w / 2.0).hypot(a.y + a.h / 2.0 - b.y - b.h / 2.0) / a.w.min(b.w);
        if rapport < 1.005 && decalage < 0.01 {
            identiques += 1;
        } else if rapport >= PAS_MIN - 0.005 {
            francs.push(rapport);
        } else {
            micro.push(arr(p[1].sortie_t0));
        }
    }
    francs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    json!({
        "coupes": identiques + francs.len() + micro.len(),
        "identiques": identiques,
        "francs": francs.len(),
        "micro_recadrages": micro.len(),
        "micro_a": micro.iter().take(10).collect::<Vec<_>>(),
        "rapport_franc_median": francs.get(francs.len() / 2).map(|v| arr(*v)),
        "rapport_max": francs.last().map(|v| arr(*v)),
    })
}

/// Plans, avec leur place dans le montage (t, d) et dans le rush d'origine (src).
pub fn plans_json(prog: &Programme) -> Value {
    json!(prog
        .plans
        .iter()
        .map(|pl| json!({"t": arr(pl.sortie_t0), "d": arr(pl.duree), "src": arr(pl.t0), "rush": pl.rush,
                         "phrase": pl.phrase, "regime": pl.regime, "cadre": pl.cadre, "accroche": pl.accroche,
                         "punch": pl.punch, "pousse": pl.pousse,
                         "transition": pl.transition, "visage": [arr3(pl.visage_debut), arr3(pl.visage_fin)],
                         "agrandissement": arr(pl.agrandissement),
                         "cadre_source": [arr(pl.zone.x), arr(pl.zone.y), arr(pl.zone.w), arr(pl.zone.h)],
                         "zoom_fin": arr3(pl.z_fin)}))
        .collect::<Vec<_>>())
}

/// Le script courant, tel que les timelines l'affichent (sans attendre un rendu).
pub fn programme_json(prog: &Programme) -> Value {
    json!({
        "duree": arr(prog.duree),
        "largeur": prog.largeur,
        "hauteur": prog.hauteur,
        "plans": plans_json(prog),
        "titres": prog.titres.iter().map(|(t, titre)| json!({"t": arr(*t), "titre": titre})).collect::<Vec<_>>(),
        "incrustations": prog.incrustations.iter().map(|i| json!({"id": i.id, "debut": arr(i.debut), "fin": arr(i.fin)}))
            .collect::<Vec<_>>(),
        "avertissements": prog.avertissements,
        "sauts_cadrage": sauts_cadrage(prog),
    })
}

pub fn rapport(prog: &Programme, duree_source: f64) -> Value {
    let mut cadres: HashMap<&str, usize> = HashMap::new();
    let mut regimes: HashMap<&str, usize> = HashMap::new();
    for p in &prog.plans {
        *cadres.entry(p.cadre.as_str()).or_default() += 1;
        *regimes.entry(p.regime.as_str()).or_default() += 1;
    }
    let n = prog.plans.len().max(1);
    json!({
        "duree_source": (duree_source * 10.0).round() / 10.0,
        "duree_montage": (prog.duree * 10.0).round() / 10.0,
        "plans": prog.plans.len(),
        "duree_moyenne_plan": ((prog.duree / n as f64) * 100.0).round() / 100.0,
        "coupes_internes": prog.coupes_internes,
        "cadres": cadres,
        "regimes": regimes,
        "zooms_animes": prog.plans.iter().filter(|p| p.z_fin > 1.0 + 1e-6).count(),
        "transitions": prog.transitions.len(),
        "plans_nettete_ia": prog.plans.iter().filter(|p| p.nettete).count(),
        "agrandissement_max": arr(prog.plans.iter().map(|p| p.agrandissement).fold(0.0, f64::max)),
        "sauts_cadrage": sauts_cadrage(prog),
        "incrustations": prog.incrustations.len(),
        "avertissements": prog.avertissements,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grille(niveaux: usize) -> Grille {
        let s = 1.3f64;
        Grille {
            source: (3840.0, 2160.0),
            haut: 0.0,
            base: (3840.0, 2160.0),
            niveaux: (0..niveaux).map(|k| s.powi(k as i32)).collect(),
            plafond: s.powi(niveaux as i32 - 1),
        }
    }

    #[test]
    fn cadres_distincts_meme_sur_un_petit_visage() {
        // Visage de 4 % de la largeur (source 4K filmée de loin) : aucun cadre n'atteint sa taille visée.
        let c = crans_cadres(&grille(8), 0.04, 1.0);
        assert_eq!(c[0], 0);
        assert!(c[0] < c[1] && c[1] < c[2] && c[2] < c[3], "{c:?}");
        assert_eq!(c[3], 7);
    }

    #[test]
    fn grille_courte_garde_large_et_yeux() {
        let c = crans_cadres(&grille(3), 0.04, 1.0);
        assert_eq!((c[0], c[3]), (0, 2));
        let c = crans_cadres(&grille(1), 0.04, 1.0);
        assert_eq!(c, [0; 4]);
    }

    /// Recalcule le programme d'un vrai projet : `EDIVID_PROJET=donnees/projets/<id> cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn cadres_d_un_projet() {
        let dossier = std::path::PathBuf::from(std::env::var("EDIVID_PROJET").expect("EDIVID_PROJET"));
        let p: Projet = serde_json::from_str(&std::fs::read_to_string(dossier.join("projet.json")).unwrap()).unwrap();
        let analyses: Vec<(String, Arc<Analyse>)> = p
            .rushs
            .iter()
            .map(|r| {
                let d = dossier.join("travail").join(&r.id);
                let mut a: Analyse = serde_json::from_str(&std::fs::read_to_string(d.join("analyse.json")).unwrap()).unwrap();
                let o = std::fs::read(d.join("enveloppe.f32")).unwrap_or_default();
                a.enveloppe = o.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
                (r.id.clone(), Arc::new(a))
            })
            .collect();
        let ps = phrases_projet(&p, &analyses);
        let prog = programme(&p, &analyses, &ps, false);
        // Soudure : deux phrases voisines n'en font plus qu'une, sous la clé de la première, et se séparent.
        let mut soude = p.clone();
        soude.fusions = vec![ps[1].cle.clone(), ps[2].cle.clone()];
        // Comme le fait l'API au moment de souder : les corrections des morceaux deviennent celle du tout.
        for ph in &ps[1..4] {
            soude.corrections.remove(&ph.cle);
        }
        soude.corrections.insert(ps[1].cle.clone(), format!("{} {} {}", ps[1].texte, ps[2].texte, ps[3].texte));
        let ps2 = phrases_projet(&soude, &analyses);
        assert_eq!(ps2.len(), ps.len() - 2);
        assert_eq!(ps2[1].fusionnees, vec![ps[2].cle.clone(), ps[3].cle.clone()]);
        assert_eq!(ps2[1].texte, format!("{} {} {}", ps[1].texte, ps[2].texte, ps[3].texte));
        assert_eq!(ps2[2].cle, ps[4].cle);
        // Poussée : une seule avance continue par phrase, pas un zoom avant à chaque coupe.
        for w in prog.plans.windows(2) {
            if w[0].pousse && w[1].pousse && w[0].phrase == w[1].phrase && w[1].transition.is_none() && w[0].rush == w[1].rush {
                let fin = w[0].zone.w / w[0].z_fin;
                assert!((fin - w[1].zone.w).abs() < 1.0, "la poussée repart de zéro : {} puis {}", fin, w[1].zone.w);
            }
        }
        let mut par_cadre: HashMap<&str, Vec<f64>> = HashMap::new();
        for pl in &prog.plans {
            if !pl.pousse {
                par_cadre.entry(pl.cadre.as_str()).or_default().push((pl.zone.w * 10.0).round() / 10.0);
            }
        }
        for (k, v) in &par_cadre {
            let mut u = v.clone();
            u.sort_by(|a, b| b.partial_cmp(a).unwrap());
            u.dedup();
            println!("{k:7} {} plans, largeurs du cadre dans la source : {u:?}", v.len());
        }
        for pl in prog.plans.iter().take(16) {
            println!("{:>7.2} {:7} {:12} {:>7.1} punch={} pousse={}", pl.sortie_t0, pl.cadre, pl.regime, pl.zone.w, pl.punch, pl.pousse);
        }
        let large = &par_cadre["large"];
        assert!(large.iter().all(|w| (*w - large[0]).abs() < 1.0), "un plan large n'est pas l'image entière");
    }
}
