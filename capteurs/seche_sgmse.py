"""EDIVID — essai : voix déréverbérée par SGMSE+ (sp-uhh, MIT), modèle par diffusion entraîné sur EARS-Reverb à 48 kHz.

Usage : python seche_sgmse.py <entree> <sortie.wav> [<pas = 30>] [<fichier_bilan.json>]

Génératif, mais guidé à chaque pas par le spectre de l'entrée (diffusion conditionnelle) : moins enclin à inventer
qu'une reconstruction comme VoiceFixer ; les mots se vérifient tout de même (verifie_voix.py). Par blocs de 6 s
recouverts d'une demi-seconde (mémoire de la carte). Code amont dans outils/sgmse-amont (dépôt sp-uhh/sgmse), poids
dans modeles/sgmse/ears_reverb_48k.ckpt, environnement .venv-sgmse. Progression : {"etape": "sechage", "pct": ...}.
"""
import json
import subprocess
import sys
import time
from pathlib import Path

import numpy as np
import soundfile as sf
import torch

ICI = Path(__file__).resolve().parent
sys.path.insert(0, str(ICI.parent / "outils" / "sgmse-amont"))
SR = 48000
BLOC = 6 * SR
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
    pas = int(sys.argv[3]) if len(sys.argv) > 3 and sys.argv[3] else 30
    fichier_bilan = sys.argv[4] if len(sys.argv) > 4 else None
    # Le module CUDA compilé du code amont (upfirdn2d) exige nvcc, absent d'azoth : on prend sa version en PyTorch pur,
    # que le code amont prévoit (op/__init__.py, ligne commentée) et qui marche aussi sur la carte.
    # Poids du lien officiel des auteurs (README de sp-uhh/sgmse) : ils contiennent aussi l'état de l'entraînement, que
    # torch 2.6+ refuse de lire par défaut.
    import os
    os.environ["TORCH_FORCE_NO_WEIGHTS_ONLY_LOAD"] = "1"
    import torch.utils.cpp_extension as extension
    extension.load = lambda *a, **k: None
    from sgmse.model import ScoreModel
    from sgmse.util.other import pad_spec
    import sgmse.backbones.ncsnpp_utils.up_or_down_sampling as reechantillonnage
    from sgmse.backbones.ncsnpp_utils.op.upfirdn2d_native import upfirdn2d as upfirdn2d_pur
    reechantillonnage.upfirdn2d = upfirdn2d_pur
    debut = time.time()
    signaler(0)
    modele = ScoreModel.load_from_checkpoint(str(ICI.parent / "modeles" / "sgmse" / "ears_reverb_48k.ckpt"), map_location="cuda")
    modele.t_eps = 0.03
    modele.eval()
    origine = lire(entree, 2)
    mono = origine.mean(axis=1)
    y = np.zeros(len(mono))
    poids = np.zeros(len(mono))
    d = 0
    with torch.inference_mode():
        while d < len(mono):
            fin = min(len(mono), d + BLOC + RECOUVREMENT)
            x = torch.from_numpy(mono[d:fin].copy()).unsqueeze(0)
            norme = x.abs().max().clamp(min=1e-6)
            Y = torch.unsqueeze(modele._forward_transform(modele._stft((x / norme).cuda())), 0)
            Y = pad_spec(Y, mode="reflection")
            echantillonneur = modele.get_pc_sampler("reverse_diffusion", "ald", Y, N=pas, corrector_steps=1, snr=0.5)
            s, _ = echantillonneur()
            s = (modele.to_audio(s.squeeze(), fin - d) * norme).cpu().numpy().reshape(-1)
            s = np.pad(s, (0, max(0, fin - d - len(s))))[: fin - d]
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
    sf.write(sortie, np.clip(np.stack([y, y], axis=1), -1, 1), SR, subtype="PCM_24")
    bilan = {"vram_max_mo": round(torch.cuda.max_memory_allocated() / 2**20), "duree_s": round(time.time() - debut, 1), "pas": pas}
    if fichier_bilan:
        Path(fichier_bilan).write_text(json.dumps(bilan))
    print(json.dumps(bilan), flush=True)


if __name__ == "__main__":
    main()
