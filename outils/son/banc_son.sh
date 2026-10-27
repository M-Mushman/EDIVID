#!/usr/bin/env bash
# Banc du son : les candidats de déréverbération et d'égalisation sur des extraits d'un rush, notés par DNSMOS.
#
#   outils/son/banc_son.sh <dossier du banc> <mezz.mkv> <analyse.json> "<nom>:<début s>" ["<nom>:<début s>"…]
#
# Produit <banc>/extraits/*.wav (30 s), <banc>/candidats/<extrait>__<candidat>.wav, <banc>/spectres, <banc>/courbes,
# <banc>/notes.json et <banc>/tableau.md. Les capteurs qui ont besoin de la carte graphique passent par le test
# `capteur_sous_bail` (bail azothd, comme en service). Relancer le script ne refait que ce qui manque.
set -euo pipefail
RACINE="$(cd "$(dirname "$0")/../.." && pwd)"
BANC="$1"; MEZZ="$2"; ANALYSE="$3"; shift 3
mkdir -p "$BANC/extraits" "$BANC/candidats" "$BANC/spectres" "$BANC/courbes"
cd "$RACINE"

sous_bail() { # <script> <args séparés par |>
  EDIVID_SCRIPT="$1" EDIVID_ARGS="$2" cargo test --release capteur_sous_bail -- --ignored --nocapture 2>&1 | grep -E "vram|panicked|Error" | head -1
}

for x in "$@"; do
  n="${x%%:*}"; t="${x##*:}"
  [ -e "$BANC/extraits/$n.wav" ] || ffmpeg -v error -y -ss "$t" -t 30 -i "$MEZZ" -vn -ac 2 -ar 48000 -c:a pcm_s16le "$BANC/extraits/$n.wav"
  E="$BANC/extraits/$n.wav"; C="$BANC/candidats/${n}__"
  # Débruiteur (Resemble), reconstruction (VoiceFixer, référence de timbre et plafond de sécheresse).
  [ -e "${C}resemble.wav" ] || sous_bail rehausse.py "$E|${C}resemble.wav|1.0"
  [ -e "${C}voicefixer.wav" ] || sous_bail restaure.py "${C}resemble.wav|${C}voicefixer.wav"
  # Séparation voix sèche / réverbération (masques : aucun mot inventé).
  while read -r court modele; do
    [ -e "${C}$court.wav" ] || sous_bail separe.py "$E|${C}$court.wav|$modele|1.0|${C}$court.json"
  done <<'EOF'
melroformer dereverb_mel_band_roformer_anvuew_sdr_19.1729.ckpt
melroformer_doux dereverb_mel_band_roformer_less_aggressive_anvuew_sdr_18.8050.ckpt
uvr_deecho UVR-DeEcho-DeReverb.pth
bsroformer deverb_bs_roformer_8_384dim_10depth.ckpt
reverbhq Reverb_HQ_By_FoxJoy.onnx
EOF
  # DeepFilterNet3 (bruit, processeur) seul et après la séparation.
  if [ -x .venv-deepfilter/bin/python ]; then
    [ -e "${C}deepfilter.wav" ] || .venv-deepfilter/bin/python capteurs/debruite.py "$E" "${C}deepfilter.wav"
    [ -e "${C}melroformer+deepfilter.wav" ] && true || { [ -e "${C}melroformer.wav" ] && .venv-deepfilter/bin/python capteurs/debruite.py "${C}melroformer.wav" "${C}melroformer+deepfilter.wav"; }
  fi
  # Spectres et égaliseur par correspondance : brut → « cette voix » (VoiceFixer) et → studio.
  .venv-son/bin/python capteurs/egaliseur.py spectre "$E" "$BANC/spectres/$n.json" "$ANALYSE" "$t" >/dev/null
  .venv-son/bin/python capteurs/egaliseur.py spectre "${C}voicefixer.wav" "$BANC/spectres/${n}__voicefixer.json" "$ANALYSE" "$t" >/dev/null
  for ref in "cettevoix:$BANC/spectres/${n}__voicefixer.json:1.0" "studio60:bibliotheque/references_voix/studio.json:0.6"; do
    IFS=: read -r nom fich inten <<< "$ref"
    .venv-son/bin/python capteurs/egaliseur.py courbe "$BANC/spectres/$n.json" "$fich" "$inten" "$BANC/courbes/${n}__$nom.json" >/dev/null
    F=$(python3 -c "import json;print(','.join(json.load(open('$BANC/courbes/${n}__$nom.json'))['filtres']))")
    [ -e "${C}eq_$nom.wav" ] || ffmpeg -v error -y -i "$E" -af "$F" -c:a pcm_s24le "${C}eq_$nom.wav"
  done
done
python3 outils/son/tableau.py "$BANC" > "$BANC/tableau.md"
cat "$BANC/tableau.md"
