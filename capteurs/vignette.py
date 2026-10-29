"""Vignette YouTube : Edina dessinée dans un style de la bibliothèque, dans une scène tirée de la vidéo, avec le titre.

Usage : python vignette.py dessiner <travail.json>   (carte graphique : FLUX.2 klein 4B)
        python vignette.py titrer <travail.json>     (processeur seul : repose le texte sur les images déjà dessinées)

travail.json : {"photo": chemin, "styles": [chemins], "scene": texte anglais, "lieu": texte anglais, "texte": titre de la
                vignette, "logo": chemin ou "", "sortie": dossier, "graines": [entiers],
                "methode": "references" | "retouche" | "mixte"}
Écrit, pour chaque graine g : <sortie>/brute_<g>.png (l'image seule) et <sortie>/vignette_<g>.jpg (1280 × 720, titre
posé, moins de 2 Mo). Les styles tournent d'une graine à l'autre.

Deux méthodes :
- « references » : FLUX.2 klein reçoit la photo (image 1, ressemblance) et l'image de style (image 2), et dessine la
  scène entière ;
- « retouche » : FLUX.2 klein redessine la photo elle-même dans le style, en gardant la pose et le visage ; seul le
  décor change (le lieu de la scène) ;
- « mixte » : une proposition sur deux de chaque méthode. La retouche ressemble davantage (tenue, bijoux, coiffure) ; la
  scène entière est plus vivante mais plus générique (essais du 2026-10-29).
Le titre n'est jamais demandé au modèle, qui déforme les lettres : le code le pose du côté opposé au visage (YuNet le
repère aussi sur un dessin), sans jamais le couvrir, en lettres blanches cernées de sombre, lisibles en petit.
"""
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

L, H = 1280, 720
POLICE = Path(__file__).resolve().parents[1] / "modeles" / "polices" / "Anton-Regular.ttf"
YUNET = Path(__file__).resolve().parents[1] / "modeles" / "yunet" / "face_detection_yunet_2023mar.onnx"
# Le style décrit en toutes lettres : l'image de style seule ne suffit pas à FLUX.2 klein (essai du 2026-10-29).
STYLE = ("Japanese anime illustration like a modern anime film still: clean thin line art, soft cel shading, luminous "
         "pastel colours, glossy highlights on skin and hair, big expressive eyes")
AGE = "a radiant woman in her fifties, with the mature features of her age"
CONSIGNE = (
    "{style}, in exactly the same drawing style as image 2. The woman from image 1 is {age}: keep her face "
    "recognisable, same face shape, eyes, nose, smile, hair colour and hairstyle as image 1. {scene} She smiles warmly "
    "at the viewer. Composition for a 16:9 YouTube thumbnail: she is large in the frame, on the {cote} third; the "
    "{autre} part of the picture is calm open background with few details. No text, no letters, no logo, no watermark."
)
RETOUCHE = (
    "Redraw this photo as a {style}. Keep the same woman, {age}, with the same face, smile, hair, pose and clothes. "
    "Replace the background with {lieu}. No text, no letters, no logo, no watermark."
)


def progres(pct: float) -> None:
    print(json.dumps({"etape": "vignette", "pct": round(pct, 3)}), flush=True)


def dessiner(t: dict) -> None:
    import torch
    from diffusers import Flux2KleinPipeline

    sortie = Path(t["sortie"])
    sortie.mkdir(parents=True, exist_ok=True)
    pipe = Flux2KleinPipeline.from_pretrained("black-forest-labs/FLUX.2-klein-4B", torch_dtype=torch.bfloat16,
                                              local_files_only=True)
    pipe.enable_model_cpu_offload()
    photo = Image.open(t["photo"]).convert("RGB")
    photo.thumbnail((1024, 1024), Image.LANCZOS)
    styles = [Image.open(s).convert("RGB") for s in t["styles"]]
    graines = t["graines"]
    methode = t.get("methode", "references")
    for k, g in enumerate(graines):
        retouche = methode == "retouche" or (methode == "mixte" and k % 2 == 0)
        style = styles[k % len(styles)]
        # Edina à droite une fois sur deux : le texte change de côté, le choix est plus large.
        cote, autre = ("right", "left") if k % 2 == 0 else ("left", "right")
        if retouche:
            # La photo garde sa forme ; elle est posée sur un côté d'une toile 16:9 que le modèle complète.
            consigne = RETOUCHE.format(style=STYLE, age=AGE, lieu=t.get("lieu") or "a sunny lakeside landscape")
            refs = [photo]
        else:
            consigne = CONSIGNE.format(style=STYLE, age=AGE, scene=t["scene"].strip(), cote=cote, autre=autre)
            refs = [photo, style]
        img = pipe(prompt=consigne, image=refs, width=L, height=H, num_inference_steps=4, guidance_scale=1.0,
                   generator=torch.Generator("cpu").manual_seed(int(g))).images[0]
        img.resize((L, H), Image.LANCZOS).save(sortie / f"brute_{g}.png")
        progres((k + 1) / len(graines) * 0.95)
    titrer(t)


def lignes(texte: str, f: ImageFont.FreeTypeFont, largeur: int) -> list[str]:
    # Espace insécable devant la ponctuation double et dans les guillemets : jamais un « ! » seul sur sa ligne.
    for signe in "!?:;»":
        texte = texte.replace(f" {signe}", f"\u00a0{signe}")
    texte = texte.replace("« ", "«\u00a0")
    mots, out = texte.split(" "), [""]
    for m in mots:
        essai = f"{out[-1]} {m}".strip()
        if out[-1] and f.getlength(essai) > largeur:
            out.append(m)
        else:
            out[-1] = essai
    return out


def bloc_titre(texte: str, largeur: int, hauteur: int) -> Image.Image:
    """Le titre en lettres blanches cernées, à la plus grande taille qui tienne dans la boîte, lignes alignées à gauche."""
    taille = 130
    while True:
        f = ImageFont.truetype(str(POLICE), taille)
        ls = lignes(texte, f, largeur)
        interligne = round(taille * 1.08)
        if (max(f.getlength(l) for l in ls) <= largeur and interligne * len(ls) <= hauteur) or taille <= 30:
            break
        taille -= 4
    trait = max(3, round(taille * 0.07))
    img = Image.new("RGBA", (largeur + 4 * trait, interligne * len(ls) + 4 * trait), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    for i, l in enumerate(ls):
        d.text((2 * trait, 2 * trait + i * interligne), l, font=f, fill=(255, 255, 255, 255), stroke_width=trait,
               stroke_fill=(20, 24, 48, 255))
    # Ombre portée douce, pour détacher le bloc d'un fond clair.
    ombre = Image.new("RGBA", img.size, (0, 0, 0, 0))
    ombre.putalpha(img.getchannel("A").filter(ImageFilter.GaussianBlur(trait * 2)).point(lambda v: v * 0.55))
    fond = Image.new("RGBA", img.size, (0, 0, 0, 0))
    fond.alpha_composite(ombre, (trait, trait))
    fond.alpha_composite(img)
    return fond


def visage(img: Image.Image) -> tuple[int, int, int, int] | None:
    """Boîte (x, y, l, h) du visage le plus sûr, ou None."""
    import cv2

    bgr = cv2.cvtColor(np.asarray(img.convert("RGB")), cv2.COLOR_RGB2BGR)
    d = cv2.FaceDetectorYN.create(str(YUNET), "", (bgr.shape[1], bgr.shape[0]), 0.5)
    _, f = d.detect(bgr)
    if f is None:
        return None
    x, y, w, h = f[np.argmax(f[:, 14])][:4]
    return int(x), int(y), int(w), int(h)


def place_texte(img: Image.Image, marge: int) -> tuple[str, int]:
    """Côté du texte (gauche ou droite) et largeur disponible : à l'opposé du visage, jusqu'à lui ; sans visage repéré,
    le côté le moins chargé (écart moyen de luminance entre pixels voisins) sur 48 % de la largeur."""
    v = visage(img)
    if v is None:
        g = np.asarray(img.convert("L").resize((128, 72)), dtype=np.float32)
        detail = np.abs(np.diff(g, axis=1)).mean(axis=0)
        return ("gauche" if detail[:63].mean() <= detail[63:].mean() else "droite"), round(L * 0.48)
    x, _, w, _ = v
    # Les cheveux débordent de la boîte du visage.
    ecart = round(0.6 * w)
    if x + w / 2 >= L / 2:
        return "gauche", max(round(L * 0.3), min(round(L * 0.55), x - ecart - marge))
    return "droite", max(round(L * 0.3), min(round(L * 0.55), L - (x + w) - ecart - marge))


def titrer(t: dict) -> None:
    sortie = Path(t["sortie"])
    logo = Image.open(t["logo"]).convert("RGBA") if t.get("logo") else None
    for g in t["graines"]:
        brute = sortie / f"brute_{g}.png"
        if not brute.exists():
            continue
        img = Image.open(brute).convert("RGBA")
        marge = 40
        cote, boite_l = place_texte(img, marge)
        boite_h = round(H * 0.62)
        titre = bloc_titre(t["texte"], boite_l, boite_h)
        x = marge if cote == "gauche" else L - marge - titre.width
        y = round(H * 0.08)
        img.alpha_composite(titre, (max(0, x), y))
        if logo is not None:
            lw = round(L * 0.2)
            lg = logo.resize((lw, round(lw * logo.height / logo.width)), Image.LANCZOS)
            # Voile blanc doux sous le logo : le magenta reste lisible sur tout fond.
            voile = Image.new("RGBA", (lg.width + 40, lg.height + 30), (255, 255, 255, 0))
            a = Image.new("L", voile.size, 0)
            ImageDraw.Draw(a).rounded_rectangle((8, 8, voile.width - 8, voile.height - 8), radius=18, fill=215)
            voile.putalpha(a.filter(ImageFilter.GaussianBlur(6)))
            voile.alpha_composite(lg, (20, 15))
            lx = marge if cote == "gauche" else L - marge - voile.width
            img.alpha_composite(voile, (lx, H - voile.height - 24))
        rgb = img.convert("RGB")
        for q in (92, 85, 75):
            f = sortie / f"vignette_{g}.jpg"
            rgb.save(f, quality=q, optimize=True)
            if f.stat().st_size < 1_900_000:
                break
    progres(1.0)


def main() -> None:
    mode, travail = sys.argv[1], json.loads(Path(sys.argv[2]).read_text())
    if mode == "dessiner":
        dessiner(travail)
    elif mode == "titrer":
        titrer(travail)
    else:
        raise SystemExit("mode : dessiner ou titrer")


if __name__ == "__main__":
    main()
