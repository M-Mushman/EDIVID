"""Range une intro, une fin ou un jingle dans la bibliothèque d'habillage d'EDIVID (donnees/habillage/).

Usage : python ranger.py intro|fin <id> <dossier_des_rendus> <nom> [jingle_d_origine]
        python ranger.py jingle <id> <dossier_du_jingle> <nom>

Intro et fin : les fichiers `intro_<l>x<h>.mp4` (ou `fin_…`) du dossier, liés (lien physique, sans copie) dans
`intros/<id>/` ou `fins/<id>/`, une vignette 480 × 270 tirée de la version 1920 × 1080, et `fiche.json` (nom, durée,
formats, jingle d'origine). Jingle : ses WAV et MP3 liés dans `jingles/<id>/`, avec `fiche.json` (nom, durées).
"""
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import soundfile as sf

RACINE = Path(__file__).resolve().parents[2] / "donnees" / "habillage"


def lier(src: Path, dst: Path) -> None:
    if dst.exists():
        dst.unlink()
    try:
        os.link(src, dst)
    except OSError:
        shutil.copy2(src, dst)


def duree(video: Path) -> float:
    s = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(video)],
                       capture_output=True, text=True, check=True).stdout
    return round(float(s), 3)


def ranger_video(genre: str, ident: str, dossier: Path, nom: str, jingle: str) -> None:
    rayon = RACINE / ("intros" if genre == "intro" else "fins") / ident
    rayon.mkdir(parents=True, exist_ok=True)
    fichiers = sorted(dossier.glob(f"{genre}_*x*.mp4"))
    if not fichiers:
        raise SystemExit(f"aucun {genre}_<l>x<h>.mp4 dans {dossier}")
    for f in fichiers:
        lier(f, rayon / f.name)
    reference = next((f for f in fichiers if f.stem.endswith("1920x1080")), fichiers[0])
    d = duree(reference)
    # Intro : l'image où tout est en place (titre monté) ; fin : celle où la signature est tracée.
    instant = 1.6 if genre == "intro" else max(0.0, d - 1.5)
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-ss", f"{instant:.2f}", "-i", str(reference), "-frames:v", "1",
                    "-vf", "scale=480:270:force_original_aspect_ratio=increase,crop=480:270", "-q:v", "3",
                    str(rayon / "vignette.jpg")], check=True)
    fiche = {"nom": nom, "duree": d, "formats": [f.stem.split("_", 1)[1] for f in fichiers], "jingle": jingle}
    (rayon / "fiche.json").write_text(json.dumps(fiche, ensure_ascii=False, indent=1))
    print(f"{genre} {ident} : {len(fichiers)} formats, {d} s")


def ranger_jingle(ident: str, dossier: Path, nom: str) -> None:
    rayon = RACINE / "jingles" / ident
    rayon.mkdir(parents=True, exist_ok=True)
    durees = {}
    for f in sorted(dossier.iterdir()):
        if f.suffix in (".wav", ".mp3"):
            lier(f, rayon / f.name)
            if f.suffix == ".wav":
                durees[f.stem] = round(sf.info(f).duration, 3)
    if not durees:
        raise SystemExit(f"aucun WAV dans {dossier}")
    (rayon / "fiche.json").write_text(json.dumps({"nom": nom, "durees": durees}, ensure_ascii=False, indent=1))
    print(f"jingle {ident} : {', '.join(durees)}")


def main() -> None:
    genre, ident, dossier, nom = sys.argv[1], sys.argv[2], Path(sys.argv[3]), sys.argv[4]
    if genre in ("intro", "fin"):
        ranger_video(genre, ident, dossier, nom, sys.argv[5] if len(sys.argv) > 5 else "")
    elif genre == "jingle":
        ranger_jingle(ident, dossier, nom)
    else:
        raise SystemExit("genre : intro, fin ou jingle")


if __name__ == "__main__":
    main()
