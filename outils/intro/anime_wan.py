"""Anime une image par Wan 2.2 TI2V-5B (Alibaba, Apache 2.0), sur la carte d'azoth.

Usage : python anime_wan.py <image.png> <sortie.mp4> <consigne.txt> [graines=11 (ou 11,12,13)] [images=93] [pas=40]

Plusieurs graines : une vidéo par graine (<sortie>_g<graine>.mp4), le modèle chargé une seule fois.
       python anime_wan.py --lot <travaux.json> [images=93] [pas=40] : plusieurs images, une seule charge du modèle.

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


def charger():
    from diffusers import WanImageToVideoPipeline
    pipe = WanImageToVideoPipeline.from_pretrained("Wan-AI/Wan2.2-TI2V-5B-Diffusers", torch_dtype=torch.bfloat16,
                                                   local_files_only=True)
    # La carte n'a que ~13 Go pour nous (le modèle résident d'azoth garde le reste) : le transformeur (10 Go) passe
    # sur la carte par groupes de blocs, chargés pendant que le groupe précédent calcule (même qualité, un peu plus
    # lent) ; l'encodeur de texte (11 Go) n'y monte que le temps d'encoder une consigne.
    pipe.transformer.enable_group_offload(onload_device=torch.device("cuda"), offload_device=torch.device("cpu"),
                                          offload_type="block_level", num_blocks_per_group=4, use_stream=True)
    pipe.vae.to("cuda")
    pipe.vae.enable_tiling()
    return pipe


def encoder(pipe, consigne: str):
    # Seul sur la carte : le décodeur en descend le temps de l'encodage (11 Go + 1,4 Go dépassaient le bail).
    pipe.vae.to("cpu")
    torch.cuda.empty_cache()
    pipe.text_encoder.to("cuda")
    with torch.inference_mode():
        positif, negatif = pipe.encode_prompt(consigne, NEGATIF, do_classifier_free_guidance=True,
                                              max_sequence_length=512, device=torch.device("cuda"))
    pipe.text_encoder.to("cpu")
    torch.cuda.empty_cache()
    pipe.vae.to("cuda")
    return positif, negatif


def animer(pipe, image: Path, fichiers: dict[int, Path], consigne: str, images: int, pas: int) -> None:
    """Une vidéo par graine ({graine: fichier}) ; une vidéo déjà faite est sautée (reprise après arrêt)."""
    import subprocess
    from diffusers.utils import export_to_video
    a_faire = {g: f for g, f in fichiers.items() if not f.exists()}
    if not a_faire:
        return
    positif, negatif = encoder(pipe, consigne)
    img = Image.open(image).convert("RGB")
    w, h = img.size
    def avancement(p, i, t, kw):
        print(json.dumps({"etape": "animation", "pct": round(100 * (i + 1) / pas, 1)}), flush=True)
        return kw
    for graine, fichier in a_faire.items():
        debut_tirage = time.time()
        video = pipe(image=img, prompt_embeds=positif, negative_prompt_embeds=negatif, width=w, height=h, num_frames=images,
                     num_inference_steps=pas, guidance_scale=5.0, generator=torch.Generator("cpu").manual_seed(graine),
                     callback_on_step_end=avancement).frames[0]
        fichier.parent.mkdir(parents=True, exist_ok=True)
        tmp = fichier.with_suffix(".brut.mp4")
        export_to_video(video, str(tmp), fps=30, quality=10)
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(tmp), "-c:v", "libx264", "-crf", "10", "-preset", "slow",
                        "-pix_fmt", "yuv420p", "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709",
                        str(fichier)], check=True)
        tmp.unlink()
        bilan = {"image": str(image), "graine": graine, "images": images, "pas": pas, "taille": [w, h],
                 "duree_s": round(time.time() - debut_tirage, 1), "vram_max_mo": round(torch.cuda.max_memory_allocated() / 2**20)}
        fichier.with_suffix(".json").write_text(json.dumps(bilan | {"consigne": consigne}, ensure_ascii=False, indent=1))
        print(json.dumps(bilan), flush=True)


def main() -> None:
    debut = time.time()
    if sys.argv[1] == "--lot":
        # Lot : [{"image", "sortie" (sans suffixe de graine), "consigne", "graines": [...]}], le modèle chargé une fois.
        travaux = json.loads(Path(sys.argv[2]).read_text())
        images = int(sys.argv[3]) if len(sys.argv) > 3 else 93
        pas = int(sys.argv[4]) if len(sys.argv) > 4 else 40
        pipe = charger()
        for t in travaux:
            base = Path(t["sortie"])
            animer(pipe, Path(t["image"]), {g: base.with_name(f"{base.stem}_g{g}.mp4") for g in t["graines"]},
                   t["consigne"], images, pas)
    else:
        image, sortie, consigne = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]).read_text().strip()
        graines = [int(g) for g in sys.argv[4].split(",")] if len(sys.argv) > 4 else [11]
        images = int(sys.argv[5]) if len(sys.argv) > 5 else 93
        pas = int(sys.argv[6]) if len(sys.argv) > 6 else 40
        fichiers = {g: sortie if len(graines) == 1 else sortie.with_name(f"{sortie.stem}_g{g}.mp4") for g in graines}
        animer(charger(), image, fichiers, consigne, images, pas)
    print(json.dumps({"total_s": round(time.time() - debut, 1)}), flush=True)


if __name__ == "__main__":
    main()
