"""EDIVID — traîne de la pièce atténuée sans rien inventer : masque spectral qui ne peut qu'atténuer.

Usage : python seche.py <entree> <sortie.wav> <mode> [options…]
  mode « traine » [<RT60 s = 0.45> <force 0–1 = 0.7>]
      Soustraction spectrale de la réverbération tardive (Lebart 2001) : à chaque instant, la part du spectre qui
      prolonge le son d'il y a 60 ms et plus, décroissant selon le RT60, est estimée puis retranchée ; gain plancher
      −14 dB, lissé dans le temps (pas de bruit musical).
  mode « guide » <guide.wav> [<plancher dB = -14> <force 0–1 = 1>]
      Le masque est le rapport |guide| / |entrée| par trame et par bande, borné à 1 (jamais d'ajout) et au plancher :
      le guide est une voix sèche du même passage (par exemple la reconstruction générative), dont on ne prend que
      « où atténuer », jamais le son. Ce qui sort est l'entrée elle-même, atténuée là où la pièce résonne.
Entrée et sortie 48 kHz stéréo, durée exacte conservée, processeur seulement (numpy).
Progression : {"etape": "sechage", "pct": ...}.
"""
import json
import subprocess
import sys

import numpy as np
import soundfile as sf

SR = 48000
N = 1024
PAS = 256


def signaler(pct: float) -> None:
    print(json.dumps({"etape": "sechage", "pct": round(pct, 1)}), flush=True)


def lire(entree: str, canaux: int) -> np.ndarray:
    brut = subprocess.run(["ffmpeg", "-v", "error", "-i", entree, "-vn", "-ac", str(canaux), "-ar", str(SR), "-f", "f32le", "-"],
                          capture_output=True, check=True).stdout
    x = np.frombuffer(brut, dtype=np.float32)
    return x.reshape(-1, canaux) if canaux > 1 else x


def stft(x: np.ndarray) -> np.ndarray:
    fen = np.hanning(N + 1)[:-1].astype(np.float32)
    x = np.pad(x, (N // 2, N // 2))
    nf = (len(x) - N) // PAS + 1
    idx = np.arange(N)[None, :] + np.arange(nf)[:, None] * PAS
    return np.fft.rfft(x[idx] * fen, axis=1)


def istft(X: np.ndarray, longueur: int) -> np.ndarray:
    fen = np.hanning(N + 1)[:-1].astype(np.float32)
    trames = np.fft.irfft(X, n=N, axis=1) * fen
    y = np.zeros(N + (len(trames) - 1) * PAS, dtype=np.float64)
    norme = np.zeros_like(y)
    for i, t in enumerate(trames):
        y[i * PAS:i * PAS + N] += t
        norme[i * PAS:i * PAS + N] += fen ** 2
    y = y / np.maximum(norme, 1e-6)
    return y[N // 2:N // 2 + longueur].astype(np.float32)


def lisser_temps(g: np.ndarray, montee: float = 0.6, descente: float = 0.85) -> np.ndarray:
    """Gain lissé trame à trame : il remonte vite (attaque des consonnes), redescend plus lentement."""
    s = np.empty_like(g)
    s[0] = g[0]
    for t in range(1, len(g)):
        a = np.where(g[t] > s[t - 1], montee, descente)
        s[t] = a * s[t - 1] + (1 - a) * g[t]
    return s


def masque_traine(Y: np.ndarray, rt60: float, force: float) -> np.ndarray:
    P = np.abs(Y) ** 2
    retard = max(1, int(round(0.06 * SR / PAS)))          # début de la réverbération tardive : 60 ms
    delta = 6.9 / max(rt60, 0.1)                           # décroissance de l'énergie : e^(-2·delta·t)
    att = np.exp(-2 * delta * retard * PAS / SR)
    tardive = np.zeros_like(P)
    # Estimation récursive : la réverbération tardive prolonge, en décroissant, l'énergie d'il y a `retard` trames.
    decr = np.exp(-2 * delta * PAS / SR)
    for t in range(retard, len(P)):
        tardive[t] = decr * tardive[t - 1] + (1 - decr) * att * P[t - retard]
    gain = 1 - force * tardive / np.maximum(P, 1e-12)
    return np.clip(gain, 10 ** (-14 / 20), 1.0)


def masque_guide(Y: np.ndarray, G: np.ndarray, plancher_db: float, force: float) -> np.ndarray:
    n = min(len(Y), len(G))
    # Rapport des amplitudes, ramené à 1 sur les trames fortes de chaque bande : le guide n'a ni le niveau ni la
    # couleur de l'entrée (une reconstruction est plus forte et plus brillante) ; seule compte sa variation dans le
    # temps, c'est-à-dire où il s'éteint plus vite que l'entrée. Lissé sur 3 bandes, borné à 1 : jamais d'ajout.
    r = np.abs(G[:n]) / np.maximum(np.abs(Y[:n]), 1e-9)
    niveau = np.abs(Y[:n])
    fortes = niveau >= np.percentile(niveau, 80, axis=0, keepdims=True)
    reference = np.array([np.median(r[fortes[:, k], k]) if fortes[:, k].any() else 1.0 for k in range(r.shape[1])])
    r = r / np.maximum(reference, 1e-9)[None, :]
    noyau = np.array([0.25, 0.5, 0.25])
    r = np.apply_along_axis(lambda v: np.convolve(v, noyau, mode="same"), 1, r)
    gain = np.clip(r, 10 ** (plancher_db / 20), 1.0)
    gain = 1 - force * (1 - gain)
    if len(Y) > n:
        gain = np.vstack([gain, np.ones((len(Y) - n, Y.shape[1]))])
    return gain


def main() -> None:
    entree, sortie, mode = sys.argv[1], sys.argv[2], sys.argv[3]
    origine = lire(entree, 2)
    signaler(0)
    mono = origine.mean(axis=1)
    Y = stft(mono)
    if mode == "traine":
        rt60 = float(sys.argv[4]) if len(sys.argv) > 4 else 0.45
        force = float(sys.argv[5]) if len(sys.argv) > 5 else 0.7
        gain = masque_traine(Y, rt60, force)
    elif mode == "guide":
        guide = lire(sys.argv[4], 1)
        plancher = float(sys.argv[5]) if len(sys.argv) > 5 else -14.0
        force = float(sys.argv[6]) if len(sys.argv) > 6 else 1.0
        gain = masque_guide(Y, stft(guide), plancher, force)
    else:
        raise SystemExit(__doc__)
    signaler(60)
    gain = lisser_temps(gain)
    # Le masque calculé sur le mélange mono s'applique à chaque canal : la stéréo d'origine est gardée.
    canaux = []
    for c in range(origine.shape[1]):
        Yc = stft(origine[:, c])
        canaux.append(istft(Yc * gain[:len(Yc)], len(origine)))
    y = np.stack(canaux, axis=1)
    sf.write(sortie, np.clip(y, -1, 1).astype(np.float32), SR, subtype="PCM_24")
    signaler(100)
    print(json.dumps({"mode": mode, "attenuation_moyenne_db": round(float(20 * np.log10(np.mean(gain) + 1e-9)), 2)}), flush=True)


if __name__ == "__main__":
    main()
