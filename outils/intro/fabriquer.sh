#!/usr/bin/env bash
# Fabrique l'intro de bout en bout, sans surveillance (lancé par systemd-run : survit à la fin de la session).
# 1. animations verticales manquantes (les horizontales sont faites) ; 2. jingles pop ; 3. déclinaisons ;
# 4. vidéos finales des trois variantes, plus l'intro 1080p de la première variante avec chaque jingle.
# Journal : <dossier>/fabriquer.log ; résultat décrit dans <dossier>/RESULTATS.md.
set -uo pipefail
cd "$(dirname "$0")/../.."
I=essais/2026-10-27-intro
PY=.venv-video/bin/python
export HF_HUB_OFFLINE=1 PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True
GRAINES=(21 22 23)
BPM=124
LOGO=$I/sources/logo_by_edina.png

echo "== début $(date -Is)"
# 1. Verticales.
manque=()
for g in "${GRAINES[@]}"; do [ -s "$I/anim/v2_g$g.mp4" ] || manque+=("$g"); done
if [ ${#manque[@]} -gt 0 ]; then
  liste=$(IFS=,; echo "${manque[*]}")
  echo "== verticales : $liste"
  outils/intro/sous_bail.sh 13500 1800 $PY outils/intro/anime_wan.py $I/depart/depart_vertical.png $I/anim/v2.mp4 \
    $I/consigne_vertical.txt "$liste" 2>&1 | grep -v -E '^\s*[0-9]+%|it/s|s/it' || echo "!! verticales en échec"
  # Une seule graine demandée : le fichier n'a pas de suffixe.
  [ ${#manque[@]} -eq 1 ] && [ -s $I/anim/v2.mp4 ] && mv $I/anim/v2.mp4 "$I/anim/v2_g${manque[0]}.mp4"
fi

# 2. Jingles : trois styles, deux tirages chacun, 30 s (pour les déclinaisons longues).
declare -A STYLES=(
  [A_pop_solaire]="Upbeat feel-good pop, sunny summer anthem, joyful and energetic, motivational fitness vibe, punchy four-on-the-floor drums, handclaps, groovy bass, bright piano chords, plucky electric guitar, uplifting synth lead hook, instrumental, starts immediately with full energy"
  [B_tropical]="Happy energetic pop with a tropical summer feel, bright and optimistic, wellness and happiness, upbeat claps and finger snaps, bouncy bass, marimba plucks, whistled catchy melody, acoustic guitar strums, instrumental, strong catchy hook from the first bar"
  [C_estate_italiana]="Energetic Italian summer pop, joyful and sunny, dance-pop beat, punchy drums and claps, funky bass, bright mandolin riff, accordion accents, shiny synths, instrumental, catchy hook from the first bar"
)
for s in "${!STYLES[@]}"; do
  if ! ls $I/jingles/$s/jingle_*.wav >/dev/null 2>&1; then
    echo "== jingle $s"
    outils/intro/sous_bail.sh 10000 600 $PY outils/intro/jingle.py $I/jingles/$s "${STYLES[$s]}" 2 30 $BPM "D major" 2>&1 \
      | grep -v -E 'it/s|s/it' || echo "!! jingle $s en échec"
  fi
done

# 3. Déclinaisons de chaque morceau.
for w in $I/jingles/*/jingle_*.wav; do
  d=$I/jingles/$(basename "$(dirname "$w")")/$(basename "$w" .wav)
  [ -s "$d/jingle_3s.wav" ] || $PY outils/intro/declinaisons.py "$w" $BPM "$d"
done

# 4. Vidéos. Jingle par défaut : le premier du style A (à remplacer après écoute).
DEF=$(ls -d $I/jingles/A_pop_solaire/jingle_*/ 2>/dev/null | head -1)
DEF=${DEF%/}
if [ -n "$DEF" ]; then
  for g in "${GRAINES[@]}"; do
    [ -s "$I/anim/h2_g$g.mp4" ] && [ -s "$I/anim/v2_g$g.mp4" ] || continue
    echo "== variante $g"
    $PY outils/intro/composer.py $I/anim/h2_g$g.mp4 $I/anim/v2_g$g.mp4 "$DEF/intro.wav" $LOGO $I/rendus/variante_$g
  done
  # Chaque jingle sur la variante 21 en 1080p, pour les comparer en situation.
  for d in $I/jingles/*/jingle_*/; do
    d=${d%/}
    nom=$(basename "$(dirname "$d")")_$(basename "$d")
    $PY - "$I/anim/h2_g21.mp4" "$d/intro.wav" "$LOGO" "$I/rendus/comparer_jingles/$nom.mp4" <<'EOF'
import sys
from pathlib import Path
from PIL import Image
sys.path.insert(0, "outils/intro")
import composer
video, son, logo, sortie = sys.argv[1:5]
Path(sortie).parent.mkdir(parents=True, exist_ok=True)
composer.composer(Path(video), Path(son), Image.open(logo).convert("RGBA"), 1920, 1080, Path(sortie))
EOF
  done
fi

# Résultats.
{
  echo "# Intro « Être heureux et en forme après 50 ans » — résultats du $(date '+%d/%m/%Y %H:%M')"
  echo
  echo "## Vidéos finales (jingle par défaut : $(basename "$(dirname "$DEF")")/$(basename "$DEF"))"
  for f in $I/rendus/variante_*/intro_*.mp4; do [ -e "$f" ] && echo "- ${f#$I/}"; done
  echo
  echo "## Comparer les jingles (variante 21, 1080p)"
  for f in $I/rendus/comparer_jingles/*.mp4; do [ -e "$f" ] && echo "- ${f#$I/}"; done
  echo
  echo "## Jingles et déclinaisons (MP3 pour l'écoute)"
  for d in $I/jingles/*/jingle_*/; do echo "- ${d#$I/} : jingle_3s, transition_8s, fin_16s, intro, final"; done
} > $I/RESULTATS.md
echo "== fin $(date -Is)"
