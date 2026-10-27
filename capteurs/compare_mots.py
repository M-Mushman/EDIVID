"""EDIVID — mots d'une voix refaite comparés à ceux du rush : un relevé, sans toucher au son.

Usage : python compare_mots.py <voix.wav> <analyse.json> <debut_s> <bilan.json>

La voix (refaite par Sidon) est transcrite par Whisper large-v3 et comparée mot à mot à la transcription du rush
(analyse.json, décalée de <debut_s>), sans ponctuation, majuscules ni accents. Les écarts sont relevés avec leur instant :
ce sont les passages à vérifier (un écart peut venir de Whisper lui-même, qui hésite sur un rush réverbérant).
Écrit le bilan JSON {methode: "sidon", mots, mots_identiques, accord, ecarts: [[t, rush, voix]]}.
Progression : {"etape": "verification", "pct": ...}.
"""
import json
import re
import subprocess
import sys
import unicodedata
from difflib import SequenceMatcher
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from analyse import precharger_cuda  # noqa: E402


def signaler(pct: float) -> None:
    print(json.dumps({"etape": "verification", "pct": round(pct, 1)}), flush=True)


def norme(m: str) -> str:
    t = unicodedata.normalize("NFD", m.lower())
    return re.sub(r"[^0-9a-z]", "", "".join(c for c in t if not unicodedata.combining(c)))


def main() -> None:
    voix, analyse, debut, fichier_bilan = sys.argv[1], sys.argv[2], float(sys.argv[3]), sys.argv[4]
    signaler(0)
    brut = subprocess.run(["ffmpeg", "-v", "error", "-i", voix, "-ac", "1", "-ar", "16000", "-f", "f32le", "-"], capture_output=True, check=True).stdout
    son = np.frombuffer(brut, np.float32)
    duree = len(son) / 16000
    rush = [(m["t0"] - debut, norme(m["mot"]), m["mot"]) for m in json.loads(Path(analyse).read_text())["mots"]
            if 0 <= m["t0"] - debut < duree and norme(m["mot"])]
    precharger_cuda()
    from faster_whisper import WhisperModel
    modele = WhisperModel("large-v3", device="cuda", compute_type="float16")
    segments, _ = modele.transcribe(son, language="fr", word_timestamps=True, beam_size=5, condition_on_previous_text=False)
    refaite = [(w.start, norme(w.word), w.word.strip()) for s in segments for w in s.words if norme(w.word)]
    signaler(90)
    sm = SequenceMatcher(a=[x[1] for x in rush], b=[x[1] for x in refaite], autojunk=False)
    identiques = sum(i2 - i1 for op, i1, i2, _, _ in sm.get_opcodes() if op == "equal")
    ecarts = []
    for op, i1, i2, j1, j2 in sm.get_opcodes():
        if op == "equal":
            continue
        t = rush[i1][0] if i1 < len(rush) else (refaite[j1][0] if j1 < len(refaite) else 0.0)
        ecarts.append([round(t + debut, 2), " ".join(x[2] for x in rush[i1:i2]), " ".join(x[2] for x in refaite[j1:j2])])
    bilan = {"methode": "sidon", "mots": len(rush), "mots_identiques": identiques,
             "accord": round(identiques / max(1, len(rush)), 4), "ecarts": ecarts}
    Path(fichier_bilan).write_text(json.dumps(bilan, ensure_ascii=False))
    signaler(100)
    print(json.dumps({k: v for k, v in bilan.items() if k != "ecarts"}, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
