#!/usr/bin/env bash
# Intro animée pour toute une série de photos, sans surveillance (systemd-run ; reprend là où il s'est arrêté).
# Attend la fin du service edivid-intro (même carte), puis :
# 1. décor prolongé par FLUX.2 klein pour chaque photo (un seul bail) ;
# 2. photo par photo : 2 tirages Wan par format (un bail par photo, la carte est rendue entre deux photos),
#    le visage de la photo recollé sur chaque tirage (visage.py), puis les trois formats assemblés pour chaque
#    tirage, avec le jingle par défaut.
# Journal : <dossier>/lot.log ; résultats : <dossier>/RESULTATS_LOT.md et planches/.
set -uo pipefail
cd "$(dirname "$0")/../.."
I=essais/2026-10-27-intro
PY=.venv-video/bin/python
export HF_HUB_OFFLINE=1 PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True
GRAINES="[31, 32]"
LOGO=$I/sources/logo_by_edina.png
L=$I/lot

echo "== début $(date -Is)"
# Baux laissés ouverts par un arrêt brutal précédent : rendus.
export EDIVID_BAUX=$I/baux_ouverts.txt
touch $EDIVID_BAUX
while read -r b; do
  [ -n "$b" ] && curl -s -X POST "${EDIVID_AZOTHD:-http://127.0.0.1:8016}/api/v1/admission/leases/$b/release" >/dev/null && echo "bail orphelin $b rendu"
done < $EDIVID_BAUX
: > $EDIVID_BAUX
while systemctl --user is-active -q edivid-intro; do sleep 60; done
echo "== edivid-intro terminé $(date -Is)"
mkdir -p $L

# 1. Décors.
outils/intro/sous_bail.sh 13500 3600 $PY outils/intro/etendre_flux.py $I/photos.json $I/sources $L 2>&1 \
  | grep --line-buffered -v -E 'it/s|s/it' || echo "!! décors en échec"

# Jingle par défaut (à remplacer après écoute : recomposer avec composer.py).
DEF=$(ls -d $I/jingles/A_pop_solaire/jingle_*/ 2>/dev/null | head -1); DEF=${DEF%/}
DEBUT=$( [ -n "$DEF" ] && python3 -c "import json;print(json.load(open('$DEF/depart.json'))['debut_s'])" || echo 0)

# 2. Animation puis assemblage, photo par photo.
for nom in $(python3 -c "import json;print(' '.join(json.load(open('$I/photos.json'))))"); do
  [ -s "$L/$nom/depart_horizontal.png" ] && [ -s "$L/$nom/depart_vertical.png" ] || { echo "!! $nom : pas de décor"; continue; }
  python3 - "$I/photos.json" "$nom" "$L" "$GRAINES" > "$L/$nom/travaux.json" <<'EOF'
import json, sys
fiches, nom, L, graines = json.load(open(sys.argv[1])), sys.argv[2], sys.argv[3], json.loads(sys.argv[4])
f = fiches[nom]
regard = {"camera": "She keeps looking straight into the camera lens with a warm, natural smile; her eyes stay on the camera.",
          "profile": "She keeps her face in profile with a gentle smile.",
          "main": "She keeps looking at her open hand with a gentle smile."}[f["regard"]]
decor = {"studio": "in front of a plain light grey studio backdrop", "rue": "in a quiet old Paris street"}[f["fond"]]
# Caméra presque immobile : une avancée franche, sur une pose en pied, la sortait du cadre (essai du 2026-10-28).
consigne = (f"Cinematic fashion photo shoot. The camera is almost static: an extremely slow, barely perceptible push-in, "
            f"the framing stays the same and her whole figure stays in frame. "
            f"A smiling woman with long blond hair, wearing {f['tenue']}, {f['pose']}, holds her pose perfectly still {decor}. "
            f"{regard} A gentle breeze softly lifts and floats strands of her long blond hair. Minimal body movement, only "
            f"natural breathing; no turning, no gestures. Professional lighting, sharp focus, realistic skin, high detail.")
print(json.dumps([{"image": f"{L}/{nom}/depart_{fmt}.png", "sortie": f"{L}/{nom}/anim_{fmt}.mp4", "consigne": consigne,
                   "graines": graines} for fmt in ("horizontal", "vertical")], ensure_ascii=False))
EOF
  manque=0
  for g in 31 32; do for fmt in horizontal vertical; do [ -s "$L/$nom/anim_${fmt}_g$g.mp4" ] || manque=1; done; done
  if [ $manque = 1 ]; then
    echo "== $nom : animation $(date -Is)"
    outils/intro/sous_bail.sh 13500 2400 $PY outils/intro/anime_wan.py --lot "$L/$nom/travaux.json" 2>&1 \
      | grep --line-buffered -E '"graine"|total_s|Error|bail' || echo "!! $nom : animation en échec"
  fi
  # Visage de la photo recollé sur chaque tirage (processeur seul) : Wan déforme les visages de petite taille.
  for g in 31 32; do for fmt in horizontal vertical; do
    a="$L/$nom/anim_${fmt}_g$g.mp4"; r="$L/$nom/visage_${fmt}_g$g.mp4"
    [ -s "$a" ] && ! [ "$r" -nt "$a" ] || continue
    nice $PY outils/intro/visage.py "$a" "$I/sources/$nom.jpg" "$r" 2>&1 | grep -v -E 'WARN|setPreferable' \
      || echo "!! $nom $fmt g$g : visage en échec"
  done; done
  [ -n "$DEF" ] || continue
  for g in 31 32; do
    h="$L/$nom/visage_horizontal_g$g.mp4"; v="$L/$nom/visage_vertical_g$g.mp4"
    [ -s "$h" ] && [ -s "$v" ] || continue
    [ "$L/$nom/rendus_g$g/intro_1080x1920.mp4" -nt "$v" ] && [ "$L/$nom/rendus_g$g/intro_1080x1920.mp4" -nt "$h" ] && continue
    $PY outils/intro/composer.py "$h" "$v" "$DEF.wav" $LOGO "$L/$nom/rendus_g$g" "$DEBUT" || echo "!! $nom g$g : assemblage en échec"
  done
  # Planche : 4 images de chaque tirage, pour choisir d'un coup d'œil.
  mkdir -p $I/planches
  for g in 31 32; do for fmt in horizontal vertical; do
    a="$L/$nom/visage_${fmt}_g$g.mp4"; [ -s "$a" ] || continue
    ffmpeg -v error -y -i "$a" -vf "select='eq(n\,0)+eq(n\,30)+eq(n\,60)+eq(n\,92)',scale=-2:360,tile=4x1" -frames:v 1 \
      "$I/planches/${nom}_${fmt}_g$g.jpg"
  done; done
done

{
  echo "# Intro animée — série de photos ($(date '+%d/%m/%Y %H:%M'))"
  echo
  echo "Jingle par défaut : ${DEF#$I/}. Planches de choix : planches/<photo>_<format>_g<tirage>.jpg."
  echo
  for d in $L/*/; do
    nom=$(basename "$d")
    echo "## $nom"
    for f in "$d"rendus_g*/intro_*.mp4; do [ -e "$f" ] && echo "- ${f#$I/}"; done
  done
} > $I/RESULTATS_LOT.md
echo "== fin $(date -Is)"
