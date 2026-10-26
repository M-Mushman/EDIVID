"""EDIVID — suppression de l'écho de la pièce (déréverbération WPE, sur le processeur).

Usage : python echo.py <entree> <sortie.wav> <part>

<entree> : vidéo ou son (le premier flux audio est lu par ffmpeg, à 48 kHz, en gardant ses canaux).
<part> : part du son traité dans le mélange (0–1) ; le reste est le son d'origine, pour une suppression douce.
La sortie est un WAV 48 kHz de même durée et calé à l'échantillon sur l'entrée : le rendu y prend la voix
à la place de celle du rush.

Méthode : WPE (Weighted Prediction Error, Nakatani et al. 2010 ; bibliothèque nara_wpe) dans le domaine
fréquentiel. Elle prédit la traîne de la pièce à partir des trames passées et la retranche ; avec deux micros
(iPhone), elle s'appuie sur les deux canaux. Traitement par blocs de 30 s qui se chevauchent : la mémoire reste
bornée sur un rush long.
Progression sur la sortie standard, une ligne JSON : {"etape": "echo", "pct": ...}.
"""
import json
import subprocess
import sys

import numpy as np
import soundfile as sf
from nara_wpe.wpe import wpe_v8

SR = 48000
NFFT = 2048
PAS = 512
TAPS = 20
DELAI = 2
ITERATIONS = 5
BLOC = 30 * SR
RECOUVREMENT = 2 * SR


def signaler(pct: float) -> None:
    print(json.dumps({"etape": "echo", "pct": round(pct, 1)}), flush=True)


def lire(entree: str) -> np.ndarray:
    """Son de l'entrée, (échantillons, canaux), en float32."""
    canaux = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "a:0", "-show_entries", "stream=channels", "-of", "csv=p=0", entree],
        capture_output=True, text=True, check=True).stdout.strip()
    c = max(1, min(2, int(canaux or 1)))
    brut = subprocess.run(["ffmpeg", "-v", "error", "-i", entree, "-vn", "-ac", str(c), "-ar", str(SR), "-f", "f32le", "-"],
                          capture_output=True, check=True).stdout
    return np.frombuffer(brut, dtype=np.float32).reshape(-1, c)


FENETRE = np.hanning(NFFT + 1)[:-1].astype(np.float32)


def stft(x: np.ndarray) -> np.ndarray:
    """x (échantillons,) → (trames, fréquences) ; complété pour que la reconstruction couvre tout x."""
    x = np.pad(x, (NFFT - PAS, NFFT))
    n = 1 + (len(x) - NFFT) // PAS
    trames = np.lib.stride_tricks.as_strided(x, (n, NFFT), (x.strides[0] * PAS, x.strides[0]))
    return np.fft.rfft(trames * FENETRE, axis=1)


def istft(X: np.ndarray, longueur: int) -> np.ndarray:
    trames = np.fft.irfft(X, n=NFFT, axis=1) * FENETRE
    total = (len(trames) - 1) * PAS + NFFT
    y = np.zeros(total, dtype=np.float64)
    norme = np.zeros(total, dtype=np.float64)
    for i, t in enumerate(trames):
        y[i * PAS:i * PAS + NFFT] += t
        norme[i * PAS:i * PAS + NFFT] += FENETRE ** 2
    y /= np.maximum(norme, 1e-8)
    return y[NFFT - PAS:NFFT - PAS + longueur]


def traiter(bloc: np.ndarray) -> np.ndarray:
    """bloc (échantillons, canaux) → même forme, sans la traîne de la pièce."""
    Y = np.stack([stft(bloc[:, c]) for c in range(bloc.shape[1])])  # (canaux, trames, fréquences)
    Z = wpe_v8(Y.transpose(2, 0, 1), taps=TAPS, delay=DELAI, iterations=ITERATIONS, statistics_mode="full")
    Z = Z.transpose(1, 2, 0)
    return np.stack([istft(Z[c], len(bloc)) for c in range(bloc.shape[1])], axis=1)


def main() -> None:
    entree, sortie, part = sys.argv[1], sys.argv[2], float(sys.argv[3])
    x = lire(entree)
    y = np.zeros_like(x, dtype=np.float64)
    poids = np.zeros(len(x), dtype=np.float64)
    debut = 0
    while debut < len(x):
        fin = min(len(x), debut + BLOC + RECOUVREMENT)
        traite = traiter(x[debut:fin])
        # Fondu triangulaire sur les recouvrements : les blocs se raccordent sans clic.
        w = np.ones(fin - debut)
        r = min(RECOUVREMENT, fin - debut)
        if debut > 0:
            w[:r] = np.linspace(0, 1, r)
        if fin < len(x):
            w[-r:] = np.linspace(1, 0, r)
        y[debut:fin] += traite * w[:, None]
        poids[debut:fin] += w
        signaler(100 * fin / len(x))
        if fin == len(x):
            break
        debut = fin - RECOUVREMENT
    y /= np.maximum(poids, 1e-8)[:, None]
    melange = part * y + (1 - part) * x
    sf.write(sortie, np.clip(melange, -1, 1).astype(np.float32), SR, subtype="PCM_16")


if __name__ == "__main__":
    main()
