"""Recolle le visage de la photo d'origine sur une animation Wan, image par image (processeur seul, sans carte).

Usage : python visage.py <anim.mp4> <photo.jpg> <sortie.mp4> [agrandissement=3 en horizontal, 2 en vertical]

Sur une pose en pied, le visage ne fait qu'une cinquantaine de pixels dans l'image de Wan (1280×704) : le modèle le
redessine à chaque image et les traits dérivent (yeux baveux, visage méconnaissable). Ici :
1. la photo est recalée sur la première image de l'animation (points SIFT, affinité robuste) ;
2. le visage est repéré sur la photo (YuNet, OpenCV, MIT) ;
3. son mouvement dans l'animation est suivi par flot optique, réduit à une similitude (déplacement, rotation,
   échelle) par image, lissée dans le temps : le visage d'origine suit la tête sans en reprendre les déformations ;
4. il est recollé à la définition de sortie (photo pleine définition, bien plus nette que l'image de Wan agrandie),
   dans une ellipse fondue, avec la lumière locale de l'animation (écart des moyennes floues).
Le reste de l'image (cheveux au vent, décor, mouvement de caméra) reste celui de Wan, agrandi.
Sortie : H.264 quasi sans perte (crf 12), à l'agrandissement voulu (3840×2112 ou 1408×2560), pour composer.py.
"""
import json
import subprocess
import sys
from pathlib import Path

import cv2
import numpy as np

YUNET = Path(__file__).resolve().parents[2] / "modeles/yunet/face_detection_yunet_2023mar.onnx"


def lire(video: Path) -> tuple[np.ndarray, int, int]:
    info = json.loads(subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
                                      "stream=width,height", "-of", "json", str(video)],
                                     capture_output=True, check=True).stdout)["streams"][0]
    w, h = info["width"], info["height"]
    brut = subprocess.run(["ffmpeg", "-v", "error", "-i", str(video), "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
                          capture_output=True, check=True).stdout
    return np.frombuffer(brut, np.uint8).reshape(-1, h, w, 3), w, h


def recaler(photo: np.ndarray, image0: np.ndarray) -> np.ndarray:
    """Affinité 2×3 photo → image de l'animation, par points SIFT appariés."""
    sift = cv2.SIFT_create(6000)
    gp, gi = cv2.cvtColor(photo, cv2.COLOR_RGB2GRAY), cv2.cvtColor(image0, cv2.COLOR_RGB2GRAY)
    # La photo réduite à peu près à l'échelle de l'animation, pour des descripteurs comparables.
    r = image0.shape[0] / photo.shape[0] * (1.0 if image0.shape[0] < image0.shape[1] else 0.62)
    petite = cv2.resize(gp, None, fx=r, fy=r, interpolation=cv2.INTER_AREA)
    kp, dp = sift.detectAndCompute(petite, None)
    ki, di = sift.detectAndCompute(gi, None)
    paires = [m for m, n in cv2.BFMatcher().knnMatch(dp, di, k=2) if m.distance < 0.75 * n.distance]
    a = np.float32([kp[m.queryIdx].pt for m in paires]) / r
    b = np.float32([ki[m.trainIdx].pt for m in paires])
    M, garde = cv2.estimateAffine2D(a, b, method=cv2.RANSAC, ransacReprojThreshold=1.5, maxIters=5000)
    if M is None or garde.sum() < 30:
        raise SystemExit(f"recalage impossible ({len(paires)} paires)")
    return M


def visage(photo: np.ndarray) -> np.ndarray:
    """Boîte (x, y, l, h) et cinq repères du visage le plus sûr de la photo."""
    d = cv2.FaceDetectorYN.create(str(YUNET), "", (photo.shape[1], photo.shape[0]), 0.6)
    _, f = d.detect(cv2.cvtColor(photo, cv2.COLOR_RGB2BGR))
    if f is None:
        raise SystemExit("aucun visage sur la photo")
    return f[np.argmax(f[:, 14])]


def masque_ellipse(forme: tuple[int, int], f: np.ndarray, flou: float) -> np.ndarray:
    """Ellipse du front au menton, inclinée comme la ligne des yeux, fondue sur ses bords."""
    x, y, w, h = f[:4]
    (ox, oy), (gx, gy) = f[4:6], f[6:8]
    angle = np.degrees(np.arctan2(gy - oy, gx - ox))
    m = np.zeros(forme, np.float32)
    cv2.ellipse(m, (int(x + w / 2), int(y + h * 0.52)), (int(w * 0.44), int(h * 0.54)), angle, 0, 360, 1.0, -1)
    if not flou:
        return m
    return np.clip((cv2.GaussianBlur(m, (0, 0), flou) - 0.5) * 2 + 0.5, 0, 1)


def suivre(images: np.ndarray, zone: np.ndarray) -> np.ndarray:
    """Similitude 3×3 image 0 → image n du visage (flot optique dans la zone, RANSAC), lissée dans le temps."""
    dis = cv2.DISOpticalFlow_create(cv2.DISOPTICAL_FLOW_PRESET_MEDIUM)
    g0 = cv2.cvtColor(images[0], cv2.COLOR_RGB2GRAY)
    ys, xs = np.nonzero(zone > 0.5)
    pas = max(1, len(xs) // 1500)
    p0 = np.float32(np.stack([xs[::pas], ys[::pas]], axis=1))
    params = []
    for im in images:
        flot = dis.calc(g0, cv2.cvtColor(im, cv2.COLOR_RGB2GRAY), None)
        p1 = p0 + flot[p0[:, 1].astype(int), p0[:, 0].astype(int)]
        M, _ = cv2.estimateAffinePartial2D(p0, p1, method=cv2.RANSAC, ransacReprojThreshold=0.8)
        if M is None:
            M = params[-1] if params else np.float64([[1, 0, 0], [0, 1, 0]])
        params.append(M)
    # Lissage : les petites secousses du flot ne doivent pas faire trembler le visage.
    P = np.array([[M[0, 0], M[1, 0], M[0, 2], M[1, 2]] for M in params])
    noyau = np.exp(-0.5 * (np.arange(-6, 7) / 2.0) ** 2)
    noyau /= noyau.sum()
    P = np.stack([np.convolve(np.pad(P[:, k], 6, mode="edge"), noyau, mode="valid") for k in range(4)], axis=1)
    P -= P[0] - [1, 0, 0, 0]  # l'image 0 est la photo, exactement
    return np.array([[[a, -b, tx], [b, a, ty], [0, 0, 1]] for a, b, tx, ty in P])


def flou_pondere(img: np.ndarray, m: np.ndarray, s: float) -> np.ndarray:
    num = cv2.GaussianBlur(img * m[..., None], (0, 0), s)
    den = cv2.GaussianBlur(m, (0, 0), s)[..., None]
    return num / np.maximum(den, 1e-3)


def main() -> None:
    anim, photo_f, sortie = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    images, w, h = lire(anim)
    k = float(sys.argv[4]) if len(sys.argv) > 4 else (3.0 if w > h else 2.0)
    W, H = round(w * k / 2) * 2, round(h * k / 2) * 2
    photo = cv2.cvtColor(cv2.imread(str(photo_f)), cv2.COLOR_BGR2RGB)
    A = np.vstack([recaler(photo, images[0]), [0, 0, 1]])  # photo → animation
    f = visage(photo)
    taille = float(f[3])  # hauteur du visage sur la photo
    masque_photo = masque_ellipse(photo.shape[:2], f, 0.07 * taille)
    zone = cv2.warpAffine(masque_ellipse(photo.shape[:2], f, 0), A[:2], (w, h))
    suivi = suivre(images, zone)
    S = np.diag([W / w, H / h, 1.0])
    # Seule une fenêtre autour du visage est calculée à la définition de sortie.
    coin = (S @ A @ np.float64([[f[0] - taille, f[1] - taille, 1], [f[0] + f[2] + taille, f[1] + 2 * taille, 1]]).T).T
    x0, y0 = np.clip(np.floor(coin[:, :2].min(axis=0)).astype(int), 0, [W, H])
    x1, y1 = np.clip(np.ceil(coin[:, :2].max(axis=0)).astype(int), 0, [W, H])
    T0 = np.float64([[1, 0, -x0], [0, 1, -y0], [0, 0, 1]])
    s_lum = 0.18 * taille * (S @ A)[1, 1]  # flou de la correction de lumière, en pixels de sortie
    sortie.parent.mkdir(parents=True, exist_ok=True)
    enc = subprocess.Popen(["ffmpeg", "-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{W}x{H}",
                            "-r", "30", "-i", "-", "-c:v", "libx264", "-crf", "12", "-preset", "slow",
                            "-pix_fmt", "yuv420p", "-colorspace", "bt709", "-color_primaries", "bt709",
                            "-color_trc", "bt709", str(sortie)], stdin=subprocess.PIPE)
    deplacement = []
    for im, Mn in zip(images, suivi):
        grand = cv2.resize(im, (W, H), interpolation=cv2.INTER_LANCZOS4)
        T = (T0 @ S @ Mn @ A)[:2]
        fen = (x1 - x0, y1 - y0)
        v = cv2.warpAffine(photo, T, fen, flags=cv2.INTER_LANCZOS4, borderMode=cv2.BORDER_REFLECT).astype(np.float32)
        m = cv2.warpAffine(masque_photo, T, fen, flags=cv2.INTER_LINEAR)
        fond = grand[y0:y1, x0:x1].astype(np.float32)
        # Lumière de l'animation : la photo prend l'écart local de couleur entre Wan et elle (basses fréquences).
        v += flou_pondere(fond, m, s_lum) - flou_pondere(v, m, s_lum)
        grand[y0:y1, x0:x1] = np.clip(fond * (1 - m[..., None]) + v * m[..., None], 0, 255).astype(np.uint8)
        enc.stdin.write(grand.tobytes())
        deplacement.append(float(np.hypot(Mn[0, 2], Mn[1, 2])))
    enc.stdin.close()
    if enc.wait():
        raise SystemExit("encodage en échec")
    print(json.dumps({"sortie": str(sortie), "taille": [W, H], "visage_px_sortie": round(taille * (S @ A)[1, 1]),
                      "deplacement_max_px": round(max(deplacement), 1)}), flush=True)


if __name__ == "__main__":
    main()
