"""Images de départ de l'intro animée, au format de génération de Wan 2.2 TI2V-5B (1280×704 et 704×1280).

Usage : python preparer.py <photo> <dossier_sortie>

La photo est verticale (2:3) sur un fond de studio presque uni : on prolonge ce fond plutôt que de couper la personne.
- horizontale : de la tête à la taille, la personne à droite, fond prolongé surtout à gauche (logo et titre) ;
- verticale : de la tête aux genoux, un tiers de fond libre au-dessus de la tête (logo et titre).
Le fond ajouté reprend, ligne par ligne, la couleur moyenne du bord de la photo (dégradé du studio), lissée, avec un
léger grain pour ne pas trancher sur le grain de la photo.
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image


def lisser(v: np.ndarray, rayon: int) -> np.ndarray:
    """Moyenne glissante le long de l'axe 0, bords répétés."""
    noyau = np.ones(2 * rayon + 1) / (2 * rayon + 1)
    p = np.pad(v, ((rayon, rayon), (0, 0)), mode="edge")
    return np.stack([np.convolve(p[:, c], noyau, mode="valid") for c in range(v.shape[1])], axis=1)


def couleur_bord(colonnes: np.ndarray) -> np.ndarray:
    """Couleur de fond, ligne par ligne, d'une bande de bord : la médiane de la bande ; les lignes où passe un vêtement
    ou une mèche (couleur saturée ou sombre) reprennent celle des lignes de fond voisines."""
    c = np.median(colonnes, axis=1)
    sat = c.max(axis=1) - c.min(axis=1)
    fond = (sat < 22) & (c.mean(axis=1) > 150)
    idx = np.arange(len(c))
    if fond.any():
        c = np.stack([np.interp(idx, idx[fond], c[fond, k]) for k in range(3)], axis=1)
    return lisser(c, 25)


def lissage(t: np.ndarray) -> np.ndarray:
    return t * t * (3 - 2 * t)


def etendre_cotes(img: np.ndarray, gauche: int, droite: int, rng) -> np.ndarray:
    h = img.shape[0]
    # Le fond ajouté part exactement de la couleur du bord (aucune couture), puis rejoint peu à peu le gris le plus
    # clair du studio (le bord droit de la photo est assombri par l'éclairage).
    bord_g, bord_d = couleur_bord(img[:, :6].astype(np.float64)), couleur_bord(img[:, -6:].astype(np.float64))
    loin = np.maximum(bord_g, bord_d)
    tg = lissage(np.linspace(1, 0, gauche))[None, :, None]
    td = lissage(np.linspace(0, 1, droite))[None, :, None]
    g = bord_g[:, None, :] * (1 - tg) + loin[:, None, :] * tg
    d = bord_d[:, None, :] * (1 - td) + loin[:, None, :] * td
    g += rng.normal(0, 1.0, (h, gauche, 1))
    d += rng.normal(0, 1.0, (h, droite, 1))
    out = np.concatenate([g, img.astype(np.float64), d], axis=1)
    return np.clip(out, 0, 255).astype(np.uint8)


def etendre_haut(img: np.ndarray, haut: int, rng) -> np.ndarray:
    w = img.shape[1]
    bord = img[2:24].mean(axis=0)  # (w, 3)
    bord = lisser(bord, 60)
    pente = np.linspace(1.015, 1.0, haut)[:, None, None]
    t = np.clip(bord[None, :, :] * pente, 0, 255)
    out = np.concatenate([t, img.astype(np.float64)], axis=0)
    for y in range(60):
        a = y / 60
        out[haut + y] = (1 - a) * bord + a * out[haut + y]
    out[:haut] += rng.normal(0, 1.2, (haut, w, 1))
    return np.clip(out, 0, 255).astype(np.uint8)


def main() -> None:
    photo, sortie = Path(sys.argv[1]), Path(sys.argv[2])
    sortie.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(11)
    img = np.asarray(Image.open(photo).convert("RGB"))
    h, w = img.shape[:2]
    # Horizontale : de la tête (marge au-dessus) à la taille ; la personne à droite, le fond libre à gauche reçoit
    # le logo et le titre.
    y0, y1 = int(0.05 * h), int(0.78 * h)
    morceau = img[y0:y1]
    largeur = round((y1 - y0) * 1280 / 704)
    ajout = largeur - w
    gauche = round(ajout * 0.72)
    horiz = etendre_cotes(morceau, gauche, ajout - gauche, rng)
    Image.fromarray(horiz).resize((1280, 704), Image.LANCZOS).save(sortie / "depart_horizontal.png")
    # Verticale : de la tête aux genoux (le bas de la robe est coupé), un tiers de fond libre au-dessus pour le logo
    # et le titre, un peu de fond ajouté sur les côtés pour le format 9:16.
    morceau = img[: int(0.88 * h)]
    hauteur = round(morceau.shape[0] / 0.69)
    largeur = round(hauteur * 704 / 1280)
    ajout = max(0, largeur - w)
    vert = etendre_cotes(morceau, ajout // 2, ajout - ajout // 2, rng) if ajout else morceau
    vert = etendre_haut(vert, hauteur - vert.shape[0], rng)
    Image.fromarray(vert).resize((704, 1280), Image.LANCZOS).save(sortie / "depart_vertical.png")
    print("images de départ :", sortie)


if __name__ == "__main__":
    main()
