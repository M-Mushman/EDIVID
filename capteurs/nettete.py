"""EDIVID — netteté IA : agrandit ×4 la zone recadrée d'un plan, et restaure le visage en option.

Usage : python nettete.py <travaux.json>

<travaux.json> : liste de {"entree", "debut", "duree", "crop": [x, y, w, h], "sortie", "visage": bool,
"largeur_max", "melange"}. "melange" (0–1, 1 par défaut) : part de l'image reconstruite, le reste venant de
l'image d'origine agrandie, pour garder le grain naturel de la peau. L'image agrandie est ramenée à "largeur_max" si elle la dépasse : inutile d'écrire plus de
pixels que le plan n'en montrera.
Chaque travail produit <sortie> : la zone recadrée, agrandie ×4 (Real-ESRGAN realesr-general-x4v3),
visage restauré par GFPGAN 1.4 si "visage" est vrai. Sans son : le rendu reprend le son du rush.
Les modèles sont chargés une seule fois pour toute la liste.
Progression sur la sortie standard, une ligne JSON : {"etape": "nettete", "pct": ...}.
"""
import json
import subprocess
import sys
from pathlib import Path

import cv2
import numpy as np
import torch
import torch.nn.functional as F
from spandrel import ModelLoader

ICI = Path(__file__).resolve().parent
MODELES = ICI.parent / "modeles"
IPS = 30
LOT = 8
# Pixels d'entrée par lot : la mémoire de la carte croît avec eux (~600 octets par pixel d'entrée au pic, mesuré
# le 2026-10-26). 3 Mpx tiennent dans le bail de 3 Go (carte.rs, VRAM_NETTETE_MO) ; en 4K, une zone de
# 2 216 × 1 246 px passe ainsi seule, là où 8 images à la fois demandaient plus de 9 Go.
PIXELS_PAR_LOT = 3_000_000
ECHELLE = 4
MELANGE_VISAGE = 0.75  # part du visage restauré : en dessous de 1, moins de scintillement d'une image à l'autre
# Gabarit FFHQ des cinq points (œil gauche à l'écran, œil droit, nez, coins de la bouche) pour 512 × 512.
GABARIT = np.array([[192.98138, 239.94708], [318.90277, 240.1936], [256.63416, 314.01935],
                    [201.26117, 371.41043], [313.08905, 371.15118]], dtype=np.float32)


def signaler(pct: float) -> None:
    print(json.dumps({"etape": "nettete", "pct": round(pct, 1)}), flush=True)


def charger(nom: str):
    modele = ModelLoader().load_from_file(str(MODELES / nom))
    modele.cuda().eval()
    if modele.supports_half:
        modele.half()
    return modele


def en_tenseur(images: np.ndarray, demi: bool) -> torch.Tensor:
    t = torch.from_numpy(images).cuda().permute(0, 3, 1, 2).contiguous()
    return (t.half() if demi else t.float()) / 255.0


def en_images(t: torch.Tensor) -> np.ndarray:
    return (t.clamp(0, 1) * 255.0).round().byte().permute(0, 2, 3, 1).cpu().numpy()


class Restaurateur:
    def __init__(self) -> None:
        self.modele = charger("GFPGANv1.4.pth")
        self.detecteur = cv2.FaceDetectorYN.create(str(MODELES / "yunet.onnx"), "", (320, 320), 0.6)
        masque = np.zeros((512, 512), np.float32)
        cv2.ellipse(masque, (256, 290), (190, 230), 0, 0, 360, 1.0, -1)
        self.masque = cv2.GaussianBlur(masque, (0, 0), 25)

    def __call__(self, image: np.ndarray) -> np.ndarray:
        h, w = image.shape[:2]
        self.detecteur.setInputSize((w, h))
        _, faces = self.detecteur.detect(cv2.cvtColor(image, cv2.COLOR_RGB2BGR))
        if faces is None:
            return image
        fc = max(faces, key=lambda v: v[2] * v[3])
        if fc[2] < 48:
            return image
        points = fc[4:14].reshape(5, 2).astype(np.float32)
        m, _ = cv2.estimateAffinePartial2D(points, GABARIT, method=cv2.LMEDS)
        if m is None:
            return image
        visage = cv2.warpAffine(image, m, (512, 512), flags=cv2.INTER_LINEAR, borderMode=cv2.BORDER_REFLECT)
        with torch.inference_mode():
            restaure = en_images(self.modele(en_tenseur(visage[None], self.modele.supports_half)))[0]
        inverse = cv2.invertAffineTransform(m)
        retour = cv2.warpAffine(restaure, inverse, (w, h), flags=cv2.INTER_LINEAR, borderMode=cv2.BORDER_REFLECT)
        alpha = cv2.warpAffine(self.masque, inverse, (w, h))[..., None] * MELANGE_VISAGE
        return (retour * alpha + image * (1 - alpha)).astype(np.uint8)


def traiter(travail: dict, agrandisseur, restaurateur, avance) -> None:
    x, y, w, h = (int(v) for v in travail["crop"])
    ips = travail.get("ips", IPS)
    w, h = w - w % 2, h - h % 2
    lecteur = subprocess.Popen(
        ["ffmpeg", "-v", "error", "-ss", f"{travail['debut']:.3f}", "-i", travail["entree"], "-t", f"{travail['duree']:.3f}",
         "-vf", f"crop={w}:{h}:{x}:{y},fps={ips}", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
        stdout=subprocess.PIPE)
    W, H = w * ECHELLE, h * ECHELLE
    largeur_max = int(travail.get("largeur_max") or W)
    if W > largeur_max:
        H = max(2, round(H * largeur_max / W / 2) * 2)
        W = max(2, largeur_max // 2 * 2)
    ecrivain = subprocess.Popen(
        ["ffmpeg", "-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{W}x{H}", "-r", str(ips),
         "-i", "-", "-c:v", "libx264", "-preset", "veryfast", "-crf", "12", "-pix_fmt", "yuv420p", travail["sortie"]],
        stdin=subprocess.PIPE)
    taille = w * h * 3
    par_lot = max(1, min(LOT, PIXELS_PAR_LOT // (w * h)))
    melange = min(1.0, max(0.0, float(travail.get("melange", 1.0))))
    total = max(1, round(travail["duree"] * ips))
    faites = 0
    while True:
        lot = []
        for _ in range(par_lot):
            brut = lecteur.stdout.read(taille)
            if len(brut) < taille:
                break
            lot.append(np.frombuffer(brut, np.uint8).reshape(h, w, 3))
        if not lot:
            break
        with torch.inference_mode():
            entree = en_tenseur(np.stack(lot), agrandisseur.supports_half)
            t = agrandisseur(entree)
            if t.shape[-1] != W or t.shape[-2] != H:
                t = F.interpolate(t.float(), size=(H, W), mode="bicubic", antialias=True)
            if melange < 1.0:
                simple = F.interpolate(entree.float(), size=(H, W), mode="bicubic", align_corners=False)
                t = t.float() * melange + simple * (1.0 - melange)
            grandes = en_images(t)
            del entree, t
        for image in grandes:
            if restaurateur:
                image = restaurateur(np.ascontiguousarray(image))
            ecrivain.stdin.write(np.ascontiguousarray(image).tobytes())
        faites += len(lot)
        avance(min(faites / total, 1.0))
    lecteur.wait()
    ecrivain.stdin.close()
    if ecrivain.wait() != 0:
        raise RuntimeError(f"encodage de {travail['sortie']} en échec")


def main() -> None:
    travaux = json.loads(Path(sys.argv[1]).read_text())
    agrandisseur = charger("realesr-general-x4v3.pth")
    restaurateur = Restaurateur() if any(t.get("visage") for t in travaux) else None
    poids = [max(t["duree"], 0.01) for t in travaux]
    fait = 0.0
    signaler(0)
    for t, p in zip(travaux, poids):
        traiter(t, agrandisseur, restaurateur if t.get("visage") else None,
                lambda f, base=fait, p=p: signaler(100 * (base + f * p) / sum(poids)))
        fait += p
    signaler(100)
    print(json.dumps({"vram_max_mo": round(torch.cuda.max_memory_allocated() / 2**20)}), flush=True)


if __name__ == "__main__":
    main()
