#!/usr/bin/env bash
# Lance une commande sous bail de carte graphique d'azothd, et rend le bail à la fin (même en cas d'échec).
# Usage : outils/intro/sous_bail.sh <vram_mo> <duree_estimee_s> <commande…>
set -euo pipefail
AZOTHD="${EDIVID_AZOTHD:-http://127.0.0.1:8016}"
VRAM="$1"; DUREE="$2"; shift 2
while :; do
  R=$(curl -s -X POST "$AZOTHD/api/v1/admission/requests" -H 'Content-Type: application/json' \
      -d "{\"charge\":\"other:EDIVID\",\"kind\":\"other:intro\",\"vram_mb\":$VRAM,\"ram_mb\":16000,\"latency_class\":\"batch\",\"estimated_duration_s\":$DUREE,\"preemptible\":false}")
  BAIL=$(python3 -c 'import json,sys; d=json.loads(sys.argv[1]); print(d["lease"]["id"] if d.get("outcome")=="granted" else "")' "$R")
  [ -n "$BAIL" ] && break
  echo "carte refusée : $R" >&2
  sleep 15
done
echo "bail $BAIL ($VRAM Mo)" >&2
# Registre des baux ouverts (EDIVID_BAUX) : un arrêt brutal (systemctl stop) peut empêcher de rendre le bail ;
# le prochain lancement les rend (voir fabriquer_lot.sh).
[ -n "${EDIVID_BAUX:-}" ] && echo "$BAIL" >> "$EDIVID_BAUX"
rendre() {
  curl -s -X POST "$AZOTHD/api/v1/admission/leases/$BAIL/release" >/dev/null || true
  [ -n "${EDIVID_BAUX:-}" ] && sed -i "/^$BAIL\$/d" "$EDIVID_BAUX"
  echo "bail $BAIL rendu" >&2
}
trap rendre EXIT
"$@"
