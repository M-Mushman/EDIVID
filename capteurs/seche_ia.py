"""EDIVID — voix séchée par IA : la pièce (réverbération) et le bruit retirés par MossFormer2_SE_48K (ClearerVoice-
Studio, Apache 2.0), un réseau de masquage : il ne peut pas inventer une syllabe.

Usage : python seche_ia.py <entree> <sortie.wav> [<part>] [<fichier_bilan.json>]

Banc du 2026-10-27 (épisode 0) : RT60 apparent de 360 ms (rush) à 130 ms, autant que VoiceFixer (génératif, écarté),
mots intacts à Whisper ; les modèles de séparation « voix / réverbération » du monde de la musique ne voyaient pas la
réverbération d'une petite pièce. Carte graphique si elle est là (bail pris par le serveur), processeur sinon (environ
la durée du son). <part> (0–1, 1 par défaut) : part de la voix séchée dans le mélange avec l'original. Sortie 48 kHz
stéréo, recalée à l'échantillon (décalage constant mesuré par corrélation), de la durée exacte de l'entrée.
Environnement à part (.venv-clearvoice) ; poids téléchargés au premier usage (cache de clearvoice).
Progression : {"etape": "sechage", "pct": ...} ; bilan {"vram_max_mo", "decalage_echantillons", "duree_s"}.
"""
import json
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import numpy as np
import soundfile as sf

SR = 48000
BLOC = 120 * SR      # deux minutes
RECOUVREMENT = 2 * SR


def signaler(pct: float) -> None:
    print(json.dumps({"etape": "sechage", "pct": round(pct, 1)}), flush=True)


def lire(entree: str, canaux: int) -> np.ndarray:
    brut = subprocess.run(["ffmpeg", "-v", "error", "-i", entree, "-vn", "-ac", str(canaux), "-ar", str(SR), "-f", "f32le", "-"],
                          capture_output=True, check=True).stdout
    x = np.frombuffer(brut, dtype=np.float32)
    return x.reshape(-1, canaux) if canaux > 1 else x


def decalage(reference: np.ndarray, signal: np.ndarray, max_ms: float = 60.0) -> int:
    n = min(len(reference), len(signal), SR * 60)
    a = reference[:n] - reference[:n].mean()
    b = signal[:n] - signal[:n].mean()
    m = int(max_ms / 1000 * SR)
    taille = 1 << (2 * n - 1).bit_length()
    corr = np.fft.irfft(np.fft.rfft(a, taille) * np.conj(np.fft.rfft(b, taille)), taille)
    corr = np.concatenate([corr[-m:], corr[:m + 1]])
    return int(np.argmax(corr)) - m


def main() -> None:
    entree, sortie = sys.argv[1], sys.argv[2]
    part = float(sys.argv[3]) if len(sys.argv) > 3 and sys.argv[3] else 1.0
    fichier_bilan = sys.argv[4] if len(sys.argv) > 4 else None
    import torch
    from clearvoice import ClearVoice
    debut = time.time()
    signaler(0)
    cv = ClearVoice(task="speech_enhancement", model_names=["MossFormer2_SE_48K"])
    origine = lire(entree, 2)
    mono = origine.mean(axis=1)
    y = np.zeros(len(mono), dtype=np.float32)
    poids = np.zeros(len(mono), dtype=np.float32)
    signaler(5)
    with tempfile.TemporaryDirectory() as tmp:
        debut_bloc = 0
        while debut_bloc < len(mono):
            fin = min(len(mono), debut_bloc + BLOC + RECOUVREMENT)
            f = Path(tmp) / "bloc.wav"
            sf.write(f, mono[debut_bloc:fin], SR, subtype="FLOAT")
            s = cv(input_path=str(f), online_write=False)
            s = np.asarray(s, dtype=np.float32).reshape(-1)
            s = np.pad(s, (0, max(0, fin - debut_bloc - len(s))))[: fin - debut_bloc]
            w = np.ones(fin - debut_bloc, dtype=np.float32)
            r = min(RECOUVREMENT, fin - debut_bloc)
            if debut_bloc > 0:
                w[:r] = np.linspace(0, 1, r, dtype=np.float32)
            if fin < len(mono):
                w[-r:] = np.linspace(1, 0, r, dtype=np.float32)
            y[debut_bloc:fin] += s * w
            poids[debut_bloc:fin] += w
            signaler(5 + 90 * fin / len(mono))
            if fin == len(mono):
                break
            debut_bloc = fin - RECOUVREMENT
    y = y / np.maximum(poids, 1e-8)
    d = decalage(mono, y)
    if d > 0:
        y = np.concatenate([np.zeros(d, dtype=np.float32), y])
    elif d < 0:
        y = y[-d:]
    y = np.pad(y, (0, max(0, len(mono) - len(y))))[: len(mono)]
    traite = np.stack([y, y], axis=1)
    melange = part * traite + (1 - part) * origine
    sf.write(sortie, np.clip(melange, -1, 1).astype(np.float32), SR, subtype="PCM_24")
    signaler(100)
    vram = round(torch.cuda.max_memory_allocated() / 2**20) if torch.cuda.is_available() else 0
    bilan = {"vram_max_mo": vram, "decalage_echantillons": d, "duree_s": round(time.time() - debut, 1)}
    if fichier_bilan:
        Path(fichier_bilan).write_text(json.dumps(bilan))
    print(json.dumps(bilan), flush=True)


if __name__ == "__main__":
    main()
