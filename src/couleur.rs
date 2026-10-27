//! Couleurs des éléments incrustés (sous-titres, textes, logos, images) selon la sortie : SDR Rec.709 ou HDR HLG
//! (BT.2100), et compensation de la matrice BT.601 que ffmpeg emploie pour dessiner les sous-titres et les textes.
//!
//! Mesuré le 2026-10-27 : `subtitles` et `drawtext` passent une couleur RVB en YUV avec la matrice BT.601, quelle
//! que soit la matrice de l'image. Le jaune des sous-titres sortait donc un peu faux dans une vidéo Rec.709, et
//! tout sortirait faux dans une vidéo BT.2020. On donne à ffmpeg la couleur qui, passée par BT.601, donne le YUV
//! voulu dans la matrice de la sortie.

/// Coefficients de luminance (Kr, Kb) d'une matrice YUV.
#[derive(Clone, Copy)]
pub enum Matrice {
    Bt601,
    Bt709,
    Bt2020,
}

impl Matrice {
    fn k(self) -> (f64, f64) {
        match self {
            Matrice::Bt601 => (0.299, 0.114),
            Matrice::Bt709 => (0.2126, 0.0722),
            Matrice::Bt2020 => (0.2627, 0.0593),
        }
    }

    /// R'G'B' (0–1) → Y'CbCr (Y 0–1, Cb et Cr −0,5–0,5).
    fn vers_yuv(self, [r, g, b]: [f64; 3]) -> [f64; 3] {
        let (kr, kb) = self.k();
        let y = kr * r + (1.0 - kr - kb) * g + kb * b;
        [y, (b - y) / (2.0 * (1.0 - kb)), (r - y) / (2.0 * (1.0 - kr))]
    }

    fn depuis_yuv(self, [y, cb, cr]: [f64; 3]) -> [f64; 3] {
        let (kr, kb) = self.k();
        let r = y + 2.0 * (1.0 - kr) * cr;
        let b = y + 2.0 * (1.0 - kb) * cb;
        let g = (y - kr * r - kb * b) / (1.0 - kr - kb);
        [r, g, b]
    }
}

/// Transfert BT.1886 (gamma 2,4) d'une couleur d'écran SDR, du signal vers la lumière.
fn lineaire_sdr(v: f64) -> f64 {
    v.clamp(0.0, 1.0).powf(2.4)
}

/// HLG (BT.2100), de la lumière de scène (0–1) vers le signal.
fn hlg_oetf(e: f64) -> f64 {
    const A: f64 = 0.178_832_77;
    const B: f64 = 0.284_668_92;
    const C: f64 = 0.559_910_73;
    let e = e.max(0.0);
    if e <= 1.0 / 12.0 { (3.0 * e).sqrt() } else { A * (12.0 * e - B).ln() + C }
}

/// Couleur SDR Rec.709 (R'G'B' 0–1) → couleur HLG BT.2020 (R'G'B' 0–1), selon BT.2408 : le blanc SDR devient le
/// blanc de référence HDR (203 cd/m², 75 % du signal) sur un écran de 1 000 cd/m², par l'OOTF inverse (gamma 1,2).
pub fn hlg_depuis_sdr(rgb: [f64; 3]) -> [f64; 3] {
    let l = rgb.map(lineaire_sdr);
    // Primaires Rec.709 → BT.2020 (lumière linéaire), BT.2087.
    let m = [[0.6274, 0.3293, 0.0433], [0.0691, 0.9195, 0.0114], [0.0164, 0.0880, 0.8956]];
    let d = [0, 1, 2].map(|i| (m[i][0] * l[0] + m[i][1] * l[1] + m[i][2] * l[2]) * 203.0 / 1000.0);
    let yd = 0.2627 * d[0] + 0.6780 * d[1] + 0.0593 * d[2];
    let gain = if yd > 0.0 { yd.powf(1.0 / 1.2 - 1.0) } else { 0.0 };
    d.map(|v| hlg_oetf(v * gain))
}

/// La couleur à donner à ffmpeg (qui la passera en YUV par BT.601) pour obtenir `rgb` encodée dans `cible`.
pub fn pour_ffmpeg(rgb: [f64; 3], cible: Matrice) -> [u8; 3] {
    Matrice::Bt601.depuis_yuv(cible.vers_yuv(rgb)).map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Couleur RVB 8 bits d'un élément SDR, prête pour ffmpeg dans la sortie voulue.
pub fn couleur_sortie([r, g, b]: [u8; 3], hdr: bool) -> [u8; 3] {
    let rgb = [r, g, b].map(|v| v as f64 / 255.0);
    if hdr { pour_ffmpeg(hlg_depuis_sdr(rgb), Matrice::Bt2020) } else { pour_ffmpeg(rgb, Matrice::Bt709) }
}

/// « 0xRRGGBB » pour drawtext et les sources de couleur.
pub fn hex_ffmpeg(rgb: [u8; 3], hdr: bool) -> String {
    let [r, g, b] = couleur_sortie(rgb, hdr);
    format!("0x{r:02X}{g:02X}{b:02X}")
}

/// Réécrit les couleurs d'un fichier de sous-titres ASS (« &HAABBGGRR » ou « &HBBGGRR ») pour la sortie.
pub fn couleurs_ass(texte: &str, hdr: bool) -> String {
    let mut sortie = String::with_capacity(texte.len());
    let mut reste = texte;
    while let Some(i) = reste.find("&H") {
        sortie.push_str(&reste[..i + 2]);
        reste = &reste[i + 2..];
        let n = reste.bytes().take_while(|b| b.is_ascii_hexdigit()).count();
        if n == 6 || n == 8 {
            let (alpha, bgr) = reste[..n].split_at(n - 6);
            let v = |k: usize| u8::from_str_radix(&bgr[k..k + 2], 16).unwrap_or(0);
            let [r, g, b] = couleur_sortie([v(4), v(2), v(0)], hdr);
            sortie.push_str(&format!("{alpha}{b:02X}{g:02X}{r:02X}"));
            reste = &reste[n..];
        }
    }
    sortie.push_str(reste);
    sortie
}

/// Table 3D (.cube, 33 niveaux) SDR Rec.709 → HLG BT.2020, pour les images incrustées (logos, médias, générique).
pub fn table_sdr_vers_hlg() -> String {
    let n = 33;
    let mut t = format!("LUT_3D_SIZE {n}\n");
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                let c = hlg_depuis_sdr([r, g, b].map(|v| v as f64 / (n - 1) as f64));
                t.push_str(&format!("{:.6} {:.6} {:.6}\n", c[0], c[1], c[2]));
            }
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blanc_de_reference() {
        let b = hlg_depuis_sdr([1.0, 1.0, 1.0]);
        for v in b {
            assert!((v - 0.75).abs() < 0.002, "{b:?}");
        }
        assert_eq!(hlg_depuis_sdr([0.0, 0.0, 0.0]), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn gris_et_matrices() {
        // Un gris ne dépend pas de la matrice.
        assert_eq!(couleur_sortie([128, 128, 128], false), [128, 128, 128]);
        assert_eq!(couleur_sortie([255, 255, 255], true), [191, 191, 191]);
        // Le jaune, repassé par BT.601, redonne son YUV Rec.709.
        let j = couleur_sortie([255, 229, 0], false).map(|v| v as f64 / 255.0);
        let y709 = Matrice::Bt709.vers_yuv([1.0, 229.0 / 255.0, 0.0]);
        let y = Matrice::Bt601.vers_yuv(j);
        for k in 0..3 {
            assert!((y[k] - y709[k]).abs() < 0.01, "{y:?} / {y709:?}");
        }
    }

    #[test]
    fn ass() {
        let t = "Style: Mot,&H00FFFFFF,&H0000E5FF,&H64000000 \\c&H0000E5FF&";
        let h = couleurs_ass(t, true);
        assert!(h.contains("&H00BFBFBF") && h.contains("&H64000000"), "{h}");
        assert_eq!(couleurs_ass("sans couleur", false), "sans couleur");
    }
}
