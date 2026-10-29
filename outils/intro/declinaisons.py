"""Déclinaisons d'un morceau d'ACE-Step : le jingle de l'intro et des transitions plus longues, tirés du même morceau.

Usage : python declinaisons.py <morceau.wav> <bpm> <dossier_sortie>

Le passage retenu commence sur une mesure (4 temps), là où les 3 secondes qui suivent sont les plus énergiques (un
jingle doit démarrer plein). Depuis ce même départ :
- jingle_3s : l'intro (fondu de fin de 0,5 s) ;
- transition_8s : un nombre entier de mesures proche de 8 s, fondu de fin de 1 s ;
- fin_16s : environ 16 s, longue sortie en fondu (3 s).
Et deux versions qui finissent franchement, sur l'accord final du morceau (ACE-Step termine ses morceaux sur un
accord tenu qui s'éteint), en partant d'un premier temps (départ net) :
- intro : la dernière mesure, puis l'accord final tenu 1,6 s (fondu sur sa queue), environ 3,5 s à 124 BPM ;
- final : la phrase de 4 mesures qui mène à l'accord, puis l'accord jusqu'à son extinction (2 s au plus), environ
  9,7 s, pour la fin d'une vidéo.
Écrit les WAV (48 kHz, 24 bits), des MP3 pour l'écoute, et depart.json {debut_s, mesure_s, intro_s, final_s}.
"""
import json
import subprocess
import sys
from pathlib import Path

import numpy as np
import soundfile as sf


def extrait(x: np.ndarray, sr: int, debut: float, duree: float, fondu: float) -> np.ndarray:
    a, b = int(debut * sr), min(len(x), int((debut + duree) * sr))
    y = x[a:b].copy()
    n = int(0.005 * sr)
    y[:n] *= np.linspace(0, 1, n)[:, None]
    f = min(len(y), int(fondu * sr))
    y[-f:] *= (np.cos(np.linspace(0, np.pi / 2, f)) ** 2)[:, None]
    return y


def cadence(mono: np.ndarray, sr: int, mesure: float, mesures: int, tenue: float) -> tuple[float, float]:
    """(début, durée) de la fin du morceau : `mesures` mesures avant l'attaque de l'accord final, puis l'accord tenu
    `tenue` secondes au plus (moins s'il s'éteint avant)."""
    pas = 0.01
    h = int(pas * sr)
    n = len(mono) // h
    db = 20 * np.log10(np.sqrt((mono[:n * h].reshape(n, h) ** 2).mean(axis=1)) + 1e-9)
    lisse = np.convolve(db, np.ones(5) / 5, mode="same")
    corps = np.median(lisse[lisse > lisse.max() - 30])
    montee = np.diff(lisse, prepend=lisse[0])
    # Dernière trame plus forte que le corps du morceau : l'accord final ; son attaque est la plus forte montée juste avant.
    der = np.nonzero(lisse > corps + 2)[0][-1]
    attaque = der - 30 + int(np.argmax(montee[der - 30:der + 1]))
    eteint = np.nonzero(lisse[attaque:] < corps - 35)[0]
    fin = attaque + min(eteint[0] if len(eteint) else len(lisse) - attaque, round(tenue / pas))
    # Premier temps : `mesures` mesures plus tôt, recalé sur l'attaque la plus franche à ±80 ms.
    d = attaque - round(mesures * mesure / pas)
    d = d - 8 + int(np.argmax(montee[d - 8:d + 9]))
    return d * pas, (fin - d) * pas


def main() -> None:
    morceau, bpm, sortie = Path(sys.argv[1]), float(sys.argv[2]), Path(sys.argv[3])
    sortie.mkdir(parents=True, exist_ok=True)
    x, sr = sf.read(morceau, dtype="float32", always_2d=True)
    mono = x.mean(axis=1)
    mesure = 4 * 60 / bpm
    # Premier son franc : là où commence la grille des mesures.
    trame = int(0.01 * sr)
    rms = np.sqrt(np.convolve(mono ** 2, np.ones(trame) / trame, mode="same"))
    au_dessus = np.nonzero(rms > 10 ** (-30 / 20))[0]
    t0 = au_dessus[0] / sr if len(au_dessus) else 0.0
    duree = len(mono) / sr
    candidats = [t0 + k * mesure for k in range(16) if t0 + k * mesure + 16 <= duree] or [t0]
    def energie(t):
        a, b = int(t * sr), int((t + 3) * sr)
        attaque = np.sqrt(np.mean(mono[a:a + int(0.4 * sr)] ** 2))
        return np.sqrt(np.mean(mono[a:b] ** 2)) + 0.5 * attaque
    debut = max(candidats, key=energie)
    mesures_8 = max(1, round(8 / mesure))
    debut_intro, duree_intro = cadence(mono, sr, mesure, 1, 1.6)
    debut_final, duree_final = cadence(mono, sr, mesure, 4, 2.0)
    for nom, de, d, fondu in [("jingle_3s", debut, 3.0, 0.5), ("transition_8s", debut, mesures_8 * mesure, 1.0),
                              ("fin_16s", debut, 16.0, 3.0), ("intro", debut_intro, duree_intro, 0.5),
                              ("final", debut_final, duree_final, 0.6)]:
        y = extrait(x, sr, de, d, fondu)
        wav = sortie / f"{nom}.wav"
        sf.write(wav, y, sr, subtype="PCM_24")
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(wav), "-af", "loudnorm=I=-16:TP=-1.5", "-ar", "48000",
                        "-c:a", "libmp3lame", "-b:a", "192k", str(wav.with_suffix(".mp3"))], check=True)
    (sortie / "depart.json").write_text(json.dumps({"debut_s": round(debut, 3), "mesure_s": round(mesure, 4),
                                                         "intro_s": round(debut_intro, 3), "final_s": round(debut_final, 3)}))
    print(json.dumps({"morceau": morceau.name, "debut_s": round(debut, 3)}))


if __name__ == "__main__":
    main()
