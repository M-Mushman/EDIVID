"""EDIVID — voix de studio sans mot inventé : vote phrase par phrase entre plusieurs reconstructions VoiceFixer.

Usage : python vote_voix.py <reference.wav> <repli.wav> <sortie.wav> <bilan.json> <analyse.json> <debut_s>
                            <candidat.wav> [<candidat.wav>…]

<reference.wav> : la voix séchée par un modèle qui n'invente rien (TF-GridNet). Sa transcription par Whisper et celle du
rush brut (<analyse.json>, décalée de <debut_s>) font foi pour les mots : un candidat est juste s'il dit les mots de
l'une ou de l'autre (Whisper hésite parfois lui-même sur un nom propre ou une liaison). <candidat.wav> : des reconstructions VoiceFixer (le son qu'on veut), tirées d'entrées différentes : elles
ne se trompent pas aux mêmes endroits. <repli.wav> : la voix de référence égalisée vers le timbre des candidats, pour les
phrases qu'aucun candidat ne rend juste.

Les tranches sont des groupes de mots de la référence de 1 à 4 s, coupés dans les silences d'au moins 0,15 s. Pour
chacune, on prend le premier candidat dont les mots (lettres et chiffres seulement) sont exactement ceux d'une des deux
références ; sinon le repli. Les
raccords tombent au milieu des silences entre phrases, avec un fondu de 40 ms : jamais dans un mot (les raccords mot à
mot faisaient changer le timbre toutes les 7 s, écoute du 2026-10-26). Tous les fichiers ont la même durée et sont
calés à l'échantillon (restaure.py compense l'avance de VoiceFixer).
Sortie : WAV 48 kHz stéréo ; bilan JSON {phrases, par_source, phrases_repli: [[t0, t1, référence, brut, candidats…]]}.
Progression : {"etape": "verification", "pct": ...}.
"""
import json
import re
import subprocess
import unicodedata
import sys
from pathlib import Path

import numpy as np
import soundfile as sf

sys.path.insert(0, str(Path(__file__).resolve().parent))
from analyse import precharger_cuda  # noqa: E402

SR = 48000
PAUSE = 0.15        # s : silence où l'on peut couper
TRANCHE_MIN = 1.0   # s
TRANCHE_MAX = 4.0   # s
FONDU = 0.04


def signaler(pct: float) -> None:
    print(json.dumps({"etape": "verification", "pct": round(pct, 1)}), flush=True)


def son16(f: str) -> np.ndarray:
    brut = subprocess.run(["ffmpeg", "-v", "error", "-i", f, "-ac", "1", "-ar", "16000", "-f", "f32le", "-"], capture_output=True, check=True).stdout
    return np.frombuffer(brut, np.float32)


def transcrire(modele, f: str) -> list:
    segments, _ = modele.transcribe(son16(f), language="fr", word_timestamps=True, beam_size=5, condition_on_previous_text=False)
    return [(m.start, m.end, m.word.strip()) for s in segments for m in s.words]


def cle(mots: list) -> str:
    """Les mots réduits à leurs lettres et chiffres, sans accents, collés : « e-mail » et « email », « A » et « À » sont
    le même mot (Whisper hésite sur la ponctuation et les majuscules accentuées)."""
    texte = unicodedata.normalize("NFD", " ".join(m for _, _, m in mots).lower())
    return re.sub(r"[^0-9a-z]", "", "".join(c for c in texte if not unicodedata.combining(c)))


def phrases(mots: list) -> list:
    """Tranches de 1 à 4 s, coupées dans un silence d'au moins PAUSE (au-delà de 4 s, entre deux mots)."""
    groupes, courant = [], []
    for m in mots:
        if courant:
            longueur = courant[-1][1] - courant[0][0]
            silence = m[0] - courant[-1][1]
            # Au-delà de 4 s sans silence, on coupe entre deux mots : entre deux versions VoiceFixer (même timbre), le
            # fondu de 40 ms ne s'entend pas.
            if (silence >= PAUSE and longueur >= TRANCHE_MIN) or longueur >= TRANCHE_MAX:
                groupes.append(courant)
                courant = []
        courant.append(m)
    if courant:
        groupes.append(courant)
    return groupes


def main() -> None:
    reference, repli, sortie, fichier_bilan, analyse = sys.argv[1:6]
    debut = float(sys.argv[6])
    candidats = sys.argv[7:]
    mots_brut = [(m["t0"] - debut, m["t1"] - debut, m["mot"]) for m in json.loads(Path(analyse).read_text())["mots"]]
    signaler(0)
    precharger_cuda()
    from faster_whisper import WhisperModel
    modele = WhisperModel("large-v3", device="cuda", compute_type="float16")
    mots_ref = transcrire(modele, reference)
    transcriptions = []
    for i, c in enumerate(candidats):
        transcriptions.append(transcrire(modele, c))
        signaler(10 + 70 * (i + 1) / len(candidats))
    sons = [sf.read(f, dtype="float32", always_2d=True)[0] for f in candidats + [repli]]
    n = min(len(s) for s in sons)
    sons = [s[:n, :2] if s.shape[1] >= 2 else np.repeat(s[:n], 2, axis=1) for s in sons]
    duree = n / SR
    groupes = phrases(mots_ref)
    # Bornes des tranches : au milieu des silences entre phrases.
    bornes = [0.0] + [(a[-1][1] + b[0][0]) / 2 for a, b in zip(groupes, groupes[1:])] + [duree]
    choix, repli_detail = [], []
    for k, g in enumerate(groupes):
        t0, t1 = bornes[k], bornes[k + 1]
        dans = lambda mots: [m for m in mots if t0 <= (m[0] + m[1]) / 2 < t1]
        attendus = {cle(dans(mots_ref)), cle(dans(mots_brut))}
        retenu = next((i for i, tr in enumerate(transcriptions) if cle(dans(tr)) in attendus), None)
        if retenu is None:
            retenu = len(candidats)
            repli_detail.append([round(t0, 2), round(t1, 2), " ".join(m for _, _, m in dans(mots_ref)), " ".join(m for _, _, m in dans(mots_brut))]
                                + [" ".join(m for _, _, m in dans(tr)) for tr in transcriptions])
        choix.append(retenu)
    # Poids de chaque source, avec un fondu à chaque changement.
    poids = np.zeros((len(sons), n), dtype=np.float32)
    f = int(FONDU * SR)
    for k, src in enumerate(choix):
        i0, i1 = int(bornes[k] * SR), min(n, int(bornes[k + 1] * SR))
        poids[src, i0:i1] = 1.0
    for k in range(1, len(choix)):
        if choix[k] != choix[k - 1]:
            c = int(bornes[k] * SR)
            a, b = max(0, c - f // 2), min(n, c + f // 2)
            rampe = np.linspace(0, 1, b - a, dtype=np.float32)
            poids[choix[k - 1], a:b] = 1 - rampe
            poids[choix[k], a:b] = rampe
    sortie_son = sum(poids[i][:, None] * sons[i] for i in range(len(sons)))
    sf.write(sortie, np.clip(sortie_son, -1, 1), SR, subtype="PCM_16")
    noms = [Path(c).name for c in candidats] + ["repli"]
    bilan = {"methode": "vote", "phrases": len(groupes), "par_source": {noms[i]: choix.count(i) for i in range(len(noms))},
             "phrases_repli": repli_detail}
    Path(fichier_bilan).write_text(json.dumps(bilan, ensure_ascii=False))
    signaler(100)
    print(json.dumps({k: v for k, v in bilan.items() if k != "phrases_repli"}, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
