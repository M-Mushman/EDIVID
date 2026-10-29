"""Sauvegarde d'un projet EDIVID en zip, et réimport.

Usage : python sauvegarde.py zip <dossier_projet>            (le zip sur la sortie standard, écrit à la volée)
        python sauvegarde.py taille <dossier_projet>         (sur la sortie : ce que le zip contiendra, en octets)
        python sauvegarde.py deballer <fichier.zip> <dossier>  (sur la sortie : le nombre de fichiers et d'octets)

Le zip garde l'essentiel pour reprendre le projet : projet.json, les rushs d'origine, les médias, les fichiers de
travail légers (analyse, transcription, voix traitées : de longs calculs sur la carte), la vidéo finale et les
vignettes. Il laisse ce qui se recalcule sans la carte : la vidéo de travail (mezz*.mkv, des dizaines de Go), la
vidéo du lecteur (proxy.mp4), les aperçus, les exports, les temporaires. Fichiers rangés tels quels (sans
compression : les vidéos ne se compressent pas), en ZIP64 au-delà de 4 Go.
"""
import json
import shutil
import sys
import zipfile
from pathlib import Path

LOURDS_RECALCULABLES = ("mezz", "proxy.mp4")
BLOC = 1 << 22


def a_garder(rel: Path, rendu: str) -> bool:
    tete = rel.parts[0]
    if rel.as_posix() == "projet.json":
        return True
    if tete in ("rushs", "medias"):
        return True
    if tete == "travail":
        return not any(rel.name.startswith(x) for x in LOURDS_RECALCULABLES)
    if tete == "sorties":
        return (len(rel.parts) == 2 and rel.name == rendu) or (len(rel.parts) >= 2 and rel.parts[1] == "vignettes")
    return False


def a_sauver(dossier: Path) -> list[Path]:
    projet = json.loads((dossier / "projet.json").read_text())
    rendu = (projet.get("rendu") or {}).get("fichier", "")
    return sorted(f for f in dossier.rglob("*") if f.is_file() and a_garder(f.relative_to(dossier), rendu))


def ecrire_zip(dossier: Path) -> None:
    fichiers = a_sauver(dossier)
    sortie = sys.stdout.buffer
    with zipfile.ZipFile(sortie, "w", compression=zipfile.ZIP_STORED, allowZip64=True) as z:
        for f in fichiers:
            info = zipfile.ZipInfo.from_file(f, f.relative_to(dossier).as_posix())
            info.compress_type = zipfile.ZIP_STORED
            with open(f, "rb") as src, z.open(info, "w", force_zip64=info.file_size > 0xFFFF0000) as dst:
                shutil.copyfileobj(src, dst, BLOC)


def deballer(archive: Path, dossier: Path) -> None:
    """Déballe dans `dossier` (neuf) ; refuse tout chemin qui sortirait du dossier ou ne vient pas d'une sauvegarde."""
    dossier.mkdir(parents=True, exist_ok=False)
    n = octets = 0
    with zipfile.ZipFile(archive) as z:
        noms = z.namelist()
        if "projet.json" not in noms:
            raise SystemExit("ce zip n'est pas une sauvegarde EDIVID (pas de projet.json)")
        for info in z.infolist():
            rel = Path(info.filename)
            if info.is_dir():
                continue
            if rel.is_absolute() or ".." in rel.parts or not a_garder(rel, rel.name):
                raise SystemExit(f"chemin refusé dans le zip : {info.filename}")
            cible = dossier / rel
            cible.parent.mkdir(parents=True, exist_ok=True)
            with z.open(info) as src, open(cible, "wb") as dst:
                shutil.copyfileobj(src, dst, BLOC)
            n += 1
            octets += info.file_size
    print(json.dumps({"fichiers": n, "octets": octets}))


def main() -> None:
    if sys.argv[1] == "zip":
        ecrire_zip(Path(sys.argv[2]))
    elif sys.argv[1] == "taille":
        fichiers = a_sauver(Path(sys.argv[2]))
        print(json.dumps({"fichiers": len(fichiers), "octets": sum(f.stat().st_size for f in fichiers)}))
    elif sys.argv[1] == "deballer":
        deballer(Path(sys.argv[2]), Path(sys.argv[3]))
    else:
        raise SystemExit("mode : zip, taille ou deballer")


if __name__ == "__main__":
    main()
