"""EDIVID — timbre de la voix reconstruit par IA (VoiceFixer, licence MIT, mode 0), sur la carte graphique.

Usage : python restaure.py <voix_debruitee.wav> <sortie.wav>

L'entrée est la voix déjà débruitée (rehausse.py) : partir d'une voix propre limite ce que le modèle invente (sur
épisode 0, 56 mots sur 58 conservés, contre 51 sur 58 à partir du rush brut). VoiceFixer resynthétise la voix : il
peut encore changer un mot ; verifie_voix.py le repère et garde, à ces endroits, la voix débruitée.

Le modèle rend une voix mono à 44,1 kHz, en avance constante de 15 à 16 ms sur l'entrée (mesuré minute par minute
sur le rush d'épisode 0, par le rythme des syllabes, corrélation 0,8) : la sortie est retardée de 750 échantillons à
48 kHz, ramenée en stéréo et à la durée exacte de l'entrée.
Environnement à part (.venv-voicefixer) ; poids dans ~/.cache/voicefixer (téléchargés au premier usage).
Progression sur la sortie standard, une ligne JSON : {"etape": "restauration", "pct": ...}.
"""
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
import soundfile as sf
import torch

SR = 48000
AVANCE = 750  # échantillons à 48 kHz (15,6 ms)


def signaler(pct: float) -> None:
    print(json.dumps({"etape": "restauration", "pct": round(pct, 1)}), flush=True)


def main() -> None:
    entree, sortie = sys.argv[1], sys.argv[2]
    if not torch.cuda.is_available():
        raise SystemExit("carte graphique indisponible")
    from voicefixer import VoiceFixer
    signaler(0)
    with tempfile.TemporaryDirectory() as tmp:
        brut = Path(tmp) / "brut.wav"
        VoiceFixer().restore(input=entree, output=str(brut), cuda=True, mode=0)
        signaler(90)
        donnees = subprocess.run(["ffmpeg", "-v", "error", "-i", str(brut), "-ac", "1", "-ar", str(SR), "-f", "f32le", "-"],
                                 capture_output=True, check=True).stdout
    y = np.frombuffer(donnees, dtype=np.float32)
    longueur = sf.info(entree).frames
    y = np.concatenate([np.zeros(AVANCE, dtype=np.float32), y])
    y = np.pad(y, (0, max(0, longueur - len(y))))[:longueur]
    sf.write(sortie, np.clip(np.stack([y, y], axis=1), -1, 1), SR, subtype="PCM_16")
    signaler(100)
    print(json.dumps({"vram_max_mo": round(torch.cuda.max_memory_allocated() / 2**20)}), flush=True)


if __name__ == "__main__":
    main()
