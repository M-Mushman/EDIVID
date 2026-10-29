"""Fin de vidéo : le titre, le logo by Edina signé à la main dessous, l'adresse du site et le courriel ; un fichier par
format.

Usage : python fin.py <logo.png> <final.wav> <dossier_sortie>

Fichier à réutiliser tel quel au bout de chaque vidéo : on le range dans la bibliothèque (ranger.py fin).
Formats : les six de sortie d'EDIVID (horizontal, carré, vertical ; 1080 et 2160). MP4 H.264 High, SDR BT.709, 30 i/s, durée de la musique : la
version `final` d'un jingle (declinaisons.py), une phrase de 4 mesures et l'accord final qui s'éteint, environ 9,7 s ;
son AAC 48 kHz. Fond blanc : l'intro part au blanc, la fin en
revient. Déroulé :
- 0 → 0,5 s : le blanc s'ouvre sur un fond très légèrement chaud ;
- 0,3 → 1,0 s : le titre apparaît en montant légèrement, comme dans l'intro ;
- 1,3 → 1,6 s : « by » apparaît ; 1,5 → 3,4 s : « Edina » se trace de gauche à droite, le front de l'encre suivant
  la pente de l'écriture, comme une signature ;
- 3,6 → 4,1 s : l'adresse du site et le courriel apparaissent ;
- les 1,5 dernières secondes, pendant que l'accord final s'éteint : tout repart au blanc.
Écrit aussi fin_<format>.png, la dernière image pleine, pour juger la mise en page.
"""
import subprocess
import sys
from pathlib import Path

import numpy as np
import soundfile as sf
from PIL import Image, ImageDraw, ImageFont

GRAS = "/usr/share/fonts/opentype/urw-base35/NimbusSans-Bold.otf"
NORMAL = "/usr/share/fonts/opentype/urw-base35/NimbusSans-Regular.otf"
LIGNES_H = ["Être heureux et en forme après 50 ans,", "c’est possible !"]
LIGNES_V = ["Être heureux", "et en forme", "après 50 ans,", "c’est possible !"]
ADRESSE = "www.by-edina.ch"
COURRIEL = "ciao@by-edina.ch"
ENCRE = (30, 30, 36)
GRIS = (95, 95, 105)
FOND = (253, 251, 248)
IPS = 30
# Les six formats de sortie d'EDIVID (src/decoupe.rs) : horizontal, carré, vertical, en 1080 et en 2160.
FORMATS = [(1920, 1080), (1080, 1080), (1080, 1920), (3840, 2160), (2160, 2160), (2160, 3840)]
BLANC = 1.5  # retour au blanc final, pendant l'extinction de l'accord
PENTE = 0.35  # inclinaison de l'écriture : le haut d'une lettre est à droite de son pied


def lisse(t: float) -> float:
    t = min(1.0, max(0.0, t))
    return t * t * (3 - 2 * t)


def texte(lignes: list[str], police: str, largeur_max: int, taille_max: int, couleur) -> Image.Image:
    """Lignes centrées sur fond transparent, à la plus grande taille qui tienne dans `largeur_max`."""
    taille = taille_max
    while True:
        f = ImageFont.truetype(police, taille)
        larg = max(f.getlength(l) for l in lignes)
        if larg <= largeur_max or taille <= 10:
            break
        taille -= 2
    interligne = round(taille * 1.12)
    marge = round(taille * 0.3)  # l'accent du Ê dépasse la hauteur des capitales
    img = Image.new("RGBA", (round(larg) + 4, marge + interligne * len(lignes) + round(taille * 0.3)), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    for i, l in enumerate(lignes):
        d.text(((img.width - f.getlength(l)) / 2, marge + i * interligne), l, font=f, fill=couleur + (255,))
    return img


def avec_opacite(img: Image.Image, a: float) -> Image.Image:
    if a >= 1:
        return img
    out = img.copy()
    out.putalpha(img.getchannel("A").point(lambda v: round(v * a)))
    return out


class Signature:
    """Le logo séparé en « by » (noir) et « Edina » (magenta), pour les faire apparaître l'un après l'autre."""

    def __init__(self, logo: Image.Image):
        a = np.asarray(logo, dtype=np.float32)
        magenta = (a[..., 0] > 120) & (a[..., 2] > 120) & (a[..., 1] < 160)
        self.rgb = a[..., :3]
        self.alpha = a[..., 3]
        self.magenta = magenta
        h, w = magenta.shape
        cols = np.nonzero(magenta.any(axis=0))[0]
        y, x = np.mgrid[0:h, 0:w].astype(np.float32)
        # Coordonnée du tracé : la position horizontale ramenée au pied de la lettre.
        self.u = x - PENTE * (h - y)
        self.u0, self.u1 = self.u[magenta].min(), self.u[magenta].max()
        self.flou = 0.04 * (cols[-1] - cols[0])

    def image(self, apparition_by: float, trace: float) -> Image.Image:
        front = self.u0 - self.flou + trace * (self.u1 - self.u0 + 2 * self.flou)
        encre = np.clip((front - self.u) / self.flou, 0, 1)
        k = np.where(self.magenta, encre, apparition_by)
        out = np.dstack([self.rgb, self.alpha * k]).astype(np.uint8)
        return Image.fromarray(out, "RGBA")


def mise_en_page(w: int, h: int, logo_src: Image.Image):
    if h > w:
        t = texte(LIGNES_V, GRAS, round(0.84 * w), round(0.10 * w), ENCRE)
        lw = round(0.74 * w)
        u = texte([ADRESSE, COURRIEL], NORMAL, round(0.8 * w), round(0.052 * w), GRIS)
    elif h == w:
        t = texte(LIGNES_V, GRAS, round(0.7 * w), round(0.075 * w), ENCRE)
        lw = round(0.52 * w)
        u = texte([ADRESSE, COURRIEL], NORMAL, round(0.6 * w), round(0.038 * w), GRIS)
    else:
        t = texte(LIGNES_H, GRAS, round(0.78 * w), round(0.052 * w), ENCRE)
        lw = round(0.38 * w)
        u = texte([ADRESSE, COURRIEL], NORMAL, round(0.5 * w), round(0.024 * w), GRIS)
    logo = logo_src.resize((lw, round(lw * logo_src.height / logo_src.width)), Image.LANCZOS)
    ecart = round(0.035 * max(w, h))
    total = t.height + ecart + logo.height + ecart // 2 + u.height
    y0 = (h - total) // 2
    pos_t = ((w - t.width) // 2, y0)
    pos_l = ((w - logo.width) // 2, y0 + t.height + ecart)
    pos_u = ((w - u.width) // 2, pos_l[1] + logo.height + ecart // 2)
    return t, Signature(logo), logo.size, u, pos_t, pos_l, pos_u


def image(n: int, duree: float, w: int, h: int, calques) -> Image.Image:
    t_img, sig, _, u_img, pos_t, pos_l, pos_u = calques
    s = n / IPS
    img = Image.new("RGBA", (w, h), FOND + (255,))
    a = lisse((s - 0.3) / 0.7)
    if a > 0:
        montee = round(0.015 * h * (1 - a))
        img.alpha_composite(avec_opacite(t_img, a), (pos_t[0], pos_t[1] + montee))
    by, trace = lisse((s - 1.3) / 0.3), lisse((s - 1.5) / 1.9)
    if by > 0 or trace > 0:
        img.alpha_composite(sig.image(by, trace), pos_l)
    a = lisse((s - 3.6) / 0.5)
    if a > 0:
        img.alpha_composite(avec_opacite(u_img, a), pos_u)
    blanc = max(1 - lisse(s / 0.5), lisse((s - (duree - BLANC)) / BLANC))
    if blanc > 0:
        img = Image.blend(img, Image.new("RGBA", (w, h), (255, 255, 255, 255)), blanc)
    return img.convert("RGB")


def rendre(logo_src: Image.Image, musique: Path, duree: float, w: int, h: int, dossier: Path) -> None:
    calques = mise_en_page(w, h, logo_src)
    nb = round(duree * IPS)
    image(round((duree - BLANC - 0.3) * IPS), duree, w, h, calques).save(dossier / f"fin_{w}x{h}.png")
    sortie = dossier / f"fin_{w}x{h}.mp4"
    p = subprocess.Popen([
        "ffmpeg", "-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{w}x{h}", "-r", str(IPS),
        "-i", "-", "-i", str(musique), "-map", "0:v", "-map", "1:a",
        "-af", "loudnorm=I=-16:TP=-1.5:LRA=11,aresample=48000", "-t", f"{duree:.3f}",
        "-vf", "format=yuv420p", "-c:v", "libx264", "-profile:v", "high", "-preset", "slow",
        "-crf", "16" if w * h <= 1920 * 1080 else "18", "-colorspace", "bt709", "-color_primaries", "bt709",
        "-color_trc", "bt709", "-color_range", "tv", "-c:a", "aac", "-b:a", "192k", "-ar", "48000",
        "-movflags", "+faststart", str(sortie)], stdin=subprocess.PIPE)
    for n in range(nb):
        p.stdin.write(image(n, duree, w, h, calques).tobytes())
    p.stdin.close()
    if p.wait() != 0:
        raise SystemExit(f"ffmpeg a échoué : {sortie}")
    print("écrit :", sortie)


def main() -> None:
    logo, musique, dossier = map(Path, sys.argv[1:4])
    dossier.mkdir(parents=True, exist_ok=True)
    src = Image.open(logo).convert("RGBA")
    duree = sf.info(musique).duration
    for w, h in FORMATS:
        rendre(src, musique, duree, w, h, dossier)


if __name__ == "__main__":
    main()
