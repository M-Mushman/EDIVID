"""Anime une image par Wan 2.2 TI2V-5B (Alibaba, Apache 2.0), sur la carte d'azoth.

Usage : python anime_wan.py <image.png> <sortie.mp4> <consigne.txt> [graines=11 (ou 11,12,13)] [images=93] [pas=40]

Plusieurs graines : une vidéo par graine (<sortie>_g<graine>.mp4), le modèle chargé une seule fois.

L'image donne le format (1280×704 ou 704×1280, les deux formats natifs du modèle). Le modèle est fait pour 24 i/s ;
on garde ses images telles quelles et la sortie est écrite à 30 i/s (93 images ≈ 3,1 s) : le mouvement, lent par
consigne, en est à peine accéléré. Mémoire de la carte : l'encodeur de texte d'abord seul, puis
le transformeur par groupes de blocs, et le décodeur par tuiles. Sortie : H.264 quasi sans perte (crf 10), à reprendre au montage.
"""
import json
import sys
import time
from pathlib import Path

import numpy as np
import torch
from PIL import Image

NEGATIF = ("static image, frozen, distorted face, deformed eyes, cross-eyed, extra fingers, deformed hands, morphing, "
           "flicker, blurry, low quality, jpeg artifacts, text, watermark, subtitles, overexposed, cartoon, painting, "
           "turning around, walking, waving, gestures, winking, closing eyes, looking away, fast motion, camera shake")


def main() -> None:
    image, sortie, consigne = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]).read_text().strip()
    graines = [int(g) for g in sys.argv[4].split(",")] if len(sys.argv) > 4 else [11]
    images = int(sys.argv[5]) if len(sys.argv) > 5 else 93
    pas = int(sys.argv[6]) if len(sys.argv) > 6 else 40
    from diffusers import WanImageToVideoPipeline
    from diffusers.utils import export_to_video
    debut = time.time()
    pipe = WanImageToVideoPipeline.from_pretrained("Wan-AI/Wan2.2-TI2V-5B-Diffusers", torch_dtype=torch.bfloat16,
                                                   local_files_only=True)
    # La carte n'a que ~13 Go pour nous (le modèle résident d'azoth garde le reste) : l'encodeur de texte (11 Go)
    # travaille seul, puis quitte la carte avant le transformeur et le décodeur.
    pipe.text_encoder.to("cuda")
    with torch.inference_mode():
        positif, negatif = pipe.encode_prompt(consigne, NEGATIF, do_classifier_free_guidance=True,
                                              max_sequence_length=512, device=torch.device("cuda"))
    pipe.text_encoder.to("cpu")
    torch.cuda.empty_cache()
    # Le transformeur (10 Go) et ses calculs ne tiennent pas ensemble : il passe sur la carte par groupes de blocs,
    # chargés pendant que le groupe précédent calcule (même qualité, un peu plus lent).
    pipe.transformer.enable_group_offload(onload_device=torch.device("cuda"), offload_device=torch.device("cpu"),
                                          offload_type="block_level", num_blocks_per_group=4, use_stream=True)
    pipe.vae.to("cuda")
    pipe.vae.enable_tiling()
    img = Image.open(image).convert("RGB")
    w, h = img.size
    def avancement(p, i, t, kw):
        print(json.dumps({"etape": "animation", "pct": round(100 * (i + 1) / pas, 1)}), flush=True)
        return kw
    import subprocess
    for graine in graines:
        debut_tirage = time.time()
        video = pipe(image=img, prompt_embeds=positif, negative_prompt_embeds=negatif, width=w, height=h, num_frames=images,
                     num_inference_steps=pas, guidance_scale=5.0, generator=torch.Generator("cpu").manual_seed(graine),
                     callback_on_step_end=avancement).frames[0]
        fichier = sortie if len(graines) == 1 else sortie.with_name(f"{sortie.stem}_g{graine}.mp4")
        fichier.parent.mkdir(parents=True, exist_ok=True)
        tmp = fichier.with_suffix(".brut.mp4")
        export_to_video(video, str(tmp), fps=30, quality=10)
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(tmp), "-c:v", "libx264", "-crf", "10", "-preset", "slow",
                        "-pix_fmt", "yuv420p", "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709",
                        str(fichier)], check=True)
        tmp.unlink()
        bilan = {"graine": graine, "images": images, "pas": pas, "taille": [w, h],
                 "duree_s": round(time.time() - debut_tirage, 1), "vram_max_mo": round(torch.cuda.max_memory_allocated() / 2**20)}
        fichier.with_suffix(".json").write_text(json.dumps(bilan | {"consigne": consigne}, ensure_ascii=False, indent=1))
        print(json.dumps(bilan), flush=True)
    print(json.dumps({"total_s": round(time.time() - debut, 1)}), flush=True)

if __name__ == "__main__":
    main()
