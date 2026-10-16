"""EDIVID — capteurs d'un rush : transcription au mot, enveloppe sonore, visage et yeux.

Usage : python analyse.py <fichier_de_travail.mkv> <dossier_sortie>

Écrit dans <dossier_sortie> :
  analyse.json   durée, dimensions, mots horodatés (avec énergie), relevés du visage ;
  enveloppe.f32  niveau sonore en dBFS, 100 valeurs par seconde, flottants 32 bits petit-boutistes.
Si analyse.json contient déjà la transcription, elle est réutilisée.
Progression sur la sortie standard, une ligne JSON par étape : {"etape": ..., "pct": ...}.
"""
import ctypes
import glob
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import cv2
import numpy as np
import soundfile as sf

ICI = Path(__file__).resolve().parent
MODELE_VISAGE = ICI.parent / "modeles" / "yunet.onnx"
CADENCE_VISAGE = 5     # images analysées par seconde
COTE_ANALYSE = 2048    # grand côté de l'image passée à YuNet (les petits visages y gagnent)
PAS_ENVELOPPE = 0.01   # 10 ms


def signaler(etape: str, pct: float, **extra) -> None:
    print(json.dumps({"etape": etape, "pct": round(pct, 1), **extra}, ensure_ascii=False), flush=True)


def precharger_cuda() -> None:
    """CTranslate2 cherche libcublas.so.12 au chargement : on charge d'abord celles du venv."""
    import nvidia  # paquets nvidia-cublas-cu12 et nvidia-cudnn-cu12
    for dossier in nvidia.__path__:
        for motif in ("cublas/lib/libcublasLt.so.*", "cublas/lib/libcublas.so.*", "cudnn/lib/libcudnn*.so.*"):
            for lib in sorted(glob.glob(os.path.join(dossier, motif))):
                try:
                    ctypes.CDLL(lib, mode=ctypes.RTLD_GLOBAL)
                except OSError:
                    pass


def extraire_audio(video: str, wav: str) -> None:
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", video, "-ac", "1", "-ar", "16000", wav], check=True)


def transcrire(audio: np.ndarray, duree: float) -> list[dict]:
    precharger_cuda()
    from faster_whisper import WhisperModel
    modele = WhisperModel("large-v3", device="cuda", compute_type="float16")
    segments, _ = modele.transcribe(audio, language="fr", word_timestamps=True, beam_size=5)
    mots = []
    for seg in segments:
        for m in seg.words:
            mots.append({"t0": round(m.start, 3), "t1": round(m.end, 3), "mot": m.word.strip(), "p": round(m.probability, 2)})
        signaler("transcription", 100 * min(seg.end / max(duree, 1e-6), 1.0))
    return mots


def enveloppe(audio: np.ndarray, sr: int) -> np.ndarray:
    """Niveau RMS en dBFS sur des fenêtres de 20 ms, toutes les 10 ms."""
    pas, fen = int(sr * PAS_ENVELOPPE), int(sr * 0.02)
    n = max(1, len(audio) // pas)
    carre = np.concatenate([[0.0], np.cumsum(audio.astype(np.float64) ** 2)])
    debut = np.clip(np.arange(n) * pas - fen // 2, 0, len(audio))
    fin = np.clip(debut + fen, 0, len(audio))
    rms = np.sqrt((carre[fin] - carre[debut]) / np.maximum(fin - debut, 1))
    return (20 * np.log10(rms + 1e-7)).astype("<f4")


def energie_des_mots(env: np.ndarray, mots: list[dict]) -> None:
    """Énergie de chaque mot en dB par rapport à la médiane des mots de la vidéo."""
    niveaux = []
    for m in mots:
        a, b = int(m["t0"] / PAS_ENVELOPPE), max(int(m["t1"] / PAS_ENVELOPPE), int(m["t0"] / PAS_ENVELOPPE) + 1)
        tranche = env[a:b]
        niveaux.append(float(tranche.max()) if len(tranche) else -70.0)
    mediane = float(np.median(niveaux)) if niveaux else 0.0
    for m, n in zip(mots, niveaux):
        m["db"] = round(n - mediane, 1)


def visages(video: str, duree: float) -> list[dict]:
    """Cadre du visage et des yeux, en coordonnées normalisées 0–1."""
    cap = cv2.VideoCapture(video)
    fps = cap.get(cv2.CAP_PROP_FPS) or 30
    w0, h0 = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH)), int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT))
    f = min(2.0, COTE_ANALYSE / max(w0, h0))
    l, h = int(w0 * f), int(h0 * f)
    detecteur = cv2.FaceDetectorYN.create(str(MODELE_VISAGE), "", (l, h), 0.7)
    pas = max(1, round(fps / CADENCE_VISAGE))
    releves, i = [], 0
    while True:
        if i % pas:
            if not cap.grab():
                break
            i += 1
            continue
        ok, image = cap.read()
        if not ok:
            break
        _, faces = detecteur.detect(cv2.resize(image, (l, h), interpolation=cv2.INTER_CUBIC if f > 1 else cv2.INTER_AREA))
        if faces is not None:
            fc = max(faces, key=lambda v: v[2] * v[3])
            x, y, w, hh = fc[:4]
            (oxd, oyd), (oxg, oyg) = fc[4:6], fc[6:8]
            ecart = abs(oxg - oxd)
            releves.append({"t": round(i / fps, 2),
                            "visage": [round(float(v), 4) for v in (x / l, y / h, (x + w) / l, (y + hh) / h)],
                            "yeux": [round(float(v), 4) for v in (
                                (min(oxd, oxg) - ecart * 0.5) / l, (min(oyd, oyg) - ecart * 0.35) / h,
                                (max(oxd, oxg) + ecart * 0.5) / l, (max(oyd, oyg) + ecart * 0.35) / h)]})
        if (i // pas) % 50 == 0:
            signaler("visage", 100 * min(i / fps / max(duree, 1e-6), 1.0))
        i += 1
    cap.release()
    return releves


def sonder(video: str) -> tuple[int, int, float]:
    sortie = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
                             "stream=width,height:format=duration", "-of", "json", video],
                            capture_output=True, text=True, check=True).stdout
    d = json.loads(sortie)
    return int(d["streams"][0]["width"]), int(d["streams"][0]["height"]), float(d["format"]["duration"])


def main() -> None:
    video, dossier = sys.argv[1], Path(sys.argv[2])
    dossier.mkdir(parents=True, exist_ok=True)
    sortie = dossier / "analyse.json"
    largeur, hauteur, duree = sonder(video)
    ancien = json.loads(sortie.read_text()) if sortie.exists() else {}
    with tempfile.TemporaryDirectory() as tmp:
        wav = f"{tmp}/audio.wav"
        signaler("son", 0)
        extraire_audio(video, wav)
        audio, sr = sf.read(wav, dtype="float32")
    env = enveloppe(audio, sr)
    (dossier / "enveloppe.f32").write_bytes(env.tobytes())
    if ancien.get("mots"):
        mots = ancien["mots"]
    else:
        signaler("transcription", 0)
        mots = transcrire(audio, duree)
    energie_des_mots(env, mots)
    signaler("visage", 0)
    faces = visages(video, duree)
    sortie.write_text(json.dumps({"largeur": largeur, "hauteur": hauteur, "duree": duree, "mots": mots,
                                  "visages": faces}, ensure_ascii=False))
    signaler("fini", 100, mots=len(mots), visages=len(faces))


if __name__ == "__main__":
    main()
