"""EDIVID — voix de studio refaite par Sidon (Université de Tokyo, 2025, MIT) : les caractéristiques phonétiques de la
voix (w2v-BERT 2.0) sont nettoyées du bruit et de la pièce, puis un vocodeur refait le son à 48 kHz.

Usage : python restaure_sidon.py <entree> <sortie.wav> [<fichier_bilan.json>]

Comme VoiceFixer, il recrée le son ; contrairement à lui, il part d'une représentation de la parole elle-même, ce qui
devrait garder les mots (à vérifier par Whisper). Méthode de la démo officielle (sarulab-speech/sidon_demo_beta) :
entrée normalisée à 0,9, passe-haut 50 Hz, 16 kHz, blocs de 30 s (96 s dans la démo) raccordés par la dernière trame de caractéristiques.
Sortie recalée sur l'entrée (décalage mesuré sur l'enveloppe, corrigé au-delà de 3 ms), 48 kHz stéréo, durée exacte, ramenée au
niveau de l'entrée. Environnement à part (.venv-sidon) ; poids dans le cache de Hugging Face (téléchargés une fois).
Progression : {"etape": "restauration", "pct": ...}.
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


def signaler(pct: float) -> None:
    print(json.dumps({"etape": "restauration", "pct": round(pct, 1)}), flush=True)


def lire(entree: str, sr: int, canaux: int) -> np.ndarray:
    brut = subprocess.run(["ffmpeg", "-v", "error", "-i", entree, "-vn", "-ac", str(canaux), "-ar", str(sr), "-f", "f32le", "-"],
                          capture_output=True, check=True).stdout
    x = np.frombuffer(brut, dtype=np.float32)
    return x.reshape(-1, canaux) if canaux > 1 else x


def decalage(reference: np.ndarray, signal: np.ndarray, max_ms: int = 80) -> int:
    """Décalage (échantillons) de `signal` sur `reference`, mesuré sur l'enveloppe sonore à la milliseconde : la forme
    d'onde d'un son refait n'a plus la phase de l'original (sa corrélation donnait ±13 ms de faux décalage)."""
    h = SR // 1000
    def env(x):
        k = min(len(x), SR * 60) // h
        e = np.log(np.sqrt((x[:k * h].reshape(k, h) ** 2).mean(axis=1)) + 1e-5)
        return e - e.mean()
    a, b = env(reference), env(signal)
    m = min(len(a), len(b))
    a, b = a[:m], b[:m]
    scores = [np.dot(a[max(0, l):m + min(0, l)], b[max(0, -l):m - max(0, l)]) for l in range(-max_ms, max_ms + 1)]
    lag = int(np.argmax(scores)) - max_ms
    return lag * h if abs(lag) > 3 else 0


def main() -> None:
    entree, sortie = sys.argv[1], sys.argv[2]
    fichier_bilan = sys.argv[3] if len(sys.argv) > 3 else None
    import torchaudio
    import transformers
    from huggingface_hub import hf_hub_download
    debut = time.time()
    signaler(0)
    fe = torch.jit.load(hf_hub_download("sarulab-speech/sidon-v0.1", "feature_extractor_cuda.pt", local_files_only=True), map_location="cuda")
    decodeur = torch.jit.load(hf_hub_download("sarulab-speech/sidon-v0.1", "decoder_cuda.pt", local_files_only=True), map_location="cuda")
    pre = transformers.SeamlessM4TFeatureExtractor.from_pretrained("facebook/w2v-bert-2.0", local_files_only=True)
    origine = lire(entree, SR, 2)
    mono = origine.mean(axis=1)
    crete = float(np.abs(mono).max()) or 1.0
    wav = torch.from_numpy(0.9 * mono / crete).view(1, -1)
    wav = torchaudio.functional.highpass_biquad(wav, SR, 50)
    wav16 = torchaudio.functional.resample(wav, SR, 16000)
    wav16 = torch.nn.functional.pad(wav16, (0, 24000))
    morceaux, cache = [], None
    # Blocs de 30 s : ceux de 96 s de la démo montaient à 9,5 Go de carte sur un rush de 11 min.
    blocs = list(wav16.view(-1).split(16000 * 30))
    with torch.inference_mode():
        for i, bloc in enumerate(blocs):
            entrees = pre(torch.nn.functional.pad(bloc, (160, 160)), sampling_rate=16000, return_tensors="pt")
            car = fe(entrees["input_features"].to("cuda"))["last_hidden_state"]
            if cache is not None:
                car = torch.cat([cache, car], dim=1)
            morceaux.append(decodeur(car.transpose(1, 2)).view(-1)[:-960].float().cpu())
            cache = car[:, -1:]
            signaler(5 + 85 * (i + 1) / len(blocs))
    y = torch.cat(morceaux).numpy()
    # Recalage et durée exacte.
    d = decalage(mono, y[: len(mono)])
    if d > 0:
        y = np.concatenate([np.zeros(d, dtype=np.float32), y])
    elif d < 0:
        y = y[-d:]
    y = np.pad(y, (0, max(0, len(mono) - len(y))))[: len(mono)]
    # Niveau de l'entrée (la démo normalisait la crête à 0,9).
    e_in, e_out = np.sqrt(np.mean(mono ** 2)), np.sqrt(np.mean(y ** 2))
    if e_out > 1e-9:
        y = y * (e_in / e_out)
    sf.write(sortie, np.clip(np.stack([y, y], axis=1), -1, 1).astype(np.float32), SR, subtype="PCM_24")
    signaler(100)
    bilan = {"vram_max_mo": round(torch.cuda.max_memory_allocated() / 2**20), "decalage_echantillons": d,
             "duree_s": round(time.time() - debut, 1)}
    if fichier_bilan:
        Path(fichier_bilan).write_text(json.dumps(bilan))
    print(json.dumps(bilan), flush=True)


if __name__ == "__main__":
    main()
