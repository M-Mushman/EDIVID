"""EDIVID — retrait de l'écho et du bruit de la voix par IA (débruiteur de Resemble Enhance, licence MIT), sur la
carte graphique.

Usage : python rehausse.py <entree> <sortie.wav> <part>

On n'utilise que l'étage débruiteur du modèle, qui filtre la voix (masque sur le spectre) : il garde les mots tels
qu'ils sont dits, et le son reste calé à l'échantillon. L'étage de reconstruction, génératif, a été écarté : sur une
voix chargée d'écho, il invente des syllabes (« danser sous la pluie » devenait « dans ses sous-appli » ; mesuré le
2026-10-26 avec Whisper sur le rush d'épisode 0, 2 à 3 mots sur 57 conservés, contre 57 sur 57 pour le débruiteur).

Le modèle travaille en mono à 44,1 kHz ; la sortie est un WAV 48 kHz stéréo de la durée exacte de l'entrée. <part>
(0–1) : part de la voix traitée dans le mélange avec la voix d'origine (le masque ne touche pas à la phase : le
mélange ne crée pas d'effet de filtre).

Environnement à part (.venv-rehaussement) : le paquet fige d'anciennes versions ; son code d'entraînement importe
DeepSpeed, qui ne sert pas ici et qu'un module vide remplace.
Poids : modeles/resemble-enhance/enhancer_stage2 (dépôt Hugging Face ResembleAI/resemble-enhance).
Progression sur la sortie standard, une ligne JSON : {"etape": "rehaussement", "pct": ...}.
"""
import functools
import json
import subprocess
import sys
import types
from pathlib import Path

import numpy as np
import soundfile as sf
import torch

ICI = Path(__file__).resolve().parent
MODELE = ICI.parent / "modeles" / "resemble-enhance" / "enhancer_stage2"
SR_MODELE = 44100
SR = 48000
# Blocs de deux minutes (le modèle les découpe lui-même en morceaux de 30 s raccordés), fondus sur deux secondes.
BLOC = 120 * SR_MODELE
RECOUVREMENT = 2 * SR_MODELE


def signaler(pct: float) -> None:
    print(json.dumps({"etape": "rehaussement", "pct": round(pct, 1)}), flush=True)


def bouchon_deepspeed() -> None:
    """DeepSpeed n'est importé que par l'entraînement : des modules vides suffisent à l'inférence."""
    class Vide:
        def __init__(self, *a, **k):
            pass

    noms = ["deepspeed", "deepspeed.accelerator", "deepspeed.runtime", "deepspeed.runtime.engine", "deepspeed.runtime.utils"]
    mods = {n: types.ModuleType(n) for n in noms}
    mods["deepspeed"].DeepSpeedConfig = Vide
    mods["deepspeed"].init_distributed = lambda *a, **k: None
    mods["deepspeed.accelerator"].get_accelerator = lambda: None
    mods["deepspeed.runtime.engine"].DeepSpeedEngine = Vide
    mods["deepspeed.runtime.utils"].clip_grad_norm_ = lambda *a, **k: None
    mods["deepspeed"].accelerator = mods["deepspeed.accelerator"]
    mods["deepspeed"].runtime = mods["deepspeed.runtime"]
    mods["deepspeed.runtime"].engine = mods["deepspeed.runtime.engine"]
    mods["deepspeed.runtime"].utils = mods["deepspeed.runtime.utils"]
    sys.modules.update(mods)


def lire(entree: str, sr: int, canaux: int) -> np.ndarray:
    brut = subprocess.run(["ffmpeg", "-v", "error", "-i", entree, "-vn", "-ac", str(canaux), "-ar", str(sr), "-f", "f32le", "-"],
                          capture_output=True, check=True).stdout
    return np.frombuffer(brut, dtype=np.float32)


def main() -> None:
    entree, sortie = sys.argv[1], sys.argv[2]
    part = float(sys.argv[3]) if len(sys.argv) > 3 else 1.0
    bouchon_deepspeed()
    # Les poids viennent du dépôt de Resemble AI : on les charge en entier (torch 2.6+ ne lit plus par défaut que
    # des tenseurs, et le fichier d'origine contient aussi l'état de l'entraînement).
    torch.load = functools.partial(torch.load, weights_only=False)
    from resemble_enhance.enhancer.inference import load_enhancer
    from resemble_enhance.inference import inference
    from torchaudio.functional import resample

    if not torch.cuda.is_available():
        raise SystemExit("carte graphique indisponible")
    # Seul l'étage débruiteur va sur la carte.
    debruiteur = load_enhancer(MODELE, "cpu").denoiser.to("cuda").eval()
    x = torch.from_numpy(lire(entree, SR_MODELE, 1).copy())
    y = torch.zeros(len(x))
    poids = torch.zeros(len(x))
    debut = 0
    signaler(0)
    with torch.inference_mode():
        while debut < len(x):
            fin = min(len(x), debut + BLOC + RECOUVREMENT)
            morceau, sr = inference(model=debruiteur, dwav=x[debut:fin], sr=SR_MODELE, device="cuda")
            morceau = morceau.float().cpu()
            if sr != SR_MODELE:
                morceau = resample(morceau, sr, SR_MODELE)
            morceau = torch.nn.functional.pad(morceau, (0, max(0, fin - debut - len(morceau))))[: fin - debut]
            w = torch.ones(fin - debut)
            r = min(RECOUVREMENT, fin - debut)
            if debut > 0:
                w[:r] = torch.linspace(0, 1, r)
            if fin < len(x):
                w[-r:] = torch.linspace(1, 0, r)
            y[debut:fin] += morceau * w
            poids[debut:fin] += w
            signaler(100 * fin / len(x))
            if fin == len(x):
                break
            debut = fin - RECOUVREMENT
    y = y / poids.clamp(min=1e-8)
    origine = lire(entree, SR, 2).reshape(-1, 2)
    y48 = resample(y.unsqueeze(0), SR_MODELE, SR).squeeze(0)
    y48 = torch.nn.functional.pad(y48, (0, max(0, len(origine) - len(y48))))[: len(origine)].numpy()
    traite = np.stack([y48, y48], axis=1)
    melange = part * traite + (1 - part) * origine
    sf.write(sortie, np.clip(melange, -1, 1).astype(np.float32), SR, subtype="PCM_16")
    print(json.dumps({"vram_max_mo": round(torch.cuda.max_memory_allocated() / 2**20)}), flush=True)


if __name__ == "__main__":
    main()
