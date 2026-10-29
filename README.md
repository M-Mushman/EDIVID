# EDIVID

Monteur vidéo IA local. On dépose des rushs (vidéo face caméra, fichiers iPhone acceptés tels quels),
un seul LLM (qwen3.5:35b-a3b dans Ollama) propose un montage rythmé — coupes, zooms, accroche,
transitions, sous-titres — puis on l'ajuste en discutant avec lui, dans une interface web unique.

- `src/` : serveur Rust (axum) — file de travaux, découpe et cadrage, rendu ffmpeg, LLM, API.
- `capteurs/` : adaptateurs de modèles en Python — `analyse.py` (Whisper large-v3, enveloppe
  sonore, visage YuNet), `nettete.py` (Real-ESRGAN, GFPGAN), `seche_ia.py` (voix séchée : pièce et bruit retirés par
  MossFormer2_SE_48K de ClearerVoice, un masque qui n'invente aucun mot ; `.venv-clearvoice`), `egaliseur.py`
  (égaliseur par correspondance spectrale : spectres 1/6 d'octave, courbe traduite en biquads identiques au direct),
  `qualite_son.py` (DNSMOS P.835, mesure sans référence), `debruite.py` (DeepFilterNet3, `.venv-deepfilter`),
  `restaure.py` (timbre reconstruit par VoiceFixer, `.venv-voicefixer` : sert de *référence de timbre*, plus de
  son final), `verifie_voix.py` (Whisper compare deux voix mot à mot), `separe.py` et `seche.py` (bancs d'essai),
  `rehausse.py` (ancien débruiteur Resemble, `.venv-rehaussement`, plus utilisé par le rendu).
- `web/` : interface (HTML, CSS, JavaScript sans dépendance).
- `deploy/` : service utilisateur systemd et script d'installation.
- `docs/` : plans techniques (`plan-qualite-et-pro.md` : qualité de captation, puis logiciel professionnel) ;
  `p0/` : scripts de la preuve de concept d'origine.
- `outils/` : banc d'essai de l'interface (`banc/banc.cjs`, Playwright) et médias d'essai.
- `essais/` (hors git, sauf `LISEZMOI.md`) : un dossier jetable par campagne d'essais (`AAAA-MM-JJ-sujet`).

## Installation

```
uv venv --python 3.12 .venv
uv pip install --python .venv/bin/python faster-whisper opencv-python-headless numpy soundfile \
    nvidia-cublas-cu12 "nvidia-cudnn-cu12==9.*" torch spandrel
# modèles dans modeles/ : yunet.onnx, realesr-general-x4v3.pth, GFPGANv1.4.pth
# débruiteur IA de la voix, dans son propre environnement (le paquet fige d'anciennes versions) :
uv venv --python 3.12 .venv-rehaussement
uv pip install --python .venv-rehaussement/bin/python --index-url https://download.pytorch.org/whl/cu130 \
    --extra-index-url https://pypi.org/simple torch==2.14.0 torchaudio
uv pip install --python .venv-rehaussement/bin/python --no-deps resemble-enhance==0.0.1
uv pip install --python .venv-rehaussement/bin/python omegaconf librosa resampy rich tabulate soundfile tqdm \
    "numpy<2" "pandas<2.3" "scipy<1.15" "matplotlib<3.10"
# poids (licence MIT) dans modeles/resemble-enhance/enhancer_stage2 : hparams.yaml, ds/G/latest,
# ds/G/default/mp_rank_00_model_states.pt (huggingface.co/ResembleAI/resemble-enhance)
# timbre reconstruit par IA (VoiceFixer, licence MIT), son propre environnement ; poids téléchargés au premier usage
# dans ~/.cache/voicefixer :
uv venv --python 3.11 .venv-voicefixer
uv pip install --python .venv-voicefixer/bin/python --index-url https://download.pytorch.org/whl/cu130 \
    --extra-index-url https://pypi.org/simple torch==2.14.0 torchaudio
uv pip install --python .venv-voicefixer/bin/python voicefixer==0.1.3
# voix séchée (MossFormer2, ClearerVoice-Studio ; poids téléchargés au premier usage) :
uv venv --python 3.12 .venv-clearvoice && uv pip install --python .venv-clearvoice/bin/python clearvoice soundfile
# boîte à outils du son (égaliseur, DNSMOS, séparation d'essai) :
uv venv --python 3.12 .venv-son && uv pip install --python .venv-son/bin/python "audio-separator[gpu]" librosa scipy soundfile numpy yt-dlp
# DeepFilterNet3 sur le processeur (le paquet exige l'ancienne interface de torchaudio) :
uv venv --python 3.12 .venv-deepfilter && uv pip install --python .venv-deepfilter/bin/python \
    --index-url https://download.pytorch.org/whl/cpu --extra-index-url https://pypi.org/simple torch==2.8.0 torchaudio==2.8.0 deepfilternet soundfile
# DNSMOS (microsoft/DNS-Challenge, CC BY 4.0) : modeles/dnsmos/sig_bak_ovr.onnx et model_v8.onnx
./deploy/installer.sh
```

Interface : `https://<machine>.<tailnet>.ts.net` (par `tailscale serve --bg http://127.0.0.1:8031`), ou
`http://<machine>:8030` (localhost et adresse du tailnet, renvoyée vers l'https). Un appareil du tailnet
appartenant à une personne entre sans code (identité `Tailscale-User-Login`, crue seulement sur l'écoute
réservée au relais, `EDIVID_ECOUTE_RELAIS`, 127.0.0.1:8031 par défaut) ; sinon, code d'accès tiré au premier
lancement dans `donnees/acces.json` (droits 600, dix essais par quart d'heure).

Sécurité : tout chemin d'adresse qui sortirait de son dossier est refusé ; toute requête qui modifie porte
l'en-tête `X-EDIVID: 1` (contre les requêtes forgées) ; sessions signées HMAC-SHA256 ; secrets (code, jetons
Google, mot de passe SMTP, compte rclone) écrits de façon atomique en 600 ; téléversements bornés (64 Go,
50 Go laissés libres).

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
- Qualité de captation : fichier de travail 10 bits à la cadence du rush (60 i/s), conversion HDR « mobius »,
  zooms agrandis en lanczos (plafond ×1,5 en sortie 4K, ×3 en 1080p) relevés par netteté adaptative (CAS), une
  seule compression avec perte (HEVC Main 10, ou H.264), export YouTube = le rendu final lui-même. Sortie HDR
  (HLG) au choix : incrustations portées en HLG selon BT.2408 (blanc à 75 % du signal). Couleurs des
  sous-titres et textes compensées (ffmpeg les dessine en BT.601).
- Netteté IA (expérimentale, désactivée par défaut) : Real-ESRGAN mélangé à 60 % avec l'image d'origine
  agrandie ; « Ce plan en qualité finale » permet de l'essayer sur un plan.
- Son de studio (2026-10-27) : voix séchée par IA (MossFormer2 : RT60 apparent 360 → 130 ms sur épisode 0, mots
  intacts), timbre de référence par égaliseur de correspondance (« cette voix, sans la pièce » = spectre de sa propre
  voix reconstruite par IA ; « voix de studio » intégrée, `bibliotheque/references_voix/` ; ou un son de la
  médiathèque), intensité dosable, entendu en direct ; timbre « Studio chaleureux » (deux compressions douces,
  sifflantes, rondeur) ; lieu « Studio » très court. Banc reproductible : `outils/son/banc_son.sh`.
- Fichiers du projet (Réglages) : place par catégorie ; fichiers de travail, voix traitées, écoutes,
  extraits et temporaires se libèrent d'un clic et se recalculent au besoin.
- Habillage permanent (logos et textes sur toute la durée, place, taille, opacité) et générique (intro ou fin de
  la bibliothèque d'habillage, vidéo, ou titre sur une image, avant et après le montage) ; repris par les jeux de
  réglages, avec copie des médias nécessaires.
- Bibliothèque d'habillage par compte (`donnees/habillage/<compte>/`, hors git ; chaque projet a un propriétaire, la
  personne connectée qui l'a créé ou le compte principal) : `intros/` et `fins/` (un
  dossier par élément, `intro_<l>x<h>.mp4` dans les six formats de sortie, `vignette.jpg`, `fiche.json`), `jingles/`,
  `styles/` (images de référence des vignettes), `photos/`, `logos/`, `medias/` (versés par les projets supprimés). Le rendu prend la version au format de sortie
  et l'enchaîne au montage en fondu croisé d'une demi-seconde, image et son ; un jingle de la bibliothèque peut
  remplacer le son de l'élément, dans sa version qui finit franchement sur l'accord final du morceau (`intro` : la
  dernière mesure et l'accord, ~3,5 s ; `final` : la phrase qui y mène et l'accord jusqu'à son extinction, ~9,7 s,
  calée pour s'éteindre avec la séquence). Fabrication : `outils/intro/` (`composer.py` pour les intros, `fin.py` pour
  la fin, `declinaisons.py` pour les versions d'un jingle, `ranger.py` pour ranger un élément dans la bibliothèque
  d'un compte, `EDIVID_COMPTE=<adresse>`).
- Fiche de la vidéo : titre, description, mots-clés (proposés par le LLM), reprise par la publication YouTube ;
  vignette YouTube (`capteurs/vignette.py`, FLUX.2 klein 4B sous bail de la carte, `.venv-video`) : Edina redessinée
  dans un style de la bibliothèque, dans un décor tiré de la vidéo, texte posé par le code à l'opposé du visage ;
  quatre propositions, la choisie part avec la vidéo (`thumbnails.set`, chaîne validée par téléphone).
- Choix de fichiers sur le Google Drive relié (rushs ou médiathèque), téléchargés avec progression.
- Rognage du haut de la source (logo CapCut ou TikTok) ; timbre de voix clair, chaud ou naturel ;
  look « Lumière cinéma ».
- Export YouTube et archive (le rendu final, sans recompression), léger (720p, H.264, SDR) ; publication sur YouTube
  (API Data v3, envoi reprenable) avec titre, description à chapitres et mots-clés proposés par le LLM.
- Transcription modifiable en place et « rechercher et remplacer » ; les sous-titres suivent.
- Onglet PostProd (couleur, lissage du teint, diffusion, netteté, grain, vignette, bandes cinéma) et
  bibliothèque de looks mode et beauté (`bibliotheque/looks.json`) ; avant/après sur l'image affichée.
- Ambiances sonores (`bibliotheque/ambiances.json`) : studio feutré, chambre, podcast, voix off, extérieur,
  salles, cathédrale, radio, téléphone ; écoute d'essai de vingt secondes avant d'appliquer. Écho et bruit
  retirés par IA, porte de traîne, acoustique de lieu par convolution, voix ramenée à −23 LUFS avant traitement.
- Sauvegarde d'un projet en zip téléchargé (`outils/sauvegarde.py`, écrit à la volée, ZIP64) : rushs d'origine,
  médias, montage, réglages, analyses, vidéo finale, vignettes ; sans la vidéo de travail ni les aperçus, qui se
  refont. Suppression ensuite : les médias du projet rejoignent la bibliothèque du propriétaire. Réimport depuis
  l'accueil : nouveau projet, vidéo de travail refaite sur le processeur, analyse reprise du zip.
- Encodage NVENC si le paquet `libnvidia-encode-<branche>` du pilote est installé, sinon x264 ; `EDIVID_NVENC=0`
  force le processeur sans même essayer la carte (instance d'essai).

Avant tout travail sur la carte graphique, EDIVID prend un bail auprès du contrôleur d'admission
d'azothd (`other:EDIVID`) et le rend ensuite.

Police Anton : SIL Open Font License (`modeles/polices/OFL.txt`).
