"""Assemble l'intro : vidéo animée, logo, titre, jingle, fondu au blanc ; un fichier par format.

Usage : python composer.py <video_horizontale.mp4> <video_verticale.mp4> <jingle.wav> <logo.png> <dossier_sortie>
                           [debut_jingle_s=0]

Formats : 3840×2160 et 1920×1080 (depuis la vidéo horizontale), 1080×1920 (depuis la verticale). MP4 H.264 High, SDR
BT.709, 30 i/s, 3 s, son AAC 48 kHz. Titre en Nimbus Sans Bold (reprise libre d'Helvetica, URW).
- horizontal : logo en haut à gauche, titre à gauche sur le fond, la personne à droite ;
- vertical : logo centré en haut, titre centré dessous, au-dessus de la tête.
Le logo est là dès la première image ; le titre apparaît en montant légèrement (0,25 → 0,8 s) ; tout part au blanc
dans la dernière demi-seconde.
"""
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

POLICE = "/usr/share/fonts/opentype/urw-base35/NimbusSans-Bold.otf"
LIGNES = ["Être heureux", "et en forme", "après 50 ans,", "c’est possible !"]
ENCRE = (30, 30, 36, 255)
DUREE, IPS, FONDU = 3.0, 30, 0.5


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
        d.text((x, marge + i * interligne), l, font=f, fill=ENCRE)
    return img


def calques(w: int, h: int, logo_src: Image.Image, dossier: Path) -> tuple[Path, Path, int, int, int, int]:
    """Logo et titre à la taille du format ; renvoie leurs fichiers et positions."""
    vertical = h > w
    if vertical:
        lw = round(0.46 * w)
        t = titre(round(0.86 * w), round(0.085 * w), centre=True)
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
    fl, ft = dossier / f"logo_{w}x{h}.png", dossier / f"titre_{w}x{h}.png"
    logo.save(fl)
    t.save(ft)
    return fl, ft, lx, ly, tx, ty


def composer(video: Path, jingle: Path, logo: Image.Image, w: int, h: int, sortie: Path, debut_jingle: float) -> None:
    fl, ft, lx, ly, tx, ty = calques(w, h, logo, sortie.parent)
    montee = round(0.015 * h)
    netete = ",unsharp=5:5:0.6:5:5:0.0" if w >= 3000 else ""
    graphe = (
        f"[0:v]fps={IPS},trim=0:{DUREE},setpts=PTS-STARTPTS,"
        f"scale={w}:{h}:force_original_aspect_ratio=increase:flags=lanczos,crop={w}:{h}{netete},setsar=1[v];"
        f"[1:v]format=rgba[logo];"
        f"[2:v]format=rgba,fade=t=in:st=0.25:d=0.55:alpha=1[titre];"
        f"[v][logo]overlay={lx}:{ly}[a];"
        f"[a][titre]overlay=x={tx}:y='{ty}+{montee}*max(0,1-max(0,t-0.25)/0.55)':eval=frame[b];"
        f"[b]fade=t=out:st={DUREE - FONDU}:d={FONDU}:color=white,format=yuv420p[sv];"
        f"[3:a]atrim={debut_jingle}:{debut_jingle + DUREE},asetpts=PTS-STARTPTS,aresample=48000,"
        f"afade=t=in:d=0.03,afade=t=out:st={DUREE - 0.6}:d=0.6,loudnorm=I=-16:TP=-1.5:LRA=11,aresample=48000[sa]"
    )
    subprocess.run([
        "ffmpeg", "-v", "error", "-y", "-i", str(video), "-loop", "1", "-i", str(fl), "-loop", "1", "-i", str(ft),
        "-i", str(jingle), "-filter_complex", graphe, "-map", "[sv]", "-map", "[sa]", "-t", str(DUREE),
        "-c:v", "libx264", "-profile:v", "high", "-preset", "slow", "-crf", "16" if w * h <= 1920 * 1080 else "18",
        "-r", str(IPS), "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", "-color_range", "tv",
        "-c:a", "aac", "-b:a", "192k", "-ar", "48000", "-movflags", "+faststart", str(sortie)], check=True)
    print("écrit :", sortie)


def main() -> None:
    horiz, vert, jingle, logo, dossier = map(Path, sys.argv[1:6])
    debut_jingle = float(sys.argv[6]) if len(sys.argv) > 6 else 0.0
    dossier.mkdir(parents=True, exist_ok=True)
    src = Image.open(logo).convert("RGBA")
    for video, (w, h) in [(horiz, (3840, 2160)), (horiz, (1920, 1080)), (vert, (1080, 1920))]:
        composer(video, jingle, src, w, h, dossier / f"intro_{w}x{h}.mp4", debut_jingle)


if __name__ == "__main__":
    main()
