#!/usr/bin/env bash
# Compile EDIVID, installe le binaire et le service utilisateur, puis (re)démarre.
# Le binaire précédent est gardé pour un retour arrière : ~/.local/bin/edivid.avant
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release
[ -e "$HOME/.local/bin/edivid" ] && cp "$HOME/.local/bin/edivid" "$HOME/.local/bin/edivid.avant"
install -m 755 target/release/edivid "$HOME/.local/bin/edivid"
install -D -m 644 deploy/edivid.service "$HOME/.config/systemd/user/edivid.service"
systemctl --user daemon-reload
systemctl --user enable edivid.service >/dev/null
systemctl --user restart edivid.service
for _ in $(seq 1 30); do curl -sf http://127.0.0.1:8030/api/session >/dev/null && break; sleep 1; done
systemctl --user --no-pager status edivid.service | head -5
echo "Code d'accès : $(python3 -c 'import json; print(json.load(open("donnees/acces.json"))["code"])')"
