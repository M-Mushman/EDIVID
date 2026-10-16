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
const ZOOM_MAX: f64 = 4.5;
/// Au-delà de cet agrandissement, la netteté IA est appliquée au plan (rendu final).
pub const SEUIL_NETTETE: f64 = 1.6;
const ZOOM_TRANSITION: f64 = 1.4;
const DUREE_MIN_PLAN: f64 = 0.3;

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
            });
            fin_prec = t1;
        }
    }
    res
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
    let (l, h) = match format.as_str() {
        "horizontal" => (1920, 1080),
        "carre" => (1080, 1080),
        _ => (1080, 1920),
    };
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

/// Cadre de sortie dans la source, centré sur (cx, cy) et borné à l'image.
fn cadre_dans(source: (f64, f64), base: (f64, f64), z: f64, cx: f64, cy: f64) -> Rect {
    let (w, h) = (base.0 / z, base.1 / z);
    let x = (cx - w / 2.0).clamp(0.0, (source.0 - w).max(0.0));
    let y = (cy - h / 2.0).clamp(0.0, (source.1 - h).max(0.0));
    Rect { x, y, w, h }
}

struct Cadrage {
    zone: Rect,
    z_fin: f64,
    centre_serre: (f64, f64),
    agrandissement: f64,
}

#[allow(clippy::too_many_arguments)]
fn cadrer(a: &Analyse, t0: f64, t1: f64, cadre: &str, z_mul_fin: f64, z_mul: f64, zooms: &str,
          sortie: (u32, u32), z_transition: f64) -> Cadrage {
    let source = (a.largeur as f64, a.hauteur as f64);
    let aspect = sortie.0 as f64 / sortie.1 as f64;
    let base = if source.0 / source.1 > aspect { (source.1 * aspect, source.1) } else { (source.0, source.0 / aspect) };
    let intensite = match zooms {
        "doux" => 0.8,
        "fort" => 1.2,
        _ => 1.0,
    };
    let visage = visage_median(a, t0, t1, false);
    let (fx, fy, fw) = match visage {
        Some(b) => (((b[0] + b[2]) / 2.0) * source.0, ((b[1] + b[3]) / 2.0) * source.1, (b[2] - b[0]) * source.0),
        None => (source.0 / 2.0, source.1 / 2.0, 0.0),
    };
    let (cible, hauteur) = match cadre {
        "moyen" => (0.17, 0.36),
        "visage" => (0.30, 0.42),
        "yeux" => (0.46, 0.47),
        _ => (0.0, 0.5),
    };
    let mut z = if cible > 0.0 && fw > 0.0 { (cible * intensite / (fw / base.0)).clamp(1.0, ZOOM_MAX) } else { 1.0 };
    z = (z * z_mul).min(ZOOM_MAX);
    let (mut cx, mut cy) = (fx, fy);
    if cadre == "yeux" {
        if let Some(b) = visage_median(a, t0, t1, true) {
            cx = (b[0] + b[2]) / 2.0 * source.0;
            cy = (b[1] + b[3]) / 2.0 * source.1;
        }
    }
    if cible > 0.0 {
        cy += (0.5 - hauteur) * base.1 / z;
    } else if visage.is_some() {
        // Plan large : le visage reste au tiers supérieur si la source dépasse le cadre.
        cy = fy + 0.15 * base.1;
    }
    let zone = cadre_dans(source, base, z, cx, cy);
    let z_fin_abs = (z * z_mul_fin).min(ZOOM_MAX);
    let z_max_abs = (z_fin_abs * z_transition).min(ZOOM_MAX * ZOOM_TRANSITION);
    let serre = cadre_dans(source, base, z_max_abs, cx, cy);
    let centre_serre = (
        ((serre.x + serre.w / 2.0 - zone.x) / zone.w).clamp(0.0, 1.0),
        ((serre.y + serre.h / 2.0 - zone.y) / zone.h).clamp(0.0, 1.0),
    );
    Cadrage { zone, z_fin: z_fin_abs / z, centre_serre, agrandissement: sortie.0 as f64 / serre.w }
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
    };
    // Corps : phrases gardées, dans l'ordre.
    let corps: Vec<&Phrase> = ps
        .iter()
        .filter(|ph| decisions.get(ph.cle.as_str()).map(|d| d.garder).unwrap_or(true))
        .collect();
    let debut_corps: HashSet<&str> = corps.iter().take(3).map(|ph| ph.cle.as_str()).collect();
    let mut entrees: Vec<Entree> = Vec::new();
    if vrai(r, "accroche") {
        let mut duree = 0.0;
        for cle in montage.map(|m| m.accroche.clone()).unwrap_or_default() {
            // Garde : l'accroche ne redit pas l'introduction.
            if debut_corps.contains(cle.as_str()) {
                continue;
            }
            if let Some(ph) = par_cle.get(cle.as_str()) {
                if duree + (ph.t1 - ph.t0) > 10.0 {
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
        if entrees.is_empty() {
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
        let Some(a) = analyses_par_rush.get(ph.rush.as_str()) else { continue };
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
            && (premier_du_corps || partie.is_some() || d.transition.is_some());
        if veut && style != "coupe" {
            transition = Some(if style == "auto" { d.transition.clone().unwrap_or_else(|| "flash".into()) } else { style.clone() });
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
            let punch = d.regime == "rafale" && k % 2 == 1 && vrai(r, "punch_jumpcut") && zooms != "aucun";
            let z_mul = if punch { 1.12 } else { 1.0 };
            let anime = vrai(r, "zoom_anime") && zooms != "aucun";
            let z_mul_fin = match d.regime.as_str() {
                "impact" if anime => 1.12,
                "respiration" if anime => 1.05,
                _ => 1.0,
            };
            let trans = if k == 0 { transition.clone() } else { None };
            let z_trans = if trans.as_deref() == Some("zoom") { ZOOM_TRANSITION } else { 1.0 };
            let c = cadrer(a, t0, t0 + duree, &d.cadre, z_mul_fin, z_mul, &zooms, (largeur, hauteur), z_trans);
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
            let veut_nettete = !apercu && nettete != "non";
            plans.push(Plan {
                rush: ph.rush.clone(),
                phrase: ph.cle.clone(),
                t0,
                duree,
                sortie_t0: sortie_t,
                cadre: d.cadre.clone(),
                regime: d.regime.clone(),
                accroche: e.accroche,
                zone: c.zone,
                z_fin: c.z_fin,
                centre_serre: c.centre_serre,
                transition: trans,
                sortie: None,
                agrandissement: c.agrandissement,
                nettete: veut_nettete && c.agrandissement > SEUIL_NETTETE,
            });
            sortie_t += duree;
        }
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

/// Plans, avec leur place dans le montage (t, d) et dans le rush d'origine (src).
pub fn plans_json(prog: &Programme) -> Value {
    json!(prog
        .plans
        .iter()
        .map(|pl| json!({"t": arr(pl.sortie_t0), "d": arr(pl.duree), "src": arr(pl.t0), "rush": pl.rush,
                         "phrase": pl.phrase, "regime": pl.regime, "cadre": pl.cadre, "accroche": pl.accroche,
                         "transition": pl.transition}))
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
        "incrustations": prog.incrustations.len(),
        "avertissements": prog.avertissements,
    })
}
