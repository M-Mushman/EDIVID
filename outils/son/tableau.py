"""Tableau du banc du son : notes DNSMOS de chaque candidat, par extrait, en Markdown.

Usage : python tableau.py <dossier du banc> > tableau.md
Le dossier contient extraits/*.wav et candidats/<extrait>__<candidat>.wav ; les notes sont calculées par
capteurs/qualite_son.py (environnement .venv-son) et gardées dans notes.json pour ne pas être refaites.
"""
import json
import subprocess
import sys
from pathlib import Path

ICI = Path(__file__).resolve().parent
RACINE = ICI.parent.parent
PYTHON = RACINE / ".venv-son" / "bin" / "python"


def noter(fichiers: list[Path], notes: dict) -> None:
    a_faire = [f for f in fichiers if str(f) not in notes]
    if not a_faire:
        return
    sortie = subprocess.run([str(PYTHON), str(RACINE / "capteurs" / "qualite_son.py"), *map(str, a_faire)],
                            capture_output=True, text=True, check=True).stdout
    for ligne in sortie.splitlines():
        if ligne.startswith("{"):
            d = json.loads(ligne)
            notes[d["fichier"]] = d


def main() -> None:
    banc = Path(sys.argv[1])
    fichier_notes = banc / "notes.json"
    notes = json.loads(fichier_notes.read_text()) if fichier_notes.exists() else {}
    extraits = sorted(p.stem for p in (banc / "extraits").glob("*.wav"))
    candidats = sorted({p.stem.split("__", 1)[1] for p in (banc / "candidats").glob("*__*.wav")})
    fichiers = [banc / "extraits" / f"{e}.wav" for e in extraits]
    fichiers += [banc / "candidats" / f"{e}__{c}.wav" for e in extraits for c in candidats if (banc / "candidats" / f"{e}__{c}.wav").exists()]
    noter(fichiers, notes)
    fichier_notes.write_text(json.dumps(notes, ensure_ascii=False, indent=1))
    print("| candidat | " + " | ".join(f"{e} SIG/BAK/OVRL" for e in extraits) + " | OVRL moyen | gain |")
    print("|---|" + "---|" * (len(extraits) + 2))
    lignes = []
    for c in ["brut"] + candidats:
        cases, ovrl = [], []
        for e in extraits:
            f = banc / "extraits" / f"{e}.wav" if c == "brut" else banc / "candidats" / f"{e}__{c}.wav"
            n = notes.get(str(f))
            if n:
                cases.append(f"{n['SIG']:.2f} / {n['BAK']:.2f} / {n['OVRL']:.2f}")
                ovrl.append(n["OVRL"])
            else:
                cases.append("—")
        moyen = sum(ovrl) / len(ovrl) if ovrl else None
        lignes.append((c, cases, moyen))
    base = next((m for c, _, m in lignes if c == "brut"), None)
    for c, cases, moyen in sorted(lignes, key=lambda x: -(x[2] or 0)):
        gain = f"{moyen - base:+.2f}" if moyen is not None and base is not None else "—"
        print(f"| {c} | " + " | ".join(cases) + f" | {moyen:.2f} | {gain} |" if moyen is not None else f"| {c} | " + " | ".join(cases) + " | — | — |")


if __name__ == "__main__":
    main()
