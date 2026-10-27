"""Jingle instrumental par ACE-Step 1.5 XL turbo (MIT, musique utilisable commercialement), sur la carte d'azoth.

Usage : python jingle.py <dossier_sortie> <consigne> [nombre=4] [duree_s=8] [bpm=112] [tonalite="G major"]

Écrit <dossier>/jingle_<graine>.wav (48 kHz stéréo), plusieurs graines pour choisir à l'écoute. La durée générée
dépasse celle de l'intro : on y choisit ensuite le meilleur passage, avec un fondu de fin.
"""
import json
import sys
import time
from pathlib import Path

import soundfile as sf
import torch


def main() -> None:
    sortie, consigne = Path(sys.argv[1]), sys.argv[2]
    nombre = int(sys.argv[3]) if len(sys.argv) > 3 else 4
    duree = float(sys.argv[4]) if len(sys.argv) > 4 else 8.0
    bpm = int(sys.argv[5]) if len(sys.argv) > 5 else 112
    tonalite = sys.argv[6] if len(sys.argv) > 6 else "G major"
    from diffusers import AceStepPipeline
    debut = time.time()
    pipe = AceStepPipeline.from_pretrained("ACE-Step/acestep-v15-xl-turbo-diffusers", torch_dtype=torch.bfloat16,
                                           local_files_only=True)
    pipe.enable_model_cpu_offload()
    sortie.mkdir(parents=True, exist_ok=True)
    for k in range(nombre):
        graine = 100 + k
        audio = pipe(prompt=consigne, lyrics="[instrumental]", audio_duration=duree, num_inference_steps=8, bpm=bpm,
                     keyscale=tonalite, timesignature="4",
                     generator=torch.Generator("cpu").manual_seed(graine)).audios
        sf.write(sortie / f"jingle_{graine}.wav", audio[0].float().cpu().numpy().T, 48000, subtype="PCM_24")
        print(json.dumps({"etape": "jingle", "pct": round(100 * (k + 1) / nombre, 1)}), flush=True)
    print(json.dumps({"duree_s": round(time.time() - debut, 1),
                      "vram_max_mo": round(torch.cuda.max_memory_allocated() / 2**20)}), flush=True)


if __name__ == "__main__":
    main()
