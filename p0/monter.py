"""EDIVID P0 — le monteur : un seul LLM décide la liste de coupes, puis la critique et la révise.

Usage : python monter.py <analyse.json> <montage.json>
"""
import json
import sys
from pathlib import Path

import requests

MODELE = "qwen3.5:35b-a3b-q4_K_M"
OLLAMA = "http://localhost:11434/api/chat"
PAUSE_PHRASE = 0.45  # un silence plus long que ça ouvre une nouvelle phrase

CONSIGNE_MONTEUR = """Tu es monteur vidéo pour YouTube. Tu montes une vidéo face caméra (une personne qui parle seule, plan fixe vertical) pour la rendre rythmée et captivante, comme les meilleurs créateurs : coupes sèches, zooms, variations de rythme.

Tu reçois la transcription découpée en phrases numérotées. Chaque phrase porte : début et fin en secondes, silence qui la précède, énergie de la voix (dB par rapport à la moyenne de la vidéo ; positif = voix plus forte, plus engagée).

Tu décides, phrase par phrase :
- garder : true ou false. Coupe les répétitions, faux départs, phrases ratées ou reprises, digressions sans valeur, formules creuses. Garde tout ce qui porte le sens, l'émotion ou la personnalité. Vise à retirer environ 20 à 30 % de la durée parlée, jamais une idée essentielle.
- regime :
  * "respiration" : moment fort qui mérite de l'ampleur — récit personnel, émotion, confidence, idée centrale énoncée lentement. Plan plus large, on laisse vivre les silences, pas de coupe interne.
  * "rafale" : passages informatifs, énumérations, listes, enchaînements rapides. Jump cuts serrés, silences supprimés, cadres qui alternent à chaque phrase.
  * "impact" : phrase choc, formule qui résume, chute, punchline, chiffre marquant. Zoom serré, coupe sèche avant et après.
  La majorité des phrases sont en "rafale" ; "respiration" et "impact" sont des événements, pas la règle (chacun environ 10 à 20 % des phrases).
- cadre : "large" (image entière), "moyen" (punch-in léger sur le buste et le visage), "visage" (gros plan visage), "yeux" (très gros plan sur le regard — rare, réservé aux deux ou trois phrases les plus intenses de toute la vidéo).
  Règles : en "rafale", ne jamais garder le même cadre deux phrases de suite (alterne large / moyen / visage). "impact" prend "visage" ou "yeux". "respiration" prend "large" ou "moyen".
- transition : "flash" uniquement au début d'une nouvelle partie de la vidéo (changement de sujet net), sinon null. Trois à six transitions au total.
- mots_forts : un à trois mots de la phrase à mettre en valeur dans les sous-titres (mots porteurs de sens, pas des mots outils), ou liste vide.

Tu choisis aussi :
- accroche : une à trois phrases (numéros) parmi les plus fortes de toute la vidéo, qui seront montées en ouverture, avant le début, pour accrocher le spectateur dans les 5 premières secondes. Durée totale visée : moins de 8 secondes.
- chapitres : les grandes parties de la vidéo, avec le numéro de la phrase où chacune commence et un titre court (2 à 5 mots) qui s'affichera à l'écran.

Réponds uniquement en JSON."""

CONSIGNE_CRITIQUE = """Tu es chef monteur. Un monteur a proposé le montage ci-dessous pour une vidéo YouTube face caméra. Tu le relis comme un spectateur exigeant qui décroche à la moindre monotonie, puis tu rends la version corrigée complète.

Vérifie :
- l'accroche est-elle vraiment la plus forte possible, et compréhensible sans contexte ?
- y a-t-il des tunnels : plus de 4 phrases de suite au même régime, ou plus de 25 secondes sans changement de cadre ? Casse-les.
- en "rafale", deux phrases consécutives ont-elles le même cadre ? Corrige.
- les "impact" sont-ils de vraies formules choc ? Les "yeux" sont-ils rares (trois au plus) et mérités ?
- les "respiration" tombent-elles sur les vrais moments d'émotion ou de récit ?
- a-t-on coupé une idée essentielle, ou gardé des répétitions et faux départs ?
- les chapitres découpent-ils bien la vidéo ?

Rends le montage corrigé complet, au même format JSON, avec en plus un champ "notes" : trois à six phrases courtes qui disent ce que tu as changé et pourquoi."""

SCHEMA = {
    "type": "object",
    "properties": {
        "accroche": {"type": "array", "items": {"type": "integer"}},
        "chapitres": {"type": "array", "items": {"type": "object", "properties": {
            "phrase": {"type": "integer"}, "titre": {"type": "string"}}, "required": ["phrase", "titre"]}},
        "phrases": {"type": "array", "items": {"type": "object", "properties": {
            "id": {"type": "integer"},
            "garder": {"type": "boolean"},
            "regime": {"type": "string", "enum": ["respiration", "rafale", "impact"]},
            "cadre": {"type": "string", "enum": ["large", "moyen", "visage", "yeux"]},
            "transition": {"anyOf": [{"type": "string", "enum": ["flash"]}, {"type": "null"}]},
            "mots_forts": {"type": "array", "items": {"type": "string"}}},
            "required": ["id", "garder", "regime", "cadre", "transition", "mots_forts"]}},
        "notes": {"type": "array", "items": {"type": "string"}},
    },
    "required": ["accroche", "chapitres", "phrases"],
}


def phrases(mots: list[dict]) -> list[dict]:
    """Regroupe les mots en phrases, sur la ponctuation finale ou un silence net."""
    res, courant = [], []
    for i, m in enumerate(mots):
        if courant and m["t0"] - courant[-1]["t1"] > PAUSE_PHRASE:
            res.append(courant)
            courant = []
        courant.append(m)
        if m["mot"].endswith((".", "?", "!")):
            res.append(courant)
            courant = []
    if courant:
        res.append(courant)
    sortie = []
    for i, ms in enumerate(res):
        avant = ms[0]["t0"] - res[i - 1][-1]["t1"] if i else ms[0]["t0"]
        sortie.append({"id": i, "t0": ms[0]["t0"], "t1": ms[-1]["t1"], "silence_avant": round(avant, 2),
                       "db": round(max(m["db"] for m in ms), 1), "texte": " ".join(m["mot"] for m in ms),
                       "mots": ms})
    return sortie


def appeler(consigne: str, contenu: str) -> dict:
    r = requests.post(OLLAMA, json={
        "model": MODELE, "stream": False, "think": True, "format": SCHEMA,
        "options": {"num_ctx": 32768, "temperature": 0.4},
        "messages": [{"role": "system", "content": consigne}, {"role": "user", "content": contenu}]}, timeout=3600)
    r.raise_for_status()
    d = r.json()
    print(f"  {d['eval_count']} jetons en {d['eval_duration'] / 1e9:.0f} s", flush=True)
    return json.loads(d["message"]["content"])


def completer(montage: dict, ps: list[dict]) -> dict:
    """Le LLM peut oublier des phrases : on les garde en rafale large plutôt que de les perdre."""
    vus = {p["id"] for p in montage["phrases"]}
    for p in ps:
        if p["id"] not in vus:
            montage["phrases"].append({"id": p["id"], "garder": True, "regime": "rafale", "cadre": "moyen",
                                       "transition": None, "mots_forts": [], "oublie": True})
    montage["phrases"].sort(key=lambda p: p["id"])
    return montage


def main() -> None:
    analyse = json.loads(Path(sys.argv[1]).read_text())
    ps = phrases(analyse["mots"])
    transcription = "\n".join(
        f"[{p['id']}] {p['t0']:.1f}–{p['t1']:.1f}s | silence {p['silence_avant']:.1f}s | {p['db']:+.0f} dB | {p['texte']}"
        for p in ps)
    print(f"{len(ps)} phrases. Montage…", flush=True)
    premier = completer(appeler(CONSIGNE_MONTEUR, transcription), ps)
    print("Critique…", flush=True)
    revu = completer(appeler(CONSIGNE_CRITIQUE, f"TRANSCRIPTION :\n{transcription}\n\nMONTAGE PROPOSÉ :\n"
                             + json.dumps(premier, ensure_ascii=False)), ps)
    Path(sys.argv[2]).write_text(json.dumps({"phrases_source": [{k: v for k, v in p.items()} for p in ps],
                                             "premier": premier, "montage": revu}, ensure_ascii=False, indent=1))
    for n in revu.get("notes", []):
        print("  •", n)


if __name__ == "__main__":
    main()
