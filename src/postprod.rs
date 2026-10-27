//! PostProd : traitements de l'image appliqués au montage entier (aperçu et rendu final), avant les
//! incrustations et les sous-titres, qui restent nets. Pensé pour l'image de mode et de beauté.

use serde::{Deserialize, Serialize};
use serde_json::Value;

const LOOKS: &str = include_str!("../bibliotheque/looks.json");

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PostProd {
    /// Look de la bibliothèque appliqué en dernier (repère pour l'interface).
    pub look: String,
    pub couleur: String,
    /// Ajustements fins, de -100 à 100.
    pub exposition: f64,
    pub contraste: f64,
    pub saturation: f64,
    pub temperature: f64,
    /// Intensités, de 0 à 100.
    pub lissage: f64,
    pub diffusion: f64,
    pub nettete: f64,
    pub grain: f64,
    pub vignette: f64,
    pub bandes: bool,
}

impl Default for PostProd {
    fn default() -> Self {
        Self {
            look: String::new(),
            couleur: "naturel".into(),
            exposition: 0.0,
            contraste: 0.0,
            saturation: 0.0,
            temperature: 0.0,
            lissage: 0.0,
            diffusion: 0.0,
            nettete: 0.0,
            grain: 0.0,
            vignette: 0.0,
            bandes: false,
        }
    }
}

pub const COULEURS: &[(&str, &str, &str)] = &[
    ("naturel", "Naturel", ""),
    ("beaute", "Beauté", "colorbalance=rm=0.04:gm=-0.01:bm=-0.02:rh=0.03,eq=brightness=0.02:saturation=1.05:gamma=1.03"),
    ("editorial", "Éditorial", "eq=contrast=1.12:saturation=0.82,curves=all='0/0.04 0.5/0.5 1/0.97'"),
    ("pastel", "Pastel", "eq=saturation=0.85:brightness=0.04:contrast=0.92,colorbalance=rs=0.03:bs=0.04"),
    ("eclatant", "Éclatant", "eq=contrast=1.06:saturation=1.18:gamma=0.98"),
    ("chaud", "Chaud", "colortemperature=temperature=5200:mix=0.6,eq=saturation=1.08"),
    ("froid", "Froid", "colortemperature=temperature=8000:mix=0.5"),
    ("cinema", "Cinéma", "eq=contrast=1.1:saturation=0.9,colorbalance=rs=-0.05:bs=0.08:rh=0.08:bh=-0.06"),
    // Lumineux et doux : noirs à peine relevés, hautes lumières qui ne brûlent pas, tons moyens éclaircis ;
    // ombres légèrement froides, peau chaude.
    ("lumiere", "Lumière cinéma", "curves=all='0/0.035 0.25/0.28 0.5/0.56 0.75/0.81 1/0.975',\
                                   colorbalance=rs=-0.015:bs=0.025:rm=0.02:bm=-0.01:rh=0.03:gh=0.01:bh=-0.02,eq=saturation=0.95"),
    ("film", "Film 35 mm", "curves=r='0/0.05 0.5/0.52 1/0.98':g='0/0.04 0.5/0.5 1/0.96':b='0/0.08 0.5/0.47 1/0.9',eq=saturation=0.95"),
    ("couture_nb", "Noir et blanc couture", "hue=s=0,curves=all='0/0 0.25/0.18 0.75/0.85 1/1',eq=contrast=1.1"),
    ("noir_blanc", "Noir et blanc", "hue=s=0,eq=contrast=1.15"),
    ("vintage", "Vintage", "curves=preset=vintage"),
    ("nuit", "Nuit bleue", "colortemperature=temperature=9000:mix=0.6,eq=brightness=-0.03:saturation=0.9"),
];

pub fn looks() -> Value {
    serde_json::from_str(LOOKS).expect("bibliotheque/looks.json invalide")
}

impl PostProd {
    pub fn borner(&mut self) {
        if !COULEURS.iter().any(|(c, _, _)| *c == self.couleur) {
            self.couleur = "naturel".into();
        }
        for v in [&mut self.exposition, &mut self.contraste, &mut self.saturation, &mut self.temperature] {
            *v = v.clamp(-100.0, 100.0);
        }
        for v in [&mut self.lissage, &mut self.diffusion, &mut self.nettete, &mut self.grain, &mut self.vignette] {
            *v = v.clamp(0.0, 100.0);
        }
    }

    /// La partie couleur seule (colorimétrie, exposition, contraste, saturation, température) : elle ne dépend
    /// que de la couleur de chaque pixel, et se met donc en table (LUT) pour l'aperçu en direct du navigateur.
    pub fn filtres_couleur(&self) -> Vec<String> {
        let mut etapes: Vec<String> = Vec::new();
        if let Some((_, _, f)) = COULEURS.iter().find(|(c, _, _)| *c == self.couleur) {
            if !f.is_empty() {
                etapes.push(f.to_string());
            }
        }
        if self.exposition != 0.0 || self.contraste != 0.0 || self.saturation != 0.0 {
            etapes.push(format!(
                "eq=brightness={:.3}:contrast={:.3}:saturation={:.3}",
                self.exposition / 400.0,
                1.0 + self.contraste / 250.0,
                1.0 + self.saturation / 100.0
            ));
        }
        if self.temperature != 0.0 {
            // -100 : froid (9 500 K) ; +100 : chaud (3 500 K) ; 6 500 K est neutre.
            let k = 6500.0 - self.temperature * 30.0;
            etapes.push(format!("colortemperature=temperature={k:.0}:mix={:.2}", (self.temperature.abs() / 100.0) * 0.7));
        }
        etapes
    }

    /// Chaîne de filtres ffmpeg, entre [entree] et [sortie] ; largeur et hauteur de l'image traitée.
    pub fn filtre(&self, entree: &str, sortie: &str, largeur: u32, hauteur: u32) -> String {
        self.filtre_avec_table(entree, sortie, largeur, hauteur, None)
    }

    /// Même chaîne ; avec `table` (fichier .cube de la partie couleur), la couleur passe par lut3d, qui garde la
    /// profondeur de l'image (10 bits), là où les filtres de couleur de ffmpeg ne travaillent qu'en 8 bits.
    pub fn filtre_avec_table(&self, entree: &str, sortie: &str, largeur: u32, hauteur: u32, table: Option<&std::path::Path>) -> String {
        let echelle = largeur.min(hauteur) as f64 / 1080.0;
        let etapes = match table {
            Some(t) => vec![format!("lut3d=file='{}':interp=tetrahedral", t.to_string_lossy())],
            None => self.filtres_couleur(),
        };
        let mut chaine = format!("[{entree}]{}", if etapes.is_empty() { "null".to_string() } else { etapes.join(",") });
        let mut n = 0;
        let mut etiquette = |chaine: &mut String, suite: String| {
            n += 1;
            chaine.push_str(&format!("[pp{n}];{suite}"));
            n
        };
        // Lissage du teint : le flou bilatéral adoucit la peau sans toucher aux contours (yeux, cheveux).
        if self.lissage > 0.0 {
            let s = (4.0 + 6.0 * self.lissage / 100.0) * echelle;
            let o = 0.85 * self.lissage / 100.0;
            let r = 0.06 + 0.08 * self.lissage / 100.0;
            let k = etiquette(&mut chaine, String::new());
            chaine.push_str(&format!(
                "[pp{k}]split[ppl{k}a][ppl{k}b];[ppl{k}b]bilateral=sigmaS={s:.1}:sigmaR={r:.3}[ppl{k}c];\
                 [ppl{k}a][ppl{k}c]blend=all_mode=normal:all_opacity={o:.2}"
            ));
        }
        // Diffusion (« Pro-Mist ») : un halo doux dans les hautes lumières.
        if self.diffusion > 0.0 {
            let sigma = 14.0 * echelle;
            let o = 0.35 * self.diffusion / 100.0;
            let k = etiquette(&mut chaine, String::new());
            chaine.push_str(&format!(
                "[pp{k}]split[ppd{k}a][ppd{k}b];[ppd{k}b]gblur=sigma={sigma:.1}[ppd{k}c];\
                 [ppd{k}a][ppd{k}c]blend=all_mode=screen:all_opacity={o:.2}"
            ));
        }
        let mut fin: Vec<String> = Vec::new();
        // Netteté adaptative au contraste (CAS), sur la luminance : elle relève les détails fins sans halo autour
        // des contours, là où un masque flou (unsharp) les redessinait.
        if self.nettete > 0.0 {
            fin.push(format!("cas=strength={:.3}:planes=1", self.nettete / 100.0));
        }
        if self.grain > 0.0 {
            fin.push(format!("noise=c0s={:.0}:c0f=t", self.grain / 100.0 * 22.0));
        }
        if self.vignette > 0.0 {
            fin.push(format!("vignette=angle={:.3}", 0.25 + 0.55 * self.vignette / 100.0));
        }
        if self.bandes && largeur > hauteur {
            let bande = ((hauteur as f64 - largeur as f64 / 2.39) / 2.0).max(0.0).round();
            fin.push(format!("drawbox=x=0:y=0:w=iw:h={bande}:color=black:t=fill,drawbox=x=0:y=ih-{bande}:w=iw:h={bande}:color=black:t=fill"));
        }
        if !fin.is_empty() {
            let k = etiquette(&mut chaine, String::new());
            chaine.push_str(&format!("[pp{k}]{}", fin.join(",")));
        }
        chaine.push_str(&format!("[{sortie}]"));
        // Nettoyage : « [ppN]; » suivi d'une chaîne vide ne doit pas exister.
        chaine.replace(";;", ";")
    }
}
