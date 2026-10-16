# EDIVID

Monteur vidéo IA local. On dépose des rushs (vidéo face caméra, fichiers iPhone acceptés tels quels),
un seul LLM (qwen3.5:35b-a3b dans Ollama) propose un montage rythmé — coupes, zooms, accroche,
transitions, sous-titres — puis on l'ajuste en discutant avec lui, dans une interface web unique.

- `src/` : serveur Rust (axum) — file de travaux, découpe et cadrage, rendu ffmpeg, LLM, API.
- `capteurs/` : adaptateurs de modèles en Python — `analyse.py` (Whisper large-v3, enveloppe
  sonore, visage YuNet), `nettete.py` (Real-ESRGAN, GFPGAN).
- `web/` : interface (HTML, CSS, JavaScript sans dépendance).
- `deploy/` : service utilisateur systemd et script d'installation.
- `docs/plan-v0.1.md` : plan technique ; `p0/` : la preuve de concept d'origine.

## Installation

```
uv venv --python 3.12 .venv
uv pip install --python .venv/bin/python faster-whisper opencv-python-headless numpy soundfile \
    nvidia-cublas-cu12 "nvidia-cudnn-cu12==9.*" torch spandrel
# modèles dans modeles/ : yunet.onnx, realesr-general-x4v3.pth, GFPGANv1.4.pth
./deploy/installer.sh
```

Interface : `http://<machine>:8030` (localhost et adresse du tailnet). Le code d'accès est tiré au
premier lancement dans `donnees/acces.json`.

Avant tout travail sur la carte graphique, EDIVID prend un bail auprès du contrôleur d'admission
d'azothd (`other:EDIVID`) et le rend ensuite.

Police Anton : SIL Open Font License (`modeles/polices/OFL.txt`).
