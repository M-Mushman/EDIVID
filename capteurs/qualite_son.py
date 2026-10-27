"""EDIVID — qualité d'une voix sans référence : DNSMOS P.835 (Microsoft, CC BY 4.0), sur le processeur.

Usage : python qualite_son.py <fichier.wav> [<fichier.wav>…]

Pour chaque fichier, trois notes de 1 à 5 : SIG (qualité de la parole), BAK (gêne du fond), OVRL (impression
d'ensemble), plus P808 (note globale d'un second modèle). Méthode de dnsmos_local.py du dépôt DNS-Challenge :
mono 16 kHz, fenêtres de 9,01 s avancées d'une seconde, polynômes de correction, moyenne des fenêtres. Les modèles
(modeles/dnsmos/sig_bak_ovr.onnx et model_v8.onnx) ont été entraînés sur des notes humaines de débruitage : la note
sanctionne aussi la réverbération et les artefacts, pas seulement le bruit.
Sortie : une ligne JSON par fichier sur la sortie standard, {"fichier", "SIG", "BAK", "OVRL", "P808", "fenetres"}.
"""
import json
import sys
from pathlib import Path

import librosa
import numpy as np
import onnxruntime as ort
import soundfile as sf

MODELES = Path(__file__).resolve().parent.parent / "modeles" / "dnsmos"
SR = 16000
FENETRE = 9.01  # s

P_OVR = np.poly1d([-0.06766283, 1.11546468, 0.04602535])
P_SIG = np.poly1d([-0.08397278, 1.22083953, 0.0052439])
P_BAK = np.poly1d([-0.13166888, 1.60915514, -0.39604546])


def mel(audio: np.ndarray) -> np.ndarray:
    m = librosa.feature.melspectrogram(y=audio, sr=SR, n_fft=321, hop_length=160, n_mels=120)
    return ((librosa.power_to_db(m, ref=np.max) + 40) / 40).T


def noter(principal, p808, fichier: str) -> dict:
    son, sr = sf.read(fichier, dtype="float32", always_2d=True)
    audio = son.mean(axis=1)
    if sr != SR:
        audio = librosa.resample(audio, orig_sr=sr, target_sr=SR)
    n = int(FENETRE * SR)
    while len(audio) < n:
        audio = np.append(audio, audio)
    fenetres = int(np.floor(len(audio) / SR) - FENETRE) + 1
    sig, bak, ovr, p8 = [], [], [], []
    for i in range(fenetres):
        seg = audio[i * SR:int((i + FENETRE) * SR)]
        if len(seg) < n:
            continue
        s, b, o = principal.run(None, {"input_1": seg[np.newaxis, :].astype(np.float32)})[0][0]
        p8.append(float(p808.run(None, {"input_1": mel(seg[:-160])[np.newaxis].astype(np.float32)})[0][0][0]))
        sig.append(float(P_SIG(s)))
        bak.append(float(P_BAK(b)))
        ovr.append(float(P_OVR(o)))
    r = lambda v: round(float(np.mean(v)), 3) if v else None
    return {"fichier": fichier, "SIG": r(sig), "BAK": r(bak), "OVRL": r(ovr), "P808": r(p8), "fenetres": len(sig)}


def main() -> None:
    options = ort.SessionOptions()
    options.log_severity_level = 3
    principal = ort.InferenceSession(str(MODELES / "sig_bak_ovr.onnx"), options, providers=["CPUExecutionProvider"])
    p808 = ort.InferenceSession(str(MODELES / "model_v8.onnx"), options, providers=["CPUExecutionProvider"])
    for f in sys.argv[1:]:
        print(json.dumps(noter(principal, p808, f), ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
