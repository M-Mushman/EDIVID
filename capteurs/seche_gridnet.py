"""EDIVID — voix séchée par TF-GridNet (modèle de référence du concours URGENT 2025, CC BY 4.0), discriminatif : il
estime la voix sèche et propre à partir du son (bruit, réverbération, bande passante), sans rien générer.

Usage : python seche_gridnet.py <entree> <sortie.wav> [<part>] [<fichier_bilan.json>]

Essai du 2026-10-27 : MossFormer2 laissait une traîne entre 2 et 4 kHz (RT60 apparent 240 ms contre 130 ailleurs),
que l'oreille entendait. Le modèle accepte toutes les fréquences d'échantillonnage du concours (8 à 48 kHz) ; on le
fait travailler à 48 kHz, par blocs de 4 s recouverts d'une demi-seconde (mémoire de la carte). Sortie 48 kHz stéréo, recalée,
de la durée exacte de l'entrée. Environnement à part (.venv-gridnet : espnet de PyPI) ; poids téléchargés au premier
usage (Hugging Face kohei0209/tfgridnet_urgent25). Progression : {"etape": "sechage", "pct": ...}.
"""
import json
import subprocess
import sys
import time
from pathlib import Path

import numpy as np
import soundfile as sf
import torch

SR = 48000
BLOC = 4 * SR  # TF-GridNet : 16 Go de carte pour 30 s à 48 kHz ; 4 s tiennent dans le bail
RECOUVREMENT = SR // 2


def signaler(pct: float) -> None:
    print(json.dumps({"etape": "sechage", "pct": round(pct, 1)}), flush=True)


def lire(entree: str, canaux: int) -> np.ndarray:
    brut = subprocess.run(["ffmpeg", "-v", "error", "-i", entree, "-vn", "-ac", str(canaux), "-ar", str(SR), "-f", "f32le", "-"],
                          capture_output=True, check=True).stdout
    x = np.frombuffer(brut, dtype=np.float32)
    return x.reshape(-1, canaux) if canaux > 1 else x


def main() -> None:
    entree, sortie = sys.argv[1], sys.argv[2]
    part = float(sys.argv[3]) if len(sys.argv) > 3 and sys.argv[3] else 1.0
    fichier_bilan = sys.argv[4] if len(sys.argv) > 4 else None
    from espnet2.bin.enh_inference import SeparateSpeech
    debut = time.time()
    signaler(0)
    # Poids dans le cache de Hugging Face (téléchargés une fois : huggingface_hub.snapshot_download) ; chargés par
    # leurs fichiers, le serveur lançant les capteurs hors ligne.
    from huggingface_hub import snapshot_download
    racine = Path(snapshot_download("kohei0209/tfgridnet_urgent25", local_files_only=True)) / "exp" / "enh_train_enh_tfgridnet_dm_raw"
    modele = SeparateSpeech(train_config=str(racine / "config.yaml"), model_file=str(racine / "valid.loss.ave_5best.pth"),
                            normalize_output_wav=False, device="cuda" if torch.cuda.is_available() else "cpu")
    origine = lire(entree, 2)
    mono = origine.mean(axis=1).astype(np.float64)
    y = np.zeros(len(mono))
    poids = np.zeros(len(mono))
    d = 0
    with torch.inference_mode():
        while d < len(mono):
            fin = min(len(mono), d + BLOC + RECOUVREMENT)
            s = np.asarray(modele(mono[None, d:fin], fs=SR)[0]).reshape(-1)
            s = np.pad(s, (0, max(0, fin - d - len(s))))[: fin - d]
            # Le modèle rend sa sortie à un autre niveau : ramenée à celui de l'entrée, bloc par bloc.
            e_in, e_out = np.sqrt(np.mean(mono[d:fin] ** 2)), np.sqrt(np.mean(s ** 2))
            if e_out > 1e-9:
                s *= e_in / e_out
            w = np.ones(fin - d)
            r = min(RECOUVREMENT, fin - d)
            if d > 0:
                w[:r] = np.linspace(0, 1, r)
            if fin < len(mono):
                w[-r:] = np.linspace(1, 0, r)
            y[d:fin] += s * w
            poids[d:fin] += w
            signaler(100 * fin / len(mono))
            if fin == len(mono):
                break
            d = fin - RECOUVREMENT
    y = (y / np.maximum(poids, 1e-8)).astype(np.float32)
    traite = np.stack([y, y], axis=1)
    melange = part * traite + (1 - part) * origine
    sf.write(sortie, np.clip(melange, -1, 1).astype(np.float32), SR, subtype="PCM_24")
    signaler(100)
    vram = round(torch.cuda.max_memory_allocated() / 2**20) if torch.cuda.is_available() else 0
    bilan = {"vram_max_mo": vram, "duree_s": round(time.time() - debut, 1)}
    if fichier_bilan:
        Path(fichier_bilan).write_text(json.dumps(bilan))
    print(json.dumps(bilan), flush=True)


if __name__ == "__main__":
    main()
