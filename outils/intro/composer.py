"""Assemble l'intro : vidéo animée, logo, titre, jingle, fondu au blanc ; un fichier par format.

Usage : python composer.py <video_horizontale.mp4> <video_verticale.mp4> <intro.wav> <logo.png> <dossier_sortie>

Le son est la version `intro` d'un jingle (declinaisons.py), pris en entier : la dernière mesure du morceau et son
accord final, une fin franche. L'intro dure autant que lui (environ 3,5 s) ; l'animation (3,1 s) garde sa dernière
image jusqu'au bout.

Formats : les six de sortie d'EDIVID. 3840×2160 et 1920×1080 depuis la vidéo horizontale ; 2160×3840, 1080×1920,
2160×2160 et 1080×1080 depuis la verticale. Un format déjà écrit n'est pas refait. MP4 H.264 High, SDR BT.709, 30 i/s,
son AAC 48 kHz. Titre en Nimbus Sans Bold (reprise libre d'Helvetica, URW), « c’est possible ! » dans le rose du logo.
- horizontal : logo en haut à gauche, titre à gauche sur le fond, la personne à droite ;
- vertical : logo centré en haut, titre centré dessous, au-dessus de la tête ;
- carré : la fenêtre carrée glisse dans la verticale jusqu'à mettre le visage aux deux tiers de la hauteur ; logo et
  titre, plus petits, centrés au-dessus de la tête.
Le logo est là dès la première image ; le titre apparaît en montant légèrement (0,25 → 0,8 s) ; tout part au blanc
dans la dernière demi-seconde.
"""
import subprocess
import sys
from pathlib import Path

import cv2
import soundfile as sf
import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

POLICE = "/usr/share/fonts/opentype/urw-base35/NimbusSans-Bold.otf"
LIGNES = ["Être heureux", "et en forme", "après 50 ans,", "c’est possible !"]
ENCRE = (30, 30, 36, 255)
FUCHSIA = (255, 0, 255, 255)  # le rose d'« Edina » dans le logo by Edina : la dernière ligne du titre
IPS, FONDU = 30, 0.5
YUNET = Path(__file__).resolve().parents[2] / "modeles/yunet/face_detection_yunet_2023mar.onnx"
# (largeur, hauteur, vidéo source) : les six formats de sortie d'EDIVID (src/decoupe.rs).
FORMATS = [(3840, 2160, "h"), (1920, 1080, "h"), (2160, 3840, "v"), (1080, 1920, "v"), (2160, 2160, "v"), (1080, 1080, "v")]


def titre(largeur_max: int, taille_max: int, centre: bool) -> Image.Image:
    """Le titre sur fond transparent, à la plus grande taille qui tienne dans `largeur_max`."""
    taille = taille_max
    while True:
        f = ImageFont.truetype(POLICE, taille)
        larg = max(f.getlength(l) for l in LIGNES)
        if larg <= largeur_max or taille <= 10:
            break
        taille -= 2
    interligne = round(taille * 1.08)
    # Marge au-dessus : l'accent du Ê dépasse la hauteur des capitales.
    marge = round(taille * 0.3)
    img = Image.new("RGBA", (round(larg) + 4, marge + interligne * len(LIGNES) + round(taille * 0.3)), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    for i, l in enumerate(LIGNES):
        x = (img.width - f.getlength(l)) / 2 if centre else 0
        d.text((x, marge + i * interligne), l, font=f, fill=FUCHSIA if i == len(LIGNES) - 1 else ENCRE)
    return img


def cadrage(w: int, h: int, haut: int | None) -> str:
    """La vidéo agrandie jusqu'à couvrir le format, coupée au centre, ou à `haut` pixels du bord supérieur."""
    y = "(ih-oh)/2" if haut is None else str(haut)
    return f"scale={w}:{h}:force_original_aspect_ratio=increase:flags=lanczos,crop={w}:{h}:(iw-ow)/2:{y}"


def haut_carre(video: Path, w: int) -> int:
    """Carré tiré de la verticale : bord supérieur de la fenêtre, d'après le visage repéré sur la première image."""
    brut = subprocess.run(["ffmpeg", "-v", "error", "-i", str(video), "-frames:v", "1", "-vf", f"scale={w}:-2",
                           "-f", "rawvideo", "-pix_fmt", "bgr24", "-"], capture_output=True, check=True).stdout
    hv = len(brut) // (3 * w)
    img = np.frombuffer(brut, np.uint8)[:hv * w * 3].reshape(hv, w, 3)
    d = cv2.FaceDetectorYN.create(str(YUNET), "", (w, hv), 0.6)
    _, f = d.detect(img)
    if f is None:
        return (hv - w) // 2
    x, y, l, t = f[np.argmax(f[:, 14])][:4]
    # Visage aux deux tiers ; un gros plan descend davantage, pour que le haut de la tête (cheveux compris, 0,4 de
    # la hauteur du visage) reste sous le titre, qui finit vers 0,47 de la hauteur ; sans jamais couper le menton.
    haut = max(min(y + t / 2 - 0.68 * w, y - 0.4 * t - 0.48 * w), y + 1.15 * t - 0.97 * w)
    return int(np.clip(haut, 0, hv - w))


def fond_charge(video: Path, w: int, h: int, zone: tuple[int, int, int, int], haut: int | None = None) -> bool:
    """Le décor sous le titre est-il chargé (rue, pierre, porte) ? Écart-type de la luminance sur la première image."""
    x, y, l, t = zone
    brut = subprocess.run(["ffmpeg", "-v", "error", "-i", str(video), "-frames:v", "1", "-vf",
                           f"{cadrage(w, h, haut)},crop={l}:{t}:{x}:{y}",
                           "-f", "rawvideo", "-pix_fmt", "gray", "-"], capture_output=True, check=True).stdout
    return float(np.frombuffer(brut, np.uint8).std()) > 22  # studio : 3 à 8 ; rue : 40 et plus


def voile(t: Image.Image, marge: int) -> Image.Image:
    """Le titre posé sur un voile blanc translucide aux bords fondus, pour rester lisible sur un décor chargé."""
    img = Image.new("RGBA", (t.width + 2 * marge, t.height + 2 * marge), (255, 255, 255, 0))
    a = Image.new("L", img.size, 0)
    ImageDraw.Draw(a).rounded_rectangle((marge // 2, marge // 2, img.width - marge // 2, img.height - marge // 2),
                                        radius=marge, fill=205)
    img.putalpha(a.filter(ImageFilter.GaussianBlur(marge / 3)))
    img.alpha_composite(t, (marge, marge))
    return img


def calques(w: int, h: int, logo_src: Image.Image, dossier: Path,
            video: Path | None = None, haut: int | None = None) -> tuple[Path, Path, int, int, int, int]:
    """Logo et titre à la taille du format ; renvoie leurs fichiers et positions. Sur un décor chargé, le titre
    reçoit un voile blanc, le logo aussi."""
    vertical = h >= w
    if h > w:
        lw = round(0.46 * w)
        t = titre(round(0.86 * w), round(0.085 * w), centre=True)
    elif h == w:
        lw = round(0.30 * w)
        t = titre(round(0.70 * w), round(0.062 * w), centre=True)
    else:
        lw = round(0.24 * w)
        t = titre(round(0.34 * w), round(0.05 * w), centre=False)
    logo = logo_src.resize((lw, round(lw * logo_src.height / logo_src.width)), Image.LANCZOS)
    if vertical:
        lx, ly = (w - logo.width) // 2, round(0.035 * h)
        tx, ty = (w - t.width) // 2, ly + logo.height
    else:
        lx, ly = round(0.045 * w), round(0.05 * h)
        tx, ty = round(0.055 * w), round(0.36 * h)
    marge = round(0.03 * max(w, h))
    if h == w:
        ly = round(0.03 * h)
        ty = ly + logo.height
    if video is not None and fond_charge(video, w, h, (tx, ty, t.width, min(t.height, h - ty)), haut):
        t, tx, ty = voile(t, marge), tx - marge, ty - marge
    if video is not None and fond_charge(video, w, h, (lx, ly, logo.width, logo.height), haut):
        m = marge // 2
        logo, lx, ly = voile(logo, m), max(0, lx - m), max(0, ly - m)
    fl, ft = dossier / f"logo_{w}x{h}.png", dossier / f"titre_{w}x{h}.png"
    logo.save(fl)
    t.save(ft)
    return fl, ft, lx, ly, tx, ty


def composer(video: Path, son: Path, logo: Image.Image, w: int, h: int, sortie: Path) -> None:
    duree = sf.info(son).duration
    haut = haut_carre(video, w) if w == h else None
    fl, ft, lx, ly, tx, ty = calques(w, h, logo, sortie.parent, video, haut)
    montee = round(0.015 * h)
    netete = ",unsharp=5:5:0.6:5:5:0.0" if max(w, h) >= 3000 else ""
    graphe = (
        f"[0:v]fps={IPS},tpad=stop_mode=clone:stop_duration=2,trim=0:{duree:.3f},setpts=PTS-STARTPTS,"
        f"{cadrage(w, h, haut)}{netete},setsar=1[v];"
        f"[1:v]format=rgba[logo];"
        f"[2:v]format=rgba,fade=t=in:st=0.25:d=0.55:alpha=1[titre];"
        f"[v][logo]overlay={lx}:{ly}[a];"
        f"[a][titre]overlay=x={tx}:y='{ty}+{montee}*max(0,1-max(0,t-0.25)/0.55)':eval=frame[b];"
        f"[b]fade=t=out:st={duree - FONDU:.3f}:d={FONDU}:color=white,format=yuv420p[sv];"
        f"[3:a]aresample=48000,loudnorm=I=-16:TP=-1.5:LRA=11,aresample=48000[sa]"
    )
    subprocess.run([
        "ffmpeg", "-v", "error", "-y", "-i", str(video), "-loop", "1", "-i", str(fl), "-loop", "1", "-i", str(ft),
        "-i", str(son), "-filter_complex", graphe, "-map", "[sv]", "-map", "[sa]", "-t", f"{duree:.3f}",
        "-c:v", "libx264", "-profile:v", "high", "-preset", "slow", "-crf", "16" if w * h <= 1920 * 1080 else "18",
        "-r", str(IPS), "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", "-color_range", "tv",
        "-c:a", "aac", "-b:a", "192k", "-ar", "48000", "-movflags", "+faststart", str(sortie)], check=True)
    print("écrit :", sortie)


def main() -> None:
    horiz, vert, son, logo, dossier = map(Path, sys.argv[1:6])
    dossier.mkdir(parents=True, exist_ok=True)
    src = Image.open(logo).convert("RGBA")
    for w, h, source in FORMATS:
        sortie = dossier / f"intro_{w}x{h}.mp4"
        if not sortie.exists():
            composer(horiz if source == "h" else vert, son, src, w, h, sortie)


if __name__ == "__main__":
    main()
