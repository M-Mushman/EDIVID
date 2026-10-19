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

Interface : `https://<machine>.<tailnet>.ts.net` (par `tailscale serve`), ou `http://<machine>:8030`
(localhost et adresse du tailnet, renvoyée vers l'https). Un appareil du tailnet appartenant à une
personne entre sans code (identité `Tailscale-User-Login`, crue seulement par le relais local) ; sinon,
code d'accès tiré au premier lancement dans `donnees/acces.json`.

## Fonctions

- Bibliothèque de demandes (`bibliotheque/demandes.json`) et demandes enregistrées par les
  utilisateurs ; consignes au monteur par projet ; « Coulisses » : les consignes réelles du LLM.
- Courriels (montage prêt, vidéo finale prête, problème) par SMTP, réglés dans Paramètres.
- Google Drive par rclone (`~/.local/bin/rclone`, compte relié par `rclone config`) : un fichier ou
  un sous-dossier déposé dans le dossier d'entrée devient un projet monté automatiquement ; la
  vidéo finale part dans le dossier de sortie.
- Lecteur Origine / Aperçu / Rendu final, deux timelines (vidéo d'origine avec ce que le script garde,
  montage) que l'on allume ou éteint ; panneaux repliables.
- Commentaire sur un instant : le LLM reformule, propose une correction sur une plage de phrases,
  la personne valide, corrige ou annule.
- Module Texte (effets fondu, splash, glissés, machine à écrire) et glisser-déposer des textes et
  médias directement sur la vidéo ; calque d'édition en pause.
- Connexion avec Google (OAuth, application Web, HTTPS du tailnet par `tailscale serve`) : ouvre la
  session et relie le Drive du compte ; veille par compte, la vidéo montée retourne à son Drive.
- Synchronisation son/image vérifiée au témoin flash + bip (latence d'afftdn compensée), calage
  manuel ± 300 ms, encodage final strictement compatible iPhone. Chaque plan a une durée exacte en
  images et en échantillons (sinon la voix dérivait d'environ 110 ms par minute) ; le rapport du rendu
  donne l'écart son/image du fichier final.
- Cadrage lissé : un coefficient de saut (rapport de taille du visage à l'écran d'un plan au suivant)
  est borné sur les coupes franches (réglage « Écart entre deux cadrages ») et rapporté (médiane, p90,
  max) ; l'agrandissement est plafonné à ×3 de la source, au-delà duquel l'IA invente les traits.
- Netteté IA : Real-ESRGAN mélangé à 60 % avec l'image d'origine agrandie, pour garder le grain de peau.
- Habillage permanent (logos et textes sur toute la durée, place, taille, opacité) et générique (vidéo, ou
  titre sur une image, avant et après le montage, enchaînés en fondu) ; repris par les jeux de réglages,
  avec copie des médias nécessaires.
- Choix de fichiers sur le Google Drive relié (rushs ou médiathèque), téléchargés avec progression.
- Rognage du haut de la source (logo CapCut ou TikTok) ; timbre de voix clair, chaud ou naturel ;
  look « Lumière cinéma ».
- Export YouTube (débits recommandés), léger (720p) ou qualité maximale ; publication sur YouTube
  (API Data v3, envoi reprenable) avec titre, description à chapitres et mots-clés proposés par le LLM.
- Transcription modifiable en place et « rechercher et remplacer » ; les sous-titres suivent.
- Onglet PostProd (couleur, lissage du teint, diffusion, netteté, grain, vignette, bandes cinéma) et
  bibliothèque de looks mode et beauté (`bibliotheque/looks.json`) ; avant/après sur l'image affichée.
- Encodage NVENC si le paquet `libnvidia-encode-<branche>` du pilote est installé, sinon x264.

Avant tout travail sur la carte graphique, EDIVID prend un bail auprès du contrôleur d'admission
d'azothd (`other:EDIVID`) et le rend ensuite.

Police Anton : SIL Open Font License (`modeles/polices/OFL.txt`).
