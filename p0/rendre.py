"""EDIVID P0 — rendu : liste de coupes → plans recadrés → MP4 sous-titré.

Usage : python rendre.py <analyse.json> <montage.json> <sortie.mp4>
"""
import json
import re
import statistics
import subprocess
import sys
import tempfile
import unicodedata
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ICI = Path(__file__).resolve().parent
POLICES = ICI.parent / "modeles" / "polices"
L_SORTIE, H_SORTIE, IPS = 1080, 1920, 30
HESITATIONS = {"euh", "heu", "hum", "hmm", "euhm", "bah"}
MARGE = 0.06  # air gardé autour des mots à chaque coupe
# Silence interne toléré avant une coupe, selon le régime.
SILENCE_MAX = {"rafale": 0.15, "impact": 0.2, "respiration": 0.9}
# Zoom (par rapport à l'image entière) et hauteur du centre du visage dans le cadre (0 = haut).
CADRES = {"large": (1.0, None), "moyen": (1.4, 0.36), "visage": (2.6, 0.42), "yeux": (3.6, 0.48)}
ZOOM_MAX = 4.0


def normaliser(mot: str) -> str:
    mot = unicodedata.normalize("NFKD", mot.lower())
    return re.sub(r"[^a-z0-9']", "", "".join(c for c in mot if not unicodedata.combining(c)))


def intervalles(phrase: dict, regime: str, serre: bool) -> list[list[dict]]:
    """Découpe une phrase en séries de mots contiguës ; chaque série deviendra un plan."""
    seuil = SILENCE_MAX["rafale"] if serre else SILENCE_MAX[regime]
    series, courant = [], []
    for m in phrase["mots"]:
        if normaliser(m["mot"]) in HESITATIONS:
            if courant:
                series.append(courant)
            courant = []
            continue
        if courant and m["t0"] - courant[-1]["t1"] > seuil:
            series.append(courant)
            courant = []
        courant.append(m)
    if courant:
        series.append(courant)
    return series


def centre_visage(visages: list[dict], t0: float, t1: float, cle: str) -> tuple[float, float] | None:
    dedans = [v[cle] for v in visages if t0 <= v["t"] <= t1] or \
             [min(visages, key=lambda v: abs(v["t"] - (t0 + t1) / 2))[cle]] if visages else []
    if not dedans:
        return None
    return (statistics.median((b[0] + b[2]) / 2 for b in dedans), statistics.median((b[1] + b[3]) / 2 for b in dedans))


def cadrage(cadre: str, centre: tuple[float, float] | None) -> tuple[float, float, float]:
    """Renvoie (zoom, cx, cy) : centre du cadre en coordonnées normalisées, borné à l'image."""
    zoom, hauteur = CADRES[cadre]
    zoom = min(zoom, ZOOM_MAX)
    if zoom == 1.0 or centre is None:
        return 1.0, 0.5, 0.5
    demi = 0.5 / zoom
    cx = min(max(centre[0], demi), 1 - demi)
    cy = min(max(centre[1] + (0.5 - hauteur) / zoom, demi), 1 - demi)
    return zoom, cx, cy


def plans(analyse: dict, montage: dict) -> list[dict]:
    source = {p["id"]: p for p in montage["phrases_source"]}
    decisions = {d["id"]: d for d in montage["montage"]["phrases"]}
    chapitres = {c["phrase"]: c["titre"] for c in montage["montage"].get("chapitres", [])}
    vis = analyse["visages"]
    sortie: list[dict] = []

    def ajouter(pid: int, d: dict, serre: bool, transition: str | None, titre: str | None) -> None:
        series = intervalles(source[pid], d["regime"], serre)
        for k, serie in enumerate(series):
            t0, t1 = max(0.0, serie[0]["t0"] - MARGE), serie[-1]["t1"] + MARGE
            cadre = d["cadre"]
            cle = "yeux" if cadre == "yeux" else "visage"
            zoom, cx, cy = cadrage(cadre, centre_visage(vis, t0, t1, cle))
            if d["regime"] == "rafale" and k % 2 == 1:
                zoom = min(zoom * 1.12, ZOOM_MAX)  # léger punch-in qui masque le jump cut interne
                zoom, cx, cy = cadrage_zoom(zoom, cx, cy)
            anime = {"impact": 1.12, "respiration": 1.05}.get(d["regime"], 1.0) if not serre or d["regime"] == "impact" else 1.0
            sortie.append({"phrase": pid, "t0": t0, "t1": t1, "mots": serie, "cadre": cadre, "regime": d["regime"],
                           "zoom": zoom, "zoom_fin": min(zoom * anime, ZOOM_MAX), "cx": cx, "cy": cy,
                           "transition": transition if k == 0 else None, "titre": titre if k == 0 else None,
                           "mots_forts": {normaliser(w) for w in d.get("mots_forts", [])}})

    for pid in montage["montage"].get("accroche", []):
        if pid in source:
            d = dict(decisions.get(pid, {}), regime="impact")
            d["cadre"] = d.get("cadre") if d.get("cadre") in ("visage", "yeux") else "visage"
            ajouter(pid, d, True, None, None)
    debut = True
    for pid in sorted(source):
        d = decisions[pid]
        if not d["garder"]:
            continue
        transition = "flash" if (debut and sortie) or d.get("transition") == "flash" else None
        ajouter(pid, d, False, transition, chapitres.get(pid))
        debut = False
    return sortie


def cadrage_zoom(zoom: float, cx: float, cy: float) -> tuple[float, float, float]:
    demi = 0.5 / zoom
    return zoom, min(max(cx, demi), 1 - demi), min(max(cy, demi), 1 - demi)


def rendre_plan(video: str, p: dict, chemin: str) -> None:
    duree = round((p["t1"] - p["t0"]) * IPS) / IPS
    p["duree"] = duree
    n = max(1, round(duree * IPS))
    if p["zoom_fin"] != p["zoom"]:
        # Zoom animé (ease-out), sur une image préagrandie pour éviter les saccades.
        f = 3
        z = f"{p['zoom']}+({p['zoom_fin']}-{p['zoom']})*(1-pow(1-min(on/{n},1),2))"
        vf = (f"fps={IPS},scale=iw*{f}:ih*{f}:flags=lanczos,"
              f"zoompan=z='{z}':x='max(0,min(iw-iw/zoom,{p['cx']}*iw-iw/zoom/2))':"
              f"y='max(0,min(ih-ih/zoom,{p['cy']}*ih-ih/zoom/2))':d=1:s={L_SORTIE}x{H_SORTIE}:fps={IPS}")
    else:
        w = f"iw/{p['zoom']}"
        h = f"ih/{p['zoom']}"
        vf = (f"fps={IPS},crop=w='{w}':h='{h}':x='{p['cx']}*iw-{w}/2':y='{p['cy']}*ih-{h}/2',"
              f"scale={L_SORTIE}:{H_SORTIE}:flags=lanczos")
    if p["zoom"] > 1.5:
        vf += ",unsharp=5:5:0.6"
    if p["transition"] == "flash":
        vf += ",fade=t=in:st=0:d=0.15:color=white"
    vf += ",setsar=1,format=yuv420p"
    af = f"aresample=48000,afade=t=in:d=0.01,afade=t=out:st={max(0, duree - 0.01):.3f}:d=0.01"
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-ss", f"{p['t0']:.3f}", "-i", video, "-t", f"{duree:.3f}",
                    "-vf", vf, "-af", af, "-frames:v", str(n), "-c:v", "libx264", "-preset", "veryfast", "-crf", "14",
                    "-c:a", "pcm_s16le", "-ac", "2", chemin], check=True)


def temps_ass(t: float) -> str:
    return f"{int(t // 3600)}:{int(t % 3600 // 60):02d}:{t % 60:05.2f}"


def sous_titres(ps: list[dict], chemin: str) -> None:
    lignes = ["[Script Info]", "ScriptType: v4.00+", f"PlayResX: {L_SORTIE}", f"PlayResY: {H_SORTIE}", "",
              "[V4+ Styles]",
              "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, "
              "Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, "
              "Alignment, MarginL, MarginR, MarginV, Encoding",
              "Style: Mot,Anton,104,&H00FFFFFF,&H00FFFFFF,&H00000000,&H64000000,0,0,0,0,100,100,1,0,1,7,3,2,60,60,400,1",
              "Style: Titre,Anton,92,&H00000000,&H00000000,&H0000E5FF,&H0000E5FF,0,0,0,0,100,100,2,0,3,22,0,8,60,60,230,1",
              "", "[Events]", "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text"]
    # Mots repositionnés sur le temps de sortie.
    mots, debut = [], 0.0
    for p in ps:
        if p["titre"]:
            lignes.append(f"Dialogue: 1,{temps_ass(debut + 0.1)},{temps_ass(debut + 1.9)},Titre,,0,0,0,,"
                          f"{{\\fad(120,200)}}{p['titre'].upper()}")
        for m in p["mots"]:
            mots.append({"t0": debut + m["t0"] - p["t0"], "t1": debut + m["t1"] - p["t0"], "mot": m["mot"],
                         "fort": normaliser(m["mot"]) in p["mots_forts"]})
        debut += p["duree"]
    # Whisper coupe « c'est » en « c » + « 'est » et « bien-être » en « bien » + « -être » : on recolle.
    recolles = []
    for m in mots:
        if recolles and m["mot"].startswith(("'", "-")):
            recolles[-1] = dict(recolles[-1], mot=recolles[-1]["mot"] + m["mot"], t1=m["t1"],
                                fort=recolles[-1]["fort"] or m["fort"])
        else:
            recolles.append(m)
    mots = recolles
    groupes, courant = [], []
    for m in mots:
        if courant and (len(courant) >= 3 or len(" ".join(x["mot"] for x in courant + [m])) > 16
                        or m["t0"] - courant[-1]["t1"] > 0.3 or courant[-1]["mot"].endswith((".", "?", "!", ","))):
            groupes.append(courant)
            courant = []
        courant.append(m)
    if courant:
        groupes.append(courant)
    for i, g in enumerate(groupes):
        fin = min(groupes[i + 1][0]["t0"], g[-1]["t1"] + 0.4) if i + 1 < len(groupes) else g[-1]["t1"] + 0.4
        texte = " ".join(("{\\c&H0000E5FF&\\fscx115\\fscy115}" + re.sub(r"[.,!?]$", "", m["mot"]).upper() + "{\\r}")
                         if m["fort"] else re.sub(r"[.,]$", "", m["mot"]).upper() for m in g)
        lignes.append(f"Dialogue: 0,{temps_ass(g[0]['t0'])},{temps_ass(fin)},Mot,,0,0,0,,"
                      f"{{\\fscx85\\fscy85\\t(0,90,\\fscx100\\fscy100)}}{texte}")
    Path(chemin).write_text("\n".join(lignes) + "\n", encoding="utf-8")


def main() -> None:
    analyse = json.loads(Path(sys.argv[1]).read_text())
    montage = json.loads(Path(sys.argv[2]).read_text())
    sortie = Path(sys.argv[3]).resolve()
    ps = plans(analyse, montage)
    print(f"{len(ps)} plans à rendre…", flush=True)
    with tempfile.TemporaryDirectory() as tmp:
        chemins = [f"{tmp}/plan{i:04d}.mkv" for i in range(len(ps))]
        with ThreadPoolExecutor(6) as pool:
            list(pool.map(lambda a: rendre_plan(analyse["video"], *a), zip(ps, chemins)))
        Path(f"{tmp}/liste.txt").write_text("".join(f"file '{c}'\n" for c in chemins))
        sous_titres(ps, f"{tmp}/sous.ass")
        print("Assemblage, sous-titres, volume…", flush=True)
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "concat", "-safe", "0", "-i", f"{tmp}/liste.txt",
                        "-vf", f"subtitles={tmp}/sous.ass:fontsdir={POLICES}",
                        "-af", "loudnorm=I=-14:TP=-1.5:LRA=11", "-ar", "48000",
                        "-c:v", "libx264", "-preset", "medium", "-crf", "19",
                        "-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart", str(sortie)], check=True)
    duree = sum(p["duree"] for p in ps)
    compte = lambda cle: {k: sum(1 for p in ps if p[cle] == k) for k in sorted({p[cle] for p in ps})}
    rapport = {"duree_source": round(analyse["duree"], 1), "duree_montage": round(duree, 1), "plans": len(ps),
               "duree_moyenne_plan": round(duree / len(ps), 2), "cadres": compte("cadre"), "regimes": compte("regime"),
               "zooms_animes": sum(1 for p in ps if p["zoom_fin"] != p["zoom"]),
               "transitions": sum(1 for p in ps if p["transition"])}
    sortie.with_suffix(".rapport.json").write_text(json.dumps(rapport, ensure_ascii=False, indent=1))
    print(json.dumps(rapport, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
