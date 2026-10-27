"""Images de départ de l'intro animée, pour une série de photos : le décor est prolongé par FLUX.2 klein 4B (Black
Forest Labs, Apache 2.0, remplissage guidé par la photo elle-même), les pixels de la photo restent intacts.

Usage : python etendre_flux.py <photos.json> <dossier_photos> <dossier_sortie> [nom…]

Mise en page (la même que l'assemblage, composer.py) :
- horizontale 16:9 : la photo entière, à droite (de 45 % à 82 % de la largeur) ; à gauche, le fond libre reçoit
  le logo et le titre ;
- verticale 9:16 : la photo entière en bas (62 % de la hauteur), centrée ; au-dessus, le logo et le titre.
Le remplissage travaille à ~2,3 Mpx, puis la photo d'origine est recollée (fondu de 16 px vers l'intérieur), et
l'image ramenée au format de Wan (1280×704, 704×1280). Écrit <sortie>/<nom>/depart_{horizontal,vertical}.png.
"""
import json
import sys
import time
from pathlib import Path

import numpy as np
import torch
from PIL import Image, ImageFilter

FORMATS = {
    # nom : (taille de travail, taille Wan, position de la photo)
    "horizontal": ((2048, 1120), (1280, 704)),
    "vertical": ((1120, 2032), (704, 1280)),
}
DECORS = {
    "studio": "The plain light grey seamless paper photo studio backdrop continues smoothly to the edges, soft even "
              "studio lighting, soft natural floor shadow, empty background.",
    "rue": "The quiet old Paris street continues naturally: old stone walls, ivy, an old wooden door, cobblestones, "
           "soft daylight.",
}


def placer(nom_format: str, w: int, h: int, pw: int, ph: int) -> tuple[int, int, int, int]:
    """Rectangle (x, y, largeur, hauteur) de la photo dans l'image de travail."""
    if nom_format == "horizontal":
        hh = h
        ww = round(pw * hh / ph)
        x = round(0.45 * w)
        return x, 0, ww, hh
    hh = round(0.62 * h)
    ww = round(pw * hh / ph)
    return (w - ww) // 2, h - hh, ww, hh


def degrade(toile: Image.Image, rect: tuple[int, int, int, int]) -> Image.Image:
    """Repli sans modèle : le fond hors de la photo est la couleur de son bord, étirée puis très floutée (studio uni)."""
    x, y, ww, hh = rect
    a = np.asarray(toile).astype(np.float64)
    h, w = a.shape[:2]
    out = a.copy()
    photo = a[y:y + hh, x:x + ww]
    gauche, droite, haut = photo[:, :4].mean(axis=1), photo[:, -4:].mean(axis=1), photo[:4].mean(axis=0)
    out[y:y + hh, :x] = gauche[:, None]
    out[y:y + hh, x + ww:] = droite[:, None]
    if y:
        out[:y, x:x + ww] = haut[None]
        out[:y, :x] = haut[0]
        out[:y, x + ww:] = haut[-1]
    flou = np.asarray(Image.fromarray(out.astype(np.uint8)).filter(ImageFilter.GaussianBlur(60))).astype(np.float64)
    return Image.fromarray(np.clip(flou + np.random.default_rng(3).normal(0, 1.0, flou.shape), 0, 255).astype(np.uint8))


def main() -> None:
    fiches = json.loads(Path(sys.argv[1]).read_text())
    photos, sortie = Path(sys.argv[2]), Path(sys.argv[3])
    noms = sys.argv[4:] or list(fiches)
    try:
        from diffusers import Flux2KleinInpaintPipeline
        pipe = Flux2KleinInpaintPipeline.from_pretrained("black-forest-labs/FLUX.2-klein-4B", torch_dtype=torch.bfloat16,
                                                         local_files_only=True)
        pipe.enable_model_cpu_offload()
    except Exception as e:  # noqa: BLE001 — repli : dégradé, la série continue
        print(json.dumps({"repli": f"FLUX.2 indisponible : {e}"}), flush=True)
        pipe = None
    debut = time.time()
    for k, nom in enumerate(noms):
        fiche = fiches[nom]
        dossier = sortie / nom
        if all((dossier / f"depart_{f}.png").exists() for f in FORMATS):
            continue
        dossier.mkdir(parents=True, exist_ok=True)
        photo = Image.open(photos / f"{nom}.jpg").convert("RGB")
        consigne = (f"Professional fashion photograph of a smiling woman with long blond hair wearing {fiche['tenue']}, "
                    f"{fiche['pose']}. {DECORS[fiche['fond']]} Sharp focus, realistic, high detail.")
        for nom_format, ((w, h), taille_wan) in FORMATS.items():
            x, y, ww, hh = placer(nom_format, w, h, *photo.size)
            p = photo.resize((ww, hh), Image.LANCZOS)
            # Toile de départ : la photo sur un gris moyen du bord (le remplissage repart de zéro dans le masque).
            bord = np.asarray(p)[:, [0, -1]].reshape(-1, 3).mean(axis=0)
            toile = Image.new("RGB", (w, h), tuple(int(c) for c in bord))
            toile.paste(p, (x, y))
            # Masque : tout sauf la photo, qu'il chevauche de 12 px pour fondre le raccord.
            masque = Image.new("L", (w, h), 255)
            masque.paste(0, (x + 12, y + (12 if y else 0), x + ww - 12, y + hh))
            # Bords de la photo qui touchent ceux de l'image : rien à remplir de ce côté.
            remplie = None
            if pipe is not None:
                try:
                    remplie = pipe(prompt=consigne, image=toile, image_reference=[photo], mask_image=masque, width=w,
                                   height=h, strength=1.0, num_inference_steps=4, guidance_scale=1.0,
                                   generator=torch.Generator("cpu").manual_seed(7)).images[0].resize((w, h))
                except Exception as e:  # noqa: BLE001
                    print(json.dumps({"repli": f"{nom} {nom_format} : {e}"}), flush=True)
            if remplie is None:
                remplie = degrade(toile, (x, y, ww, hh))
            # La photo d'origine, recollée avec un fondu de 16 px vers l'intérieur.
            alpha = Image.new("L", (w, h), 0)
            alpha.paste(255, (x + 16, y + (16 if y else 0), x + ww - 16, y + hh))
            alpha = alpha.filter(ImageFilter.GaussianBlur(8))
            finale = Image.composite(toile, remplie, alpha)
            finale.save(dossier / f"etendue_{nom_format}.png")
            finale.resize(taille_wan, Image.LANCZOS).save(dossier / f"depart_{nom_format}.png")
        (dossier / "consigne_decor.txt").write_text(consigne)
        print(json.dumps({"etape": "decor", "photo": nom, "pct": round(100 * (k + 1) / len(noms), 1)}), flush=True)
    print(json.dumps({"duree_s": round(time.time() - debut, 1),
                      "vram_max_mo": round(torch.cuda.max_memory_allocated() / 2**20)}), flush=True)


if __name__ == "__main__":
    main()
