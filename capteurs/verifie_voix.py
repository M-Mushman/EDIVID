"""EDIVID — garde-fou de la voix reconstruite : elle doit dire les mêmes mots que le rush.

Usage : python verifie_voix.py <reconstruite.wav> <debruitee.wav> <analyse.json> <sortie.wav> [<debut_s> [<bilan.json>]]

La voix reconstruite (restaure.py) et la voix débruitée sont transcrites avec le modèle et les réglages de l'analyse
du rush (Whisper large-v3, analyse.py), puis comparées mot à mot ; les longues plages qui diffèrent sont réécoutées
à part. Whisper juge l'ensemble : si au moins SEUIL des mots sont identiques, la sortie est toute la voix
reconstruite ; sinon, toute la voix débruitée. Jamais de mélange des deux : raccordées mot à mot (fondus de 30 ms),
elles faisaient changer le timbre toutes les 7 s en moyenne sur épisode 0 (93 bascules en 11 min, 2026-10-26).
<debut_s> : instant du rush où commencent les fichiers (écoute d'essai ; 0 sinon). <analyse.json> n'est plus lu
(gardé pour l'appel). Écrit le WAV de sortie (48 kHz stéréo, durée des entrées) et le bilan en JSON (sortie standard
et <bilan.json> s'il est donné).
"""
import json
import re
import subprocess
import sys
from difflib import SequenceMatcher
from pathlib import Path

import numpy as np
import soundfile as sf

sys.path.insert(0, str(Path(__file__).resolve().parent))
from analyse import precharger_cuda  # noqa: E402

MARGE = 0.12    # s, autour d'un mot changé
FONDU = 0.03    # s
SEUIL = 0.99    # part des mots identiques pour garder la voix reconstruite


def signaler(pct: float) -> None:
    print(json.dumps({"etape": "verification", "pct": round(pct, 1)}), flush=True)


def jetons(mots):
    """(jeton, t0, t1) : mots en minuscules, sans ponctuation, coupés aux apostrophes et traits d'union."""
    res = []
    for m in mots:
        for j in re.findall(r"\w+", m["mot"].lower()):
            res.append((j, m["t0"], m["t1"]))
    return res


def transcrire(modele, son: np.ndarray, decalage: float) -> list:
    segments, _ = modele.transcribe(son, language="fr", word_timestamps=True, beam_size=5)
    return [{"t0": m.start + decalage, "t1": m.end + decalage, "mot": m.word.strip()} for seg in segments for m in seg.words]


def ecarts(a: list, b: list, borne0: float, borne1: float) -> list:
    """Plages (s) où les deux suites de jetons diffèrent, élargies de MARGE."""
    plages = []
    for op, i1, i2, j1, j2 in SequenceMatcher(a=[x[0] for x in a], b=[x[0] for x in b], autojunk=False).get_opcodes():
        if op == "equal":
            continue
        bornes = [x for x in (a[i1:i2] + b[j1:j2])]
        if bornes:
            t0, t1 = min(x[1] for x in bornes), max(x[2] for x in bornes)
        else:
            t0 = a[i1 - 1][2] if i1 > 0 else borne0
            t1 = a[i1][1] if i1 < len(a) else borne1
        plages.append([max(borne0, t0 - MARGE), min(borne1, t1 + MARGE)])
    return plages


def fusionner(plages: list) -> list:
    res = []
    for p in sorted(plages):
        if res and p[0] <= res[-1][1] + 2 * FONDU:
            res[-1][1] = max(res[-1][1], p[1])
        else:
            res.append(list(p))
    return res


def main() -> None:
    reconstruite, debruitee, _analyse, sortie = sys.argv[1:5]
    debut = float(sys.argv[5]) if len(sys.argv) > 5 else 0.0
    fichier_bilan = sys.argv[6] if len(sys.argv) > 6 else None
    r, sr = sf.read(reconstruite, dtype="float32", always_2d=True)
    d, _ = sf.read(debruitee, dtype="float32", always_2d=True)
    n = min(len(r), len(d))
    r, d = r[:n], d[:n]
    duree = n / sr
    signaler(0)
    son16 = lambda f: np.frombuffer(subprocess.run(["ffmpeg", "-v", "error", "-i", f, "-ac", "1", "-ar", "16000", "-f", "f32le", "-"],
                                                   capture_output=True, check=True).stdout, np.float32)
    sr_, sd_ = son16(reconstruite), son16(debruitee)
    precharger_cuda()
    from faster_whisper import WhisperModel
    modele = WhisperModel("large-v3", device="cuda", compute_type="float16")
    # On compare la voix reconstruite à la voix débruitée, transcrites dans les mêmes conditions (deux voix propres) :
    # contre la transcription du rush, faite à travers l'écho, Whisper signalait des écarts de lui-même.
    tr = transcrire(modele, sr_, debut)
    signaler(40)
    td = transcrire(modele, sd_, debut)
    signaler(80)
    plages = fusionner(ecarts(jetons(td), jetons(tr), debut, debut + duree))
    # Seconde écoute des longues plages : Whisper saute parfois tout un passage d'un long fichier ; isolé, il le
    # transcrit. On ne garde que ce qui diffère encore.
    affinees = []
    for t0, t1 in plages:
        if t1 - t0 <= 3.0:
            affinees.append([t0, t1])
            continue
        a0, a1 = max(debut, t0 - 1.0), min(debut + duree, t1 + 1.0)
        i0, i1 = int((a0 - debut) * 16000), int((a1 - debut) * 16000)
        pr = jetons(transcrire(modele, sr_[i0:i1], a0))
        pd = jetons(transcrire(modele, sd_[i0:i1], a0))
        affinees += [p for p in ecarts(pd, pr, a0, a1) if p[1] > t0 and p[0] < t1]
    fusion = fusionner(affinees)
    a = jetons(td)
    b = jetons(tr)
    identiques = sum(i2 - i1 for op, i1, i2, _, _ in SequenceMatcher(a=[x[0] for x in a], b=[x[0] for x in b], autojunk=False).get_opcodes() if op == "equal")
    # Mots de la voix débruitée qui tombent dans une plage où les deux voix diffèrent encore après la réécoute.
    changes = sum(1 for _, t0, t1 in a if any(p0 <= (t0 + t1) / 2 <= p1 for p0, p1 in fusion))
    accord = 1.0 - changes / max(1, len(a))
    reconstruite_gardee = accord >= SEUIL
    sf.write(sortie, np.clip(r if reconstruite_gardee else d, -1, 1), sr, subtype="PCM_16")
    signaler(100)
    bilan = {"voix": "reconstruite" if reconstruite_gardee else "debruitee", "accord": round(accord, 4), "seuil": SEUIL,
             "mots": len(a), "mots_changes": changes, "mots_identiques_premiere_ecoute": identiques,
             "plages_differentes": [[round(x, 2), round(y, 2)] for x, y in fusion],
             "secondes_differentes": round(sum(y - x for x, y in fusion), 1)}
    if fichier_bilan:
        Path(fichier_bilan).write_text(json.dumps(bilan, ensure_ascii=False))
    print(json.dumps(bilan, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
