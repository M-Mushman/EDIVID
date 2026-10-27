# EDIVID : un son de studio depuis une captation iPhone (voix chaude, enveloppante)

Le plan précédent (qualité d'image, sécurité, branche pro) est archivé dans `docs/plan-qualite-et-pro.md` ; ses parties
A1–A4, A3 et B0 sont faites. Ce plan ne traite que le son.

## Contexte

Verdict du porteur du projet après écoute des quatre extraits : le débruiteur (Resemble, masque) ne change presque
rien ; la reconstruction (VoiceFixer) enlève bien la réverbération mais écrase le son, et elle invente des mots, ce
qui est interdit. Il veut une voix de studio, chaude et enveloppante, et suggère un égaliseur par correspondance
spectrale : prendre un échantillon reconstruit (ou une voix de studio, par exemple
https://www.youtube.com/watch?v=_5XylMCBlCg) comme référence et en déduire l'égalisation à appliquer au son brut.

Ce que montre la chaîne actuelle (`src/rendu.rs`) :
- la seule vraie déréverbération est générative (VoiceFixer) ; le « débruiteur » de Resemble est un masque de bruit,
  pas un dé-réverbérateur ; la porte de traîne `agate` (`reverberation`) ne fait que couper la queue entre les mots ;
- le timbre est une poignée de préréglages fixes (`filtres_direct`, rendu.rs:1044-1100 : `chaud` = bass +3,5 dB à
  150 Hz, −2 dB à 420 Hz, +1,5 dB à 2,8 kHz, aigus −3 dB, compresseur 2,5:1) ; rien ne s'adapte à la voix réelle ;
- le navigateur rejoue à l'identique les filtres `highpass/lowpass/equalizer/bass/treble/agate/acompressor/deesser/
  aexciter` (web/son-dsp.js:340-351) : tout ce qui s'exprime avec ces filtres reste réglable en direct ; le reste
  (déréverbération) s'inscrit dans l'aperçu.

État de l'art vérifié le 2026-10-27 (recherche web) :
- **Déréverbération non générative** : les modèles de séparation « voix sèche / réverbération » du monde de la
  musique (famille UVR) sont des masques spectraux : ils ne peuvent pas inventer un mot. Disponibles par le paquet
  `audio-separator` (MIT, GPU CUDA) : `dereverb_mel_band_roformer_mono_anvuew_sdr_20.4029.ckpt` (entraîné sur voix
  sèches mono, parlées et chantées ; **GPL-3.0**, à noter pour un futur produit), `UVR-DeEcho-DeReverb.pth`,
  `MDX23C-De-Reverb-aufr33-jarredou.ckpt`, `Reverb_HQ_By_FoxJoy.onnx`. Aucun banc publié sur la parole : il faut
  mesurer sur nos rushs. `DeepFilterNet3` (MIT/Apache, 48 kHz, masque + filtrage profond) est le meilleur débruiteur
  non génératif candidat. Les systèmes « studio sound » du commerce (Adobe, Descript) sont génératifs et hallucinent
  eux aussi (signalement d'utilisateurs, 2025).
- **Mesure objective** : DNSMOS P.835 (Microsoft, MIT, trois modèles ONNX, 16 kHz, fenêtres de 9 s) donne SIG/BAK/OVRL
  sur 1–5 sans référence ; c'est l'étalon des concours de débruitage. On l'exécute sur le processeur.
- **Match EQ** (Logic, Ozone, CurveEQ) : moyenne des spectres de puissance de la source et de la référence, différence
  en dB, lissage à largeur fractionnaire d'octave (1/6 à 1/3), courbe bornée, intensité dosable. Pour la parole, on
  ne garde que les segments parlés.
- **Chaîne de voix pro** : passe-haut → nettoyage (résonances, boue 250–500 Hz) → compression douce (2,5–4:1, 3–6 dB
  de réduction) → dé-esseur → égalisation de timbre (chaleur 120–250 Hz, présence 2–4 kHz, air 10–12 kHz) →
  saturation légère ; normalisation en dernier (déjà faite en deux passes).

Le vrai gain de prise de son restera le micro DJI (import avec calage, plus tard) ; ce plan tire le meilleur du son
de l'iPhone.

## Principes

1. **Aucune invention de mots** : la déréverbération ne se fait que par séparation (masque). La reconstruction
   générative ne sert plus qu'à fabriquer une *référence de timbre* (son spectre), jamais à produire le son final.
2. **Mesurer avant de choisir** : chaque candidat est noté par DNSMOS et écouté sur les mêmes extraits ; le défaut est
   celui qui gagne, pas celui qu'on suppose.
3. **Le direct reste exact** : l'égaliseur par correspondance se traduit en biquads `equalizer`/`bass`/`treble`
   que le navigateur joue déjà ; seule la déréverbération s'inscrit dans l'aperçu (comme aujourd'hui `voix_ia_*`).
4. **Un seul environnement Python** pour la boîte à outils du son : `.venv-son` (audio-separator[gpu], deepfilternet,
   librosa, scipy, onnxruntime), nouvelle branche dans `capteur()` (rendu.rs:336-340).

## Étape 1 : banc de mesure et d'écoute (1 à 2 h de calcul, carte sous bail)

Fichiers : `capteurs/qualite_son.py` (DNSMOS), `capteurs/separe.py` (déréverbération par séparation),
`outils/son/banc_son.sh` (orchestration), `essais/2026-10-27-son-studio/`.

- **Extraits** d'épisode 0, 30 s chacun, à partir de `mezz.mkv` : calme (vers 0:08), énergique (vers 3:20), riche en
  sifflantes (à repérer dans `analyse.json`). Tous ramenés au même niveau (−16 LUFS, deux passes, comme les écoutes
  précédentes) pour que l'oreille ne juge que le timbre.
- **DNSMOS** : télécharger `sig_bak_ovr.onnx`, `model_v8.onnx` (dépôt microsoft/DNS-Challenge, MIT) dans
  `modeles/dnsmos/` ; `qualite_son.py <wav…>` → CSV SIG/BAK/OVRL/P808 (réécriture compacte de `dnsmos_local.py`,
  16 kHz, fenêtres de 9,01 s, polynômes de correction).
- **Candidats de déréverbération**, tous sous bail `other:dereverb` (VRAM mesurée au premier passage, comme
  `nettete.py`) : brut ; Resemble masque (actuel) ; `dereverb_mel_band_roformer_mono_anvuew` ; sa variante
  « less aggressive » ; `UVR-DeEcho-DeReverb` ; `MDX23C-De-Reverb` ; `DeepFilterNet3` seul ; roformer puis
  DeepFilterNet3 (réverbération puis bruit) ; VoiceFixer (pour mémoire, comme plafond de « sécheresse »).
  `separe.py <entree> <sortie> <modele> [part]` : mono 44,1 kHz → 48 kHz stéréo de la durée exacte, mélange
  `part` avec l'original comme `rehausse.py` ; vérifie l'alignement à l'échantillon par corrélation (un décalage
  constant est compensé, comme les 750 échantillons de VoiceFixer).
- **Garde-fou mots** : Whisper (verifie_voix.py, mode comparaison) sur le meilleur candidat ; attendu 100 % de
  mots identiques ; sinon le candidat est écarté.
- **Sortie** : `tableau.md` (DNSMOS, VRAM, durée de calcul, accord des mots), spectrogrammes avant/après (ffmpeg
  `showspectrumpic`), MP3 numérotés pour le porteur du projet.

## Étape 2 : égaliseur par correspondance spectrale (match EQ)

Fichier : `capteurs/egaliseur.py` (numpy seulement, dans `.venv-son` pour scipy si besoin).

- **Spectre d'une voix** : segments parlés seulement (mots de `analyse.json`, ou détection d'énergie pour une
  référence externe), fenêtres de 4 096 à 48 kHz, moyenne des puissances, intégration en bandes de 1/6 d'octave
  de 63 Hz à 16 kHz, normalisation à la bande 1 kHz. Résultat : `{"bandes":[{"f":..,"db":..}]}`.
- **Différence** référence − source, lissée à 1/3 d'octave, bornée à ±8 dB, atténuée aux extrémités (rien sous 70 Hz,
  dégressif au-dessus de 12 kHz), **pente globale préservée** à hauteur de l'intensité choisie.
- **Traduction en biquads** : 11 bandes paramétriques fixes (100, 160, 250, 400, 630, 1 000, 1 600, 2 500, 4 000,
  6 300, 10 000 Hz, Q ≈ 1,4) + étagères basse (80 Hz) et haute (12 kHz) ; gains obtenus par moindres carrés sur la
  réponse réelle des biquads (formules RBJ de `son-dsp.js`, recalculées en Python), deux itérations. Erreur résiduelle
  rapportée (dB RMS). Sortie : liste `equalizer=f=..:t=q:w=..:g=..`, `bass=…`, `treble=…` → jouable à l'identique
  par le direct **sans toucher au JS**, et acceptée par le `split(',')` de rendu.rs:1099.
- **Références de timbre** (`bibliotheque/references_voix/*.json`, spectre seulement, jamais l'audio) :
  1. **« Cette voix, sans la pièce »** : spectre de la voix reconstruite (VoiceFixer) de 60 s du rush — le timbre sec
     du même locuteur ; les mots inventés sont sans importance pour un spectre. Calculé à la demande, par rush.
  2. **« Voix de studio »** (intégrée) : spectre tiré de l'extrait YouTube proposé (yt-dlp dans `.venv-son`, audio
     seul, 90 s de parole, fichier effacé après le calcul ; on ne garde que les nombres). Autre locuteur : on l'offre
     à intensité modérée (60 %) et lissage 1/3 d'octave, pour emprunter la *couleur* sans copier les formants.
  3. **« Ma référence »** : un fichier son déposé dans la médiathèque et marqué « référence de timbre » (par exemple
     l'enregistrement au NT1 évoqué le 2026-10-26).
- **Réglages** (`src/reglages.rs`, section Son) : `reference_timbre` (aucune / cette_voix / studio / media:<id>) et
  `intensite_timbre` (0–100 %, défaut 60). Les deux entrent dans `REGLAGES_SON_DIRECT` et `CLES_SON_DIRECT`
  (web/app.js:909) : changement entendu aussitôt, aperçu non périmé. Le serveur garde la courbe calculée dans
  `travail/<rush>/timbre_<reference>.json` et l'injecte dans `filtres_direct` juste avant le timbre fixe.

## Étape 3 : chaîne de voix « studio » et nouveaux préréglages

Dans `filtres_direct` (rendu.rs:1044-1100), ordre revu : passe-haut → égaliseur de correspondance → nettoyage →
compression en deux étages → dé-esseur → timbre → harmoniques. Uniquement des filtres déjà portés en JS.

- **`timbre_voix`** gagne `studio_chaud` (défaut des ambiances voix) : `highpass=f=75`, `equalizer=f=350:t=q:w=1.2:
  g=-2` (boue), `bass=g=2.5:f=170:w=0.8` (chaleur, moins fort que `chaud` car le match EQ fait le gros), `equalizer=
  f=3200:t=q:w=1.3:g=1.5` (présence), `treble=g=1:f=11000:w=0.7` (air), **deux compresseurs** : niveleur lent
  `acompressor=threshold=-24dB:ratio=2:attack=40:release=400:knee=6` puis crêtes `threshold=-16dB:ratio=3.5:
  attack=8:release=120:knee=3:makeup=…`, `deesser=i=0.35`, `aexciter=amount=0.4:drive=5:freq=3000:blend=2`
  (harmoniques douces = « rondeur »). Valeurs de départ, à régler à l'écoute sur les trois extraits.
- **Enveloppant** : nouveau lieu `studio` (traîne 0,18 s, passe-bas 6 kHz, pré-délai 8 ms, dosage −22 dB) dans
  `lieu()` (rendu.rs:1284-1311) : une pièce sèche et proche, après avoir retiré la vraie pièce. Optionnel.
- **Ambiances** (`bibliotheque/ambiances.json`) : `studio_feutre`, `podcast`, `voix_off` passent à
  `dereverberation: forte` (nouveau moteur), `timbre_voix: studio_chaud`, `reference_timbre: cette_voix`,
  `restauration: false` ; description de `studio_feutre` corrigée (elle parle encore d'IA reconstructive).
- **Déréverbération** : le réglage `dereverberation` (douce/forte) garde ses noms ; son moteur devient le gagnant de
  l'étape 1 (`voix_ia_<force>.wav` recalculé si le moteur change : clé « moteur » dans le nom du fichier). `debruitage`
  léger/fort peut appeler DeepFilterNet3 si le banc le montre meilleur qu'`afftdn`.
- **Cache de l'écoute d'essai** (rendu.rs:1236-1280) : ajouter `calage_son`, `reference_timbre`, `intensite_timbre`
  à l'empreinte (le calage en manque déjà).

## Étape 4 : interface

- Bloc « Son de la voix » (web/app.js:2431-2492) : une ligne **Timbre de référence** (menu : aucune / cette voix sans
  la pièce / voix de studio / un son de la médiathèque) et un curseur **Intensité**, marqués « en direct ». Quand
  la référence « cette voix » n'est pas encore calculée, le bouton « Calculer (carte graphique, ~1 min) » lance le
  travail ; en attendant, l'intensité est grisée.
- Les cartes d'ambiance affichent « voix de studio » dans leur résumé ; « Écouter » compare toujours sur 20 s.
- Rubrique « Fichiers du projet » : les voix traitées par le nouveau moteur et les références de timbre comptent dans
  « Voix traitées par IA ».

## Fichiers touchés

- Nouveaux : `capteurs/separe.py`, `capteurs/qualite_son.py`, `capteurs/egaliseur.py`, `outils/son/banc_son.sh`,
  `bibliotheque/references_voix/studio.json`, `modeles/dnsmos/*.onnx` (ignoré par git, téléchargé par
  l'installeur), `.venv-son` (ignoré).
- `src/rendu.rs` : `capteur()` (interpréteur `.venv-son`, étapes `separation`, `egalisation`, `qualite`),
  `filtres_direct`, `voix_sans_echo`/`debruiter` (moteur), `essai_son` (empreinte), `lieu()`, `REGLAGES_SON_DIRECT`,
  `resume_son` ; `src/config.rs` (`python_son`) ; `src/carte.rs` (`VRAM_DEREVERB_MO`, mesuré) ; `src/reglages.rs`
  (deux réglages, `accorder_son` : une référence « cette voix » exige `dereverberation ≠ non`) ; `src/api.rs`
  (`POST /son/reference` pour calculer « cette voix », `/son/direct` inchangé : il renvoie déjà `filtres`) ;
  `web/app.js` (bloc son, `CLES_SON_DIRECT`) ; `README.md`, `deploy/installer.sh` (création de `.venv-son`,
  modèles).

## Vérification

- **Étape 1** : tableau DNSMOS (SIG/BAK/OVRL) des candidats sur trois extraits ; le gagnant améliore OVRL d'au moins
  0,3 par rapport au brut et garde 100 % des mots à Whisper ; écoute du porteur du projet (MP3 au même niveau).
- **Étape 2** : distance spectrale source→référence (dB RMS sur les bandes) avant/après ; test unitaire Python :
  une source filtrée artificiellement (−6 dB à 300 Hz, +4 dB à 4 kHz) est ramenée à ±1 dB de la référence ;
  identité direct/rendu : la réponse des biquads calculée en Python égale celle de `son-dsp.js` à 0,1 dB.
- **Étape 3** : les quatre extraits d'écoute refaits (brut, déréverbéré, déréverbéré + match EQ « cette voix »,
  idem + « studio ») au même niveau ; sonie à court terme stable (écart-type) ; pas de pompage ; `cargo test`
  (ambiances valides, règles du son), banc d'interface (`outils/banc/banc.cjs`) sur le bloc son.
- **Recette** : un aperçu d'épisode 0 avec l'ambiance `studio_feutre` revue, écouté en direct dans l'interface
  (bascule « Son en direct » pour comparer avec la voix de base), puis un rendu final 1080p.
