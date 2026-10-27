"""EDIVID — retrait de la réverbération par séparation (voix sèche / réverbération), sur la carte graphique.

Usage : python separe.py <entree> <sortie.wav> <modele> [<part>] [<fichier_bilan.json>]

Modèles de séparation de la famille UVR, par le paquet audio-separator : ce sont des masques spectraux, qui
répartissent ce qui est déjà dans le son entre « voix sèche » et « réverbération ». Ils ne peuvent pas inventer
une syllabe, là où une reconstruction générative (VoiceFixer) le fait. Modèles éprouvés sur épisode 0 (banc du
2026-10-27, essais/2026-10-27-son-studio) :
  - dereverb_mel_band_roformer_mono_anvuew_sdr_20.4029.ckpt  (voix sèches mono, parlées et chantées ; GPL-3.0)
  - dereverb_mel_band_roformer_less_aggressive_anvuew_sdr_18.8050.ckpt
  - UVR-DeEcho-DeReverb.pth, MDX23C-De-Reverb-aufr33-jarredou.ckpt, Reverb_HQ_By_FoxJoy.onnx
Le modèle est téléchargé au premier usage dans modeles/separation/.

<part> (0–1, 1 par défaut) : part de la voix sèche dans le mélange avec le son d'origine (le masque ne touche pas à
la phase : le mélange ne crée pas d'effet de filtre). La sortie est recalée à l'échantillon sur l'entrée (le décalage
constant d'un modèle est mesuré par corrélation et compensé), en 48 kHz stéréo, de la durée exacte de l'entrée.
Progression sur la sortie standard, une ligne JSON : {"etape": "separation", "pct": ...} ; bilan final
{"vram_max_mo", "decalage_echantillons", "modele"}.
"""
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
import soundfile as sf
import torch

ICI = Path(__file__).resolve().parent
MODELES = ICI.parent / "modeles" / "separation"
SR = 48000
# Le stem gardé selon le modèle : la voix sans sa réverbération.
STEM_SEC = {"No Reverb", "Vocals", "dry", "Dry", "No Echo", "noreverb"}


def signaler(pct: float) -> None:
    print(json.dumps({"etape": "separation", "pct": round(pct, 1)}), flush=True)


def lire(entree: str, sr: int, canaux: int) -> np.ndarray:
    brut = subprocess.run(["ffmpeg", "-v", "error", "-i", entree, "-vn", "-ac", str(canaux), "-ar", str(sr), "-f", "f32le", "-"],
                          capture_output=True, check=True).stdout
    return np.frombuffer(brut, dtype=np.float32).reshape(-1, canaux) if canaux > 1 else np.frombuffer(brut, dtype=np.float32)


def decalage(reference: np.ndarray, signal: np.ndarray, sr: int, max_ms: float = 60.0) -> int:
    """Décalage (échantillons) de `signal` par rapport à `reference`, par corrélation sur une minute au plus."""
    n = min(len(reference), len(signal), sr * 60)
    a = reference[:n] - reference[:n].mean()
    b = signal[:n] - signal[:n].mean()
    m = int(max_ms / 1000 * sr)
    taille = 1 << (2 * n - 1).bit_length()
    fa = np.fft.rfft(a, taille)
    fb = np.fft.rfft(b, taille)
    corr = np.fft.irfft(fa * np.conj(fb), taille)
    corr = np.concatenate([corr[-m:], corr[:m + 1]])
    return int(np.argmax(corr)) - m


def main() -> None:
    entree, sortie, modele = sys.argv[1], sys.argv[2], sys.argv[3]
    part = float(sys.argv[4]) if len(sys.argv) > 4 and sys.argv[4] else 1.0
    fichier_bilan = sys.argv[5] if len(sys.argv) > 5 else None
    if not torch.cuda.is_available():
        raise SystemExit("carte graphique indisponible")
    from audio_separator.separator import Separator

    signaler(0)
    origine = lire(entree, SR, 2)
    with tempfile.TemporaryDirectory() as tmp:
        # Entrée mono 44,1 kHz (les modèles ont appris sur des voix mono) ; sortie WAV dans le dossier provisoire.
        mono = Path(tmp) / "entree.wav"
        sf.write(mono, lire(entree, 44100, 1), 44100, subtype="FLOAT")
        separateur = Separator(output_dir=tmp, output_format="WAV", sample_rate=44100, model_file_dir=str(MODELES),
                               log_level=30, mdxc_params={"segment_size": 256, "overlap": 4, "batch_size": 1},
                               vr_params={"batch_size": 1, "window_size": 512, "aggression": 5, "enable_tta": False})
        separateur.load_model(model_filename=modele)
        signaler(10)
        fichiers = separateur.separate(str(mono))
        signaler(85)
        # Le stem « sec » : celui dont le nom ne parle pas de réverbération, sinon le premier.
        chemins = [Path(tmp) / f if not Path(f).is_absolute() else Path(f) for f in fichiers]
        sec = next((c for c in chemins if any(f"({s})" in c.name for s in STEM_SEC)), None)
        if sec is None:
            sec = next((c for c in chemins if "reverb" not in c.name.lower() and "echo" not in c.name.lower()), chemins[0])
        voix, sr = sf.read(sec, dtype="float32", always_2d=True)
        voix = voix.mean(axis=1)
    if sr != SR:
        from torchaudio.functional import resample
        voix = resample(torch.from_numpy(voix.copy()).unsqueeze(0), sr, SR).squeeze(0).numpy()
    # Recalage sur l'entrée.
    ref = origine.mean(axis=1)
    d = decalage(ref, voix, SR)
    if d > 0:
        voix = np.concatenate([np.zeros(d, dtype=np.float32), voix])
    elif d < 0:
        voix = voix[-d:]
    voix = np.pad(voix, (0, max(0, len(origine) - len(voix))))[: len(origine)]
    traite = np.stack([voix, voix], axis=1)
    melange = part * traite + (1 - part) * origine
    sf.write(sortie, np.clip(melange, -1, 1).astype(np.float32), SR, subtype="PCM_24")
    signaler(100)
    bilan = {"vram_max_mo": round(torch.cuda.max_memory_allocated() / 2**20), "decalage_echantillons": d, "modele": modele,
             "stem": sec.name}
    if fichier_bilan:
        Path(fichier_bilan).write_text(json.dumps(bilan))
    print(json.dumps(bilan), flush=True)


if __name__ == "__main__":
    main()
