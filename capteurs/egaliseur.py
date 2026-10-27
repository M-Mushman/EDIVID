"""EDIVID — égaliseur par correspondance spectrale (« match EQ ») : donner à une voix la couleur d'une référence.

Usage :
  python egaliseur.py spectre <son> <spectre.json> [<analyse.json> <debut_s>]
      Spectre moyen de la voix : fenêtres de 8 192 échantillons à 48 kHz sur les seuls passages parlés (les mots de
      l'analyse s'ils sont donnés, sinon les fenêtres assez fortes), intégré en bandes de 1/6 d'octave de 63 Hz à
      16 kHz, normalisé à la bande 1 kHz. Écrit {"bandes": [{"f", "db"}], "fenetres", "source"}.
  python egaliseur.py courbe <spectre_source.json> <spectre_reference.json> <intensite 0-1> <sortie.json>
      Différence référence − source, lissée à 1/3 d'octave, bornée à ±8 dB, nulle sous 70 Hz et dégressive au-dessus
      de 12 kHz, multipliée par l'intensité ; puis traduite en filtres ffmpeg (equalizer/bass/treble, formules RBJ,
      les mêmes que web/son-dsp.js) par moindres carrés sur leur réponse réelle. Écrit {"filtres": [...],
      "cible": [...], "obtenue": [...], "erreur_db"}.
  python egaliseur.py essai
      Auto-contrôle : un bruit coloré est abîmé par ffmpeg (−6 dB à 300 Hz, +4 dB à 4 kHz), la courbe le ramène à
      ±1,5 dB de l'original (le lissage à 1/3 d'octave arrondit le creux). Imprime l'erreur et s'arrête en échec sinon.
Aucune carte graphique ; numpy seulement.
"""
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np

SR = 48000
N = 8192
PAS = 4096
# Bandes de 1/6 d'octave autour de 1 kHz : 63 Hz à 16 kHz.
BANDES = [1000.0 * 2 ** (k / 6) for k in range(-24, 25)]
# Filtres paramétriques de la traduction : centres et largeur (Q), étagères aux extrémités.
CENTRES = [100, 160, 250, 400, 630, 1000, 1600, 2500, 4000, 6300, 10000]
Q = 1.4
ETAGERE_BASSE = 80.0
ETAGERE_HAUTE = 12000.0
GAIN_MAX = 8.0
PLANCHER = 70.0
PLAFOND = 12000.0


def lire(fichier: str) -> np.ndarray:
    brut = subprocess.run(["ffmpeg", "-v", "error", "-i", fichier, "-vn", "-ac", "1", "-ar", str(SR), "-f", "f32le", "-"],
                          capture_output=True, check=True).stdout
    return np.frombuffer(brut, dtype=np.float32)


def fenetres_parlees(n_fenetres: int, mots: list | None, debut: float, niveaux: np.ndarray) -> np.ndarray:
    """Masque des fenêtres à garder : dans un mot si l'analyse est donnée, sinon assez fortes."""
    centres = (np.arange(n_fenetres) * PAS + N / 2) / SR
    if mots:
        garde = np.zeros(n_fenetres, dtype=bool)
        for m in mots:
            t0, t1 = m["t0"] - debut, m["t1"] - debut
            garde |= (centres >= t0) & (centres <= t1)
        if garde.sum() >= 20:
            return garde
    seuil = max(np.percentile(niveaux, 90) - 25.0, -60.0)
    return niveaux > seuil


def spectre(son: np.ndarray, mots: list | None = None, debut: float = 0.0) -> tuple[list, int]:
    nf = (len(son) - N) // PAS + 1
    if nf < 4:
        raise SystemExit("extrait trop court pour un spectre")
    fen = np.hanning(N).astype(np.float32)
    idx = np.arange(N)[None, :] + np.arange(nf)[:, None] * PAS
    trames = son[idx] * fen
    puissance = np.abs(np.fft.rfft(trames, axis=1)) ** 2
    niveaux = 10 * np.log10(puissance.mean(axis=1) + 1e-20)
    garde = fenetres_parlees(nf, mots, debut, niveaux)
    moyenne = puissance[garde].mean(axis=0)
    freqs = np.fft.rfftfreq(N, 1 / SR)
    bandes = []
    for f in BANDES:
        lo, hi = f / 2 ** (1 / 12), f * 2 ** (1 / 12)
        sel = (freqs >= lo) & (freqs < hi)
        # Une bande grave plus étroite qu'un pas de FFT : le pas le plus proche.
        p = moyenne[sel].mean() if sel.any() else moyenne[int(np.argmin(np.abs(freqs - f)))]
        bandes.append(10 * np.log10(p + 1e-20))
    bandes = np.array(bandes)
    bandes -= bandes[24]  # 1 kHz
    return [{"f": round(f, 1), "db": round(float(d), 2)} for f, d in zip(BANDES, bandes)], int(garde.sum())


def lisser(courbe: np.ndarray) -> np.ndarray:
    """Lissage triangulaire sur ±2 bandes de 1/6 d'octave (largeur 2/3 d'octave, −3 dB à 1/3)."""
    noyau = np.array([1, 2, 3, 2, 1], dtype=float)
    noyau /= noyau.sum()
    etendue = np.pad(courbe, 2, mode="edge")
    return np.convolve(etendue, noyau, mode="valid")


def cible(source: list, reference: list, intensite: float) -> np.ndarray:
    s = np.array([b["db"] for b in source])
    r = np.array([b["db"] for b in reference])
    d = lisser(r - s)
    for i, f in enumerate(BANDES):
        if f < PLANCHER:
            d[i] = 0.0
        elif f > PLAFOND:
            d[i] *= max(0.0, 1 - (np.log2(f / PLAFOND) / np.log2(16000 / PLAFOND)))
    d = np.clip(d, -GAIN_MAX, GAIN_MAX) * float(np.clip(intensite, 0.0, 1.0))
    return d - d[24]  # la bande 1 kHz reste à 0 : pas de changement de niveau


# ---------------------------------------------------------------- biquads (RBJ, comme ffmpeg et son-dsp.js)

def coefficients(genre: str, f: float, largeur: float, gain: float) -> tuple:
    A = 10 ** (gain / 40)
    w0 = 2 * np.pi * f / SR
    alpha = np.sin(w0) / (2 * largeur)
    c = np.cos(w0)
    if genre == "equalizer":
        a0, a1, a2 = 1 + alpha / A, -2 * c, 1 - alpha / A
        b0, b1, b2 = 1 + alpha * A, -2 * c, 1 - alpha * A
    else:
        beta = np.sqrt((A * A + 1) - (A - 1) * (A - 1))
        if genre == "bass":
            a0, a1, a2 = (A + 1) + (A - 1) * c + beta * alpha, -2 * ((A - 1) + (A + 1) * c), (A + 1) + (A - 1) * c - beta * alpha
            b0, b1, b2 = A * ((A + 1) - (A - 1) * c + beta * alpha), 2 * A * ((A - 1) - (A + 1) * c), A * ((A + 1) - (A - 1) * c - beta * alpha)
        else:  # treble
            a0, a1, a2 = (A + 1) - (A - 1) * c + beta * alpha, 2 * ((A - 1) - (A + 1) * c), (A + 1) - (A - 1) * c - beta * alpha
            b0, b1, b2 = A * ((A + 1) + (A - 1) * c + beta * alpha), -2 * A * ((A - 1) + (A + 1) * c), A * ((A + 1) + (A - 1) * c - beta * alpha)
    return b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0


def reponse_db(genre: str, f: float, largeur: float, gain: float, freqs: np.ndarray) -> np.ndarray:
    b0, b1, b2, a1, a2 = coefficients(genre, f, largeur, gain)
    z = np.exp(-1j * 2 * np.pi * freqs / SR)
    h = (b0 + b1 * z + b2 * z * z) / (1 + a1 * z + a2 * z * z)
    return 20 * np.log10(np.abs(h) + 1e-12)


FILTRES = [("bass", ETAGERE_BASSE, 0.7)] + [("equalizer", float(c), Q) for c in CENTRES] + [("treble", ETAGERE_HAUTE, 0.7)]


def traduire(d: np.ndarray) -> tuple[list, np.ndarray, float]:
    """Gains des filtres qui approchent la courbe `d` (dB par bande), par moindres carrés itérés."""
    freqs = np.array(BANDES)
    poids = np.where((freqs >= PLANCHER) & (freqs <= 16000), 1.0, 0.1)
    gains = np.zeros(len(FILTRES))
    for _ in range(4):
        obtenue = sum(reponse_db(g, f, w, gains[i], freqs) for i, (g, f, w) in enumerate(FILTRES))
        jac = np.stack([reponse_db(g, f, w, 1.0, freqs) for (g, f, w) in FILTRES], axis=1)
        reste = (d - obtenue) * poids
        delta, *_ = np.linalg.lstsq(jac * poids[:, None], reste, rcond=None)
        gains = np.clip(gains + delta, -GAIN_MAX, GAIN_MAX)
    obtenue = sum(reponse_db(g, f, w, gains[i], freqs) for i, (g, f, w) in enumerate(FILTRES))
    erreur = float(np.sqrt(np.mean(((d - obtenue) * (poids > 0.5)) ** 2)))
    filtres = []
    for (g, f, w), gain in zip(FILTRES, gains):
        if abs(gain) < 0.15:
            continue
        if g == "equalizer":
            filtres.append(f"equalizer=f={f:.0f}:t=q:w={w}:g={gain:.2f}")
        else:
            filtres.append(f"{g}=g={gain:.2f}:f={f:.0f}:w={w}")
    return filtres, obtenue, erreur


# ---------------------------------------------------------------- commandes

def commande_spectre(args: list) -> None:
    son, sortie = args[0], args[1]
    mots, debut = None, 0.0
    if len(args) >= 4:
        mots = json.loads(Path(args[2]).read_text()).get("mots")
        debut = float(args[3])
    bandes, n = spectre(lire(son), mots, debut)
    Path(sortie).write_text(json.dumps({"bandes": bandes, "fenetres": n, "source": Path(son).name}, ensure_ascii=False))
    print(json.dumps({"fenetres": n, "sortie": sortie}), flush=True)


def commande_courbe(args: list) -> None:
    src = json.loads(Path(args[0]).read_text())["bandes"]
    ref = json.loads(Path(args[1]).read_text())["bandes"]
    intensite = float(args[2])
    d = cible(src, ref, intensite)
    filtres, obtenue, erreur = traduire(d)
    resultat = {"filtres": filtres, "cible": [round(float(x), 2) for x in d], "obtenue": [round(float(x), 2) for x in obtenue],
                "erreur_db": round(erreur, 2), "intensite": intensite}
    Path(args[3]).write_text(json.dumps(resultat, ensure_ascii=False))
    print(json.dumps({"filtres": len(filtres), "erreur_db": round(erreur, 2)}), flush=True)


def commande_essai() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        t = Path(tmp)
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", "anoisesrc=d=20:c=pink:a=0.3:r=48000:seed=7",
                        "-af", "lowpass=f=9000", str(t / "ref.wav")], check=True)
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(t / "ref.wav"), "-af",
                        "equalizer=f=300:t=q:w=1.4:g=-6,equalizer=f=4000:t=q:w=1.4:g=4", str(t / "abime.wav")], check=True)
        ref, _ = spectre(lire(str(t / "ref.wav")))
        src, _ = spectre(lire(str(t / "abime.wav")))
        filtres, _, erreur = traduire(cible(src, ref, 1.0))
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(t / "abime.wav"), "-af", ",".join(filtres), str(t / "corrige.wav")], check=True)
        cor, _ = spectre(lire(str(t / "corrige.wav")))
        ecart = np.array([c["db"] - r["db"] for c, r in zip(cor, ref)])
        sel = [(PLANCHER <= f <= 8000) for f in BANDES]
        pire = float(np.max(np.abs(ecart[sel])))
        print(json.dumps({"filtres": filtres, "erreur_ajustement_db": round(erreur, 2), "ecart_max_apres_db": round(pire, 2)}, ensure_ascii=False))
        # Le lissage à 1/3 d'octave arrondit volontairement un creux étroit : 1,5 dB de tolérance autour de lui.
        if pire > 1.5:
            raise SystemExit(f"écart résiduel {pire:.2f} dB > 1,5 dB")


def main() -> None:
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    commande, args = sys.argv[1], sys.argv[2:]
    if commande == "spectre":
        commande_spectre(args)
    elif commande == "courbe":
        commande_courbe(args)
    elif commande == "essai":
        commande_essai()
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
