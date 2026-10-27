"""EDIVID — bruit de fond retiré par DeepFilterNet3 (MIT / Apache 2.0), sur le processeur.

Usage : python debruite.py <entree> <sortie.wav> [<part>] [<attenuation_max_dB>]

Masque spectral et filtrage profond (non génératif : aucun mot inventé), 48 kHz. <part> (0–1, 1 par défaut) : part
du son traité dans le mélange avec l'original. <attenuation_max_dB> (par défaut 100 : pas de limite) : plafond
d'atténuation du bruit, pour garder un peu d'air de la pièce. La sortie est 48 kHz stéréo, recalée et de la durée
exacte de l'entrée. Environnement à part (.venv-deepfilter : torch 2.8 pour le processeur, le paquet exige l'ancienne
interface de torchaudio) ; le modèle est téléchargé au premier usage dans le cache de DeepFilterNet.
Progression sur la sortie standard, une ligne JSON : {"etape": "debruitage", "pct": ...}.
"""
import json
import subprocess
import sys

import numpy as np
import soundfile as sf
import torch

SR = 48000


def signaler(pct: float) -> None:
    print(json.dumps({"etape": "debruitage", "pct": round(pct, 1)}), flush=True)


def lire(entree: str, canaux: int) -> np.ndarray:
    brut = subprocess.run(["ffmpeg", "-v", "error", "-i", entree, "-vn", "-ac", str(canaux), "-ar", str(SR), "-f", "f32le", "-"],
                          capture_output=True, check=True).stdout
    x = np.frombuffer(brut, dtype=np.float32)
    return x.reshape(-1, canaux) if canaux > 1 else x


def main() -> None:
    entree, sortie = sys.argv[1], sys.argv[2]
    part = float(sys.argv[3]) if len(sys.argv) > 3 and sys.argv[3] else 1.0
    atten = float(sys.argv[4]) if len(sys.argv) > 4 else 100.0
    from df import enhance, init_df
    signaler(0)
    modele, etat, _ = init_df(log_level="ERROR")
    origine = lire(entree, 2)
    mono = torch.from_numpy(origine.mean(axis=1).copy()).unsqueeze(0)
    signaler(10)
    # Par blocs de deux minutes (mémoire), avec recouvrement d'une seconde et fondu.
    bloc, rec = 120 * SR, SR
    sortie_mono = torch.zeros(mono.shape[1])
    poids = torch.zeros(mono.shape[1])
    debut = 0
    while debut < mono.shape[1]:
        fin = min(mono.shape[1], debut + bloc + rec)
        y = enhance(modele, etat, mono[:, debut:fin], atten_lim_db=atten if atten < 100 else None).squeeze(0)
        y = torch.nn.functional.pad(y, (0, max(0, fin - debut - len(y))))[: fin - debut]
        w = torch.ones(fin - debut)
        r = min(rec, fin - debut)
        if debut > 0:
            w[:r] = torch.linspace(0, 1, r)
        if fin < mono.shape[1]:
            w[-r:] = torch.linspace(1, 0, r)
        sortie_mono[debut:fin] += y * w
        poids[debut:fin] += w
        signaler(10 + 85 * fin / mono.shape[1])
        if fin == mono.shape[1]:
            break
        debut = fin - rec
    y = (sortie_mono / poids.clamp(min=1e-8)).numpy()
    traite = np.stack([y, y], axis=1)
    melange = part * traite + (1 - part) * origine
    sf.write(sortie, np.clip(melange, -1, 1).astype(np.float32), SR, subtype="PCM_24")
    signaler(100)


if __name__ == "__main__":
    main()
