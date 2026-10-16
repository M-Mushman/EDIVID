"""EDIVID P0 — capteurs : transcription au mot, énergie vocale, visage et yeux.

Usage : python analyse.py <video> <sortie.json>
Si <sortie.json> contient déjà la transcription, elle est réutilisée.
"""
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import cv2
import numpy as np
import soundfile as sf

MODELE_VISAGE = Path(__file__).resolve().parent.parent / "modeles" / "yunet.onnx"
CADENCE_VISAGE = 5  # images analysées par seconde
AGRANDISSEMENT = 2  # YuNet voit mieux les petits visages sur une image agrandie


def extraire_audio(video: str, wav: str) -> None:
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", video, "-ac", "1", "-ar", "16000", wav], check=True)


def transcrire(wav: str) -> list[dict]:
    from faster_whisper import WhisperModel
    modele = WhisperModel("large-v3", device="cuda", compute_type="float16")
    audio, _ = sf.read(wav, dtype="float32")
    segments, _ = modele.transcribe(audio, language="fr", word_timestamps=True, beam_size=5)
    mots = []
    for seg in segments:
        for m in seg.words:
            mots.append({"t0": round(m.start, 3), "t1": round(m.end, 3), "mot": m.word.strip(), "p": round(m.probability, 2)})
        print(f"  {seg.end:7.1f}s  {seg.text.strip()[:70]}", flush=True)
    return mots


def energie(wav: str, mots: list[dict]) -> None:
    """Ajoute à chaque mot son énergie en dB relative à la médiane de la vidéo."""
    audio, sr = sf.read(wav)
    rms = []
    for m in mots:
        bout = audio[int(m["t0"] * sr): max(int(m["t1"] * sr), int(m["t0"] * sr) + 1)]
        rms.append(float(np.sqrt(np.mean(bout ** 2))) + 1e-9)
    mediane = float(np.median(rms))
    for m, r in zip(mots, rms):
        m["db"] = round(20 * np.log10(r / mediane), 1)


def visages(video: str) -> list[dict]:
    """Cadre du visage et des yeux, en coordonnées normalisées 0–1."""
    cap = cv2.VideoCapture(video)
    fps = cap.get(cv2.CAP_PROP_FPS) or 30
    l, h = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH)) * AGRANDISSEMENT, int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT)) * AGRANDISSEMENT
    detecteur = cv2.FaceDetectorYN.create(str(MODELE_VISAGE), "", (l, h), 0.7)
    pas = max(1, round(fps / CADENCE_VISAGE))
    releves, i = [], 0
    while True:
        ok, image = cap.read()
        if not ok:
            break
        if i % pas == 0:
            _, faces = detecteur.detect(cv2.resize(image, (l, h), interpolation=cv2.INTER_CUBIC))
            if faces is not None:
                f = max(faces, key=lambda f: f[2] * f[3])
                x, y, w, hh = f[:4]
                (oxd, oyd), (oxg, oyg) = f[4:6], f[6:8]
                ecart = abs(oxg - oxd)
                releves.append({"t": round(i / fps, 2),
                                "visage": [round(float(v), 4) for v in (x / l, y / h, (x + w) / l, (y + hh) / h)],
                                "yeux": [round(float(v), 4) for v in (
                                    (min(oxd, oxg) - ecart * 0.5) / l, (min(oyd, oyg) - ecart * 0.35) / h,
                                    (max(oxd, oxg) + ecart * 0.5) / l, (max(oyd, oyg) + ecart * 0.35) / h)]})
        i += 1
    cap.release()
    return releves


def main() -> None:
    video, sortie = sys.argv[1], Path(sys.argv[2])
    largeur, hauteur = map(int, subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height",
         "-of", "csv=p=0", video], capture_output=True, text=True, check=True).stdout.strip().split(","))
    duree = float(subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", video],
                                 capture_output=True, text=True, check=True).stdout)
    ancien = json.loads(sortie.read_text()) if sortie.exists() else {}
    if ancien.get("mots"):
        mots = ancien["mots"]
        print(f"Transcription réutilisée ({len(mots)} mots)", flush=True)
    else:
        with tempfile.TemporaryDirectory() as tmp:
            wav = f"{tmp}/audio.wav"
            extraire_audio(video, wav)
            print("Transcription…", flush=True)
            mots = transcrire(wav)
            energie(wav, mots)
    print("Visage…", flush=True)
    faces = visages(video)
    print(f"{len(mots)} mots, visage trouvé sur {len(faces)} relevés", flush=True)
    sortie.write_text(json.dumps({"video": str(Path(video).resolve()), "largeur": largeur, "hauteur": hauteur,
                                  "duree": duree, "mots": mots, "visages": faces}, ensure_ascii=False))


if __name__ == "__main__":
    main()
