"""Images de départ de l'intro animée, pour une série de photos : le décor est prolongé par FLUX.2 klein 4B (Black
Forest Labs, Apache 2.0, remplissage guidé par la photo elle-même), les pixels de la photo restent intacts.

Usage : python etendre_flux.py <photos.json> <dossier_photos> <dossier_sortie> [nom…]

Mise en page (la même que l'assemblage, composer.py) :
- horizontale 16:9 : la photo entière, à droite (de 45 % à 82 % de la largeur) ; à gauche, le fond libre reçoit
  le logo et le titre ;
- verticale 9:16 : la photo entière en bas (62 % de la hauteur), centrée ; au-dessus, le logo et le titre.
Studio : le fond est prolongé par dégradé depuis les bords de la photo (sans couture, comme preparer.py) ; FLUX ne
repeint qu'une bande étroite là où la personne touche un bord (bras, épaule, main coupés par le cadre), pour la
continuer. Remplir tout le fond par FLUX y ajoutait une seconde personne et un fond d'un autre gris (essai du
2026-10-28). Rue : FLUX prolonge tout le décor. Le travail se fait à ~2,3 Mpx, la photo d'origine est recollée
(fondu de 16 px vers l'intérieur), puis l'image est ramenée au format de Wan (1280×704, 704×1280).
Écrit <sortie>/<nom>/depart_{horizontal,vertical}.png.
"""
import json
import sys
import time
from pathlib import Path

import numpy as np
import torch
from PIL import Image, ImageFilter, ImageOps

sys.path.insert(0, str(Path(__file__).resolve().parent))
from preparer import couleur_bord, etendre_cotes, lissage, lisser  # noqa: E402

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


def prolonger(p: np.ndarray, gauche: int, droite: int, haut: int) -> np.ndarray:
    """Fond de studio prolongé par dégradé : sur les côtés ligne par ligne, puis au-dessus depuis la couleur de fond du
    bord haut, qui se fond vers sa moyenne en montant (sans quoi chaque colonne monterait en raie)."""
    rng = np.random.default_rng(11)
    # Dégradé vers le gris clair seulement sur le large fond de gauche du format horizontal.
    out = etendre_cotes(p, gauche, droite, rng, rampe=(0.0, 0.0) if haut else (1.0, 0.0)) if gauche or droite else p
    if haut:
        bord = couleur_bord(out[:6].transpose(1, 0, 2).astype(np.float64))  # (largeur, 3)
        bord = lisser(bord, 120)
        t = lissage(np.linspace(1, 0, haut))[:, None, None]  # 1 tout en haut, 0 au bord de la photo
        dessus = bord[None] * (1 - t) + bord.mean(axis=0)[None, None] * t
        dessus = np.clip(dessus * np.linspace(1.015, 1.0, haut)[:, None, None]
                         + rng.normal(0, 1.0, (haut, out.shape[1], 1)), 0, 255)
        out = np.concatenate([dessus.astype(np.uint8), out], axis=0)
    return raccorder(out, p, gauche, haut)


def raccorder(out: np.ndarray, p: np.ndarray, gauche: int, haut: int) -> np.ndarray:
    """Le dégradé repart exactement de la couleur du bord de la photo partout où ce bord est du fond ou une ombre au
    sol (gris, même teinté), pas la personne ; l'écart s'efface en s'éloignant de la photo (sans quoi une ombre rosée,
    écartée du fond par sa teinte, laisse un trait vertical)."""
    o = out.astype(np.float64)
    hp, wp = p.shape[:2]
    def ecart(bord_photo, bord_ext):
        c = np.median(bord_photo, axis=1)
        sur_fond = ((c.max(axis=1) - c.min(axis=1)) < 22) & (c.mean(axis=1) > 120)
        d = (c - bord_ext) * sur_fond[:, None]
        return lisser(d, 12)
    attenue = lambda n: np.exp(-np.arange(n) / 180.0)[None, :, None]
    y0 = haut
    if gauche:
        d = ecart(p[:, :4].astype(np.float64), o[y0:y0 + hp, gauche - 1])
        o[y0:y0 + hp, :gauche] += d[:, None, :] * attenue(gauche)[:, ::-1]
    droite = o.shape[1] - gauche - wp
    if droite:
        d = ecart(p[:, -4:].astype(np.float64), o[y0:y0 + hp, gauche + wp])
        o[y0:y0 + hp, gauche + wp:] += d[:, None, :] * attenue(droite)
    if haut:
        bande = o[y0:y0 + 4].transpose(1, 0, 2)  # le bord haut de la photo élargie (côtés compris)
        d = ecart(bande, o[y0 - 1])
        o[:y0] += (d[None] * np.exp(-np.arange(haut) / 180.0)[::-1, None, None])
    # Un fond prolongé ligne par ligne recopie en stries les variations du bord : flou croissant avec la distance à
    # la photo (nul au raccord, entier à ~250 px).
    def adoucir(zone: np.ndarray, distance: np.ndarray) -> np.ndarray:
        flou = np.asarray(Image.fromarray(np.clip(zone, 0, 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(35)))
        poids = (1 - np.exp(-distance / 120.0))[..., None]
        return zone * (1 - poids) + flou.astype(np.float64) * poids
    if gauche:
        z = o[y0:y0 + hp, :gauche]
        o[y0:y0 + hp, :gauche] = adoucir(z, np.broadcast_to(np.arange(gauche)[::-1][None], z.shape[:2]).astype(float))
    if droite:
        z = o[y0:y0 + hp, gauche + wp:]
        o[y0:y0 + hp, gauche + wp:] = adoucir(z, np.broadcast_to(np.arange(droite)[None], z.shape[:2]).astype(float))
    if haut:
        z = o[:y0]
        o[:y0] = adoucir(z, np.broadcast_to(np.arange(haut)[::-1][:, None], z.shape[:2]).astype(float))
    return np.clip(o, 0, 255).astype(np.uint8)


def touche(bande: np.ndarray, marge: int) -> np.ndarray:
    """Positions (le long du bord) où la personne touche le bord de la photo, élargies de `marge`."""
    c = np.median(bande, axis=1)
    sujet = ((c.max(axis=1) - c.min(axis=1)) >= 14) | (c.mean(axis=1) <= 150)
    if not sujet.any():
        return sujet
    noyau = np.ones(2 * marge + 1)
    return np.convolve(sujet.astype(float), noyau, mode="same") > 0


def masque_bandes(w: int, h: int, rect: tuple[int, int, int, int], p: np.ndarray, nom_format: str) -> Image.Image | None:
    """Masque FLUX : des bandes étroites hors de la photo, là seulement où la personne touche un bord."""
    x, y, ww, hh = rect
    m = np.zeros((h, w), np.uint8)
    large = round(0.09 * w) if nom_format == "horizontal" else w
    for cote, bande in (("g", p[:, :4]), ("d", p[:, -4:])):
        lignes = touche(bande, 60)
        if not lignes.any():
            continue
        x0, x1 = (max(0, x - large), x + 12) if cote == "g" else (x + ww - 12, min(w, x + ww + large))
        m[y:y + hh][lignes, x0:x1] = 255
    if y:
        colonnes = touche(p[:4].transpose(1, 0, 2), 60)
        if colonnes.any():
            y0 = max(0, y - 110)
            sous = m[y0:y + 12, x:x + ww]
            sous[:, colonnes] = 255
    return Image.fromarray(m) if m.any() else None


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
        print(json.dumps({"repli": f"FLUX.2 indisponible, dégradé seul : {e}"}), flush=True)
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
            rue = fiche["fond"] == "rue"
            if rue:
                # Rue : tout le décor par FLUX, la toile n'est qu'un départ.
                bord = np.asarray(p)[:, [0, -1]].reshape(-1, 3).mean(axis=0)
                toile = Image.new("RGB", (w, h), tuple(int(c) for c in bord))
                toile.paste(p, (x, y))
                masque = Image.new("L", (w, h), 255)
                masque.paste(0, (x + 12, y + (12 if y else 0), x + ww - 12, y + hh))
            else:
                toile = Image.fromarray(prolonger(np.asarray(p), x, w - x - ww, y))
                masque = masque_bandes(w, h, (x, y, ww, hh), np.asarray(p), nom_format)
            remplie = toile
            if pipe is not None and masque is not None:
                try:
                    gen = pipe(prompt=consigne, image=toile, image_reference=[photo], mask_image=masque, width=w,
                               height=h, strength=1.0, num_inference_steps=4, guidance_scale=1.0,
                               generator=torch.Generator("cpu").manual_seed(7)).images[0].resize((w, h))
                    # Le repeint n'entre que dans ses bandes, avec un fondu vers le dégradé.
                    # Rue : passage progressif vers le repeint sur 24 px à l'intérieur de la photo, où FLUX la
                    # reproduit ; un passage franc au bord du masque laissait un trait vertical (paris3, 2026-10-28).
                    fondu = (ImageOps.invert(ImageOps.invert(masque).filter(ImageFilter.MinFilter(49))
                                             .filter(ImageFilter.GaussianBlur(10))) if rue
                             else masque.filter(ImageFilter.GaussianBlur(14)))
                    remplie = Image.composite(gen, toile, fondu)
                except Exception as e:  # noqa: BLE001 — repli : le dégradé seul
                    print(json.dumps({"repli": f"{nom} {nom_format} : {e}"}), flush=True)
            # La photo d'origine, recollée avec un fondu de 16 px vers l'intérieur (la rue l'a déjà, par son fondu).
            alpha = Image.new("L", (w, h), 0)
            alpha.paste(255, (x + 16, y + (16 if y else 0), x + ww - 16, y + hh))
            alpha = alpha.filter(ImageFilter.GaussianBlur(8))
            finale = remplie if rue else Image.composite(toile, remplie, alpha)
            finale.save(dossier / f"etendue_{nom_format}.png")
            finale.resize(taille_wan, Image.LANCZOS).save(dossier / f"depart_{nom_format}.png")
        (dossier / "consigne_decor.txt").write_text(consigne)
        print(json.dumps({"etape": "decor", "photo": nom, "pct": round(100 * (k + 1) / len(noms), 1)}), flush=True)
    print(json.dumps({"duree_s": round(time.time() - debut, 1),
                      "vram_max_mo": round(torch.cuda.max_memory_allocated() / 2**20)}), flush=True)


if __name__ == "__main__":
    main()
