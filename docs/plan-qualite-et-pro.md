# EDIVID : qualité de captation d'abord, puis passage au logiciel professionnel

## Contexte

Constat du porteur du projet : sur une captation iPhone 4K, le rendu final est moins propre que l'original.
- Image : aplats (isohélie), postérisation, flou, contours redessinés.
- Son : la voix « module » entre reconstruction et captation.

Il soupçonne la chaîne de fichiers (iPhone → Google Drive en .mov → YouTube).

**Diagnostic mesuré** (2026-10-26, projet « épisode 0 », lecture seule) :

1. **La chaîne de fichiers n'est pas en cause.** Le rush venu du Drive est l'original intact : HEVC Main 10,
   HLG BT.2020, 3840×2160, **60 i/s**, 106 Mb/s. Le fichier de travail (`mezz.mkv`) est visuellement très proche
   de l'original (comparaison au pixel : planches, plante, texte du panneau).
2. **C'est le rendu qui dégrade l'image.** La comparaison source / rendu au même instant montre la texture du bois
   effacée, une peau cireuse, des hautes lumières brûlées et des halos. Les causes, par ordre de poids :
   - **PostProd du look « Clean YouTube »** (`bibliotheque/looks.json`) :
     - `lissage 20` = filtre bilatéral (`src/postprod.rs:122-128`) : c'est lui qui fait les aplats ;
     - `netteté 35` = `unsharp` (`:143-144`) : c'est lui qui redessine les contours ;
     - couleur « éclatant », exposition +3, contraste +5, appliqués en 8 bits par table de couleurs :
       postérisation et hautes lumières brûlées.
   - **Netteté IA Real-ESRGAN** sur les zooms (`capteurs/nettete.py`, modèle général ×4, mélangé à 60 %) : rendu
     « peint » et peau lissée. En 4K de sortie, **tout** zoom est un agrandissement, d'où 108 plans traités ici.
   - **Chaîne 8 bits et cadence réduite** (`src/rendu.rs` `preparer`, l.~162) :
     - le HDR 10 bits est converti en SDR 8 bits (`tonemap=hable`) dès le fichier de travail ;
     - le 60 i/s est ramené à 30 i/s.
   - **Quatre générations de compression avec perte** : fichier de travail (x264 crf 16), plans (crf 12 ou NVENC
     cq 13), assemblage (cq 17), puis l'export YouTube réencode encore (x264 à 45 Mb/s, GOP 15, 30 i/s,
     `exporter` l.~1898).
3. **La voix bascule 93 fois** entre deux timbres sur 11 min (une fois toutes les 7 s). La voix reconstruite
   (VoiceFixer) et la voix débruitée sont raccordées partout où Whisper entend un mot différent
   (`capteurs/verifie_voix.py`, fondus de 30 ms). C'est la « modulation » entendue. À vérifier aussi : le
   `loudnorm` final en une passe est dynamique et peut « pomper ».

But : un rendu au moins aussi propre que l'original, avec une seule compression avec perte, l'image ni lissée ni
redessinée par défaut, et une voix d'un seul tenant. Ensuite seulement vient la refonte professionnelle (partie B).

## Étape 0 : sauvegarder (sur `main`)

- Le dépôt est propre, 25 commits d'avance sur GitHub : `git push github main`, envoi vers athanor,
  `sauvegarder.sh`.
- Copier ce plan dans `docs/plan-qualite-et-pro.md`.

## Partie A : qualité (sur `main`, par tranches mesurées)

Méthode commune à chaque tranche :
- avant / après sur les mêmes instants du projet « épisode 0 » : un plan large et un zoom ;
- recadrages au pixel près source / rendu, et SSIM contre la source convertie de la même façon (`ffmpeg ssim`) ;
- le porteur du projet juge à l'œil et à l'oreille sur des extraits déposés dans le scratchpad (pas de rendu complet
  pour chaque essai).

### A1. Ne plus abîmer l'image par défaut (rapide, gros effet)

- **Looks** (`bibliotheque/looks.json`) :
  - `lissage` à 0 dans tous les looks, sauf un look « Beauté » assumé ;
  - `netteté` à 0 par défaut ;
  - couleur et exposition plus douces dans « Clean YouTube ».
- **Netteté** : remplacer `unsharp` par `cas` (netteté adaptative au contraste, sans halo), à faible dose.
  Le **lissage** reste un choix explicite, avec une infobulle qui dit ce qu'il fait.
- **Netteté IA** : défaut `non` (`src/reglages.rs`, `nettete_ia`).
  - Les zooms sont agrandis en `lanczos` + `cas` léger.
  - Real-ESRGAN reste une option « expérimentale », à n'essayer que sur un plan, avec « Ce plan en qualité
    finale ».
- **Plafond de zoom lié à la définition** :
  - en sortie 4K, agrandissement ×1,5 au plus, l'aperçu le montrant ;
  - en 1080p, ×2 reste du vrai pixel du rush 4K. L'aide du réglage « Définition » l'explique.
- Vérification :
  - un plan large sans PostProd a un SSIM ≥ 0,98 contre la source ;
  - les recadrages ne montrent plus d'aplats sur la peau ni de halo sur les contours.

### A2. Une seule compression avec perte, en 10 bits

- **Fichier de travail** :
  - garder la cadence de la source (réglage « Cadence » : comme la source / 30 i/s ; 60 i/s par défaut pour un
    rush à 60) ;
  - passer en 10 bits : x264 High 10 crf ≈ 10, pour des disques raisonnables, plutôt que ProRes (≈ 80 Go pour
    11 min) ;
  - conversion HDR → SDR en virgule flottante puis 10 bits ; essayer `tonemap=mobius` ou `bt2390`, en mesurant
    (le `libplacebo` présent exige la carte en Vulkan : écarté par défaut).
- **Plans et assemblage** :
  - plans en intermédiaire quasi sans perte (x264 10 bits crf 4, `-preset veryfast`, dans `tmp/`) ;
  - PostProd et table de couleurs appliquées en 10 bits ou en flottant ;
  - **un seul encodage final** : HEVC Main 10 (`libx265 -preset slow -crf 16`, ou `hevc_nvenc` main10 sous bail si
    la carte est libre), ou H.264 High si la compatibilité est demandée.
- **Exports** :
  - « YouTube » = le rendu final tel quel (copie, sans réencodage), puisqu'il dépasse déjà ce que YouTube
    recommande. YouTube réencode de toute façon : un fichier maître de haute qualité lui donne ses meilleures
    versions (VP9 et AV1 en 4K) ;
  - « iPhone léger » seul réencode ;
  - plus de `-r 30` ni de `-g 15` forcés.
- **Fichiers à toucher** : `src/rendu.rs` (`preparer`, `encodeur`, `rendre_plan`, `assembler`,
  `sortie_video`, `exporter`), `src/postprod.rs` (format de travail), `src/reglages.rs` (cadence, codec).
- **Coût** : il faut recréer le fichier de travail des rushs existants. La première fois, c'est relancé
  automatiquement, sur le processeur, environ la durée du rush.
- **Vérification** :
  - bandes de dégradé (ciel, mur blanc) sans marches : histogramme et recadrage agrandi ;
  - SSIM du rendu sans PostProd contre la source ≥ 0,98 ;
  - lecture à 60 i/s ;
  - taille et lecture sur iPhone de l'export léger.

### A3. Option sortie HDR (HLG) — la plus fidèle à l'iPhone

- La source est HLG : sortie HLG BT.2020 10 bits sans conversion en SDR, pour YouTube et l'iPhone, qui la montrent
  comme l'original.
- Il faut caler les incrustations (sous-titres, logo, textes) sur le blanc de référence HLG (203 nits), et les
  modules « direct » de l'aperçu restent en SDR (l'aperçu le dit).
- Réglage « Plage dynamique : SDR / HDR comme la source ».
- Vérification :
  - métadonnées `arib-std-b67` / `bt2020` présentes ;
  - lecture HDR sur iPhone ;
  - envoi YouTube reconnu en HDR (recette réelle).

### A4. Une voix d'un seul tenant

- **Fin des raccords mot à mot.** Une vidéo garde une seule voix de bout en bout (`src/rendu.rs`
  `voix_reconstruite`, `capteurs/verifie_voix.py`).
  - Si la reconstruction IA est choisie, Whisper sert de juge **global** : à partir de N % de mots identiques, on
    garde toute la voix reconstruite.
  - Sinon, on garde toute la voix débruitée, et l'interface dit pourquoi.
- **Défaut « captation »** :
  - débruiteur à masque (non génératif), dosé (part 0,6–0,8 mêlée à la prise d'origine, pour garder l'air de la
    pièce), sans reconstruction ;
  - ambiances de `bibliotheque/ambiances.json` revues : `restauration` à faux partout par défaut.
- **Niveau** : `loudnorm` en deux passes, linéaire (mesure puis `linear=true`), pour ne plus « pomper » ; limiteur
  vrai-crête à −1 dBTP.
- **Écoute comparée** : quatre extraits de 20 s, identiques et au même niveau, à juger avant de fixer le défaut :
  - prise brute normalisée ;
  - débruitée à 70 % ;
  - débruitée à 100 % ;
  - reconstruite entière.
- **Vérification** :
  - 0 bascule de timbre (même mesure par blocs de 10 ms qu'au diagnostic) ;
  - pas de pompage : écart-type de la sonie à court terme, avant / après ;
  - écoute du porteur du projet.

### A5. Après la qualité

L'import du micro DJI avec calage automatique (déjà évoqué) apportera le vrai gain de prise de son. C'est à
planifier séparément.

## Partie B : vers un logiciel professionnel (branche `dev/pro`)

Standards STRADA recensés de façon indirecte (Nexus 54671027, a2dc83ed, 65cecb24 ; contrat de design v1.4.0 ;
`karayan-azoth/Cargo.toml`, `justfile`, `CLAUDE.md`) ; le dépôt STRADA est sur LaSource, non lu directement.

**B0. Correctifs de sécurité urgents** (sur `main`, avec tests de régression, à faire tôt, en parallèle de A) :
1. Traversée de chemin : `DELETE /api/jeux/{jid}` (`src/jeux.rs:105-110`, `..` efface `donnees/`), ainsi que `id`
   non validé dans `video_origine` et `fichier_sortie` (`src/api.rs:1160-1180`). Fonction commune
   `ident_valide()`.
2. SMTP : un changement d'hôte efface le mot de passe gardé ; pas d'envoi sans TLS si un mot de passe est donné.
3. `url_publique` limitée à une liste d'hôtes (aujourd'hui, redirection ouverte).
4. Requêtes GET qui lancent un travail sur la carte → POST, et jeton anti-CSRF.
5. `acces.json` et `comptes.json` en 600, écrits de façon atomique ; plus de jeton Google sur la ligne de commande
   de rclone.
6. Sessions en HMAC, comparaisons en temps constant, `Secure`, limite des essais de code ; en-tête Tailscale cru
   seulement derrière `tailscale serve`.
7. Limite de taille des téléversements et quota par projet.

**B1–B8. Phases de la branche** (instance isolée port 18030 ; `main` reste en service) :

| # | Phase | Contenu |
|---|---|---|
| B1 | Socle | Workspace Cargo (`edivid-modele`, `-montage`, `-rendu`, `-calcul`, `-stockage`, `-identite`, `-travaux`, `-serveur`, `-integrations`) ; `unsafe` interdit, `clippy -D warnings`, `cargo deny`, `justfile`, toolchain épinglée ; thiserror/anyhow, aucun `panic` sur un chemin serveur |
| B2 | Rendu pur | Graphes ffmpeg en fonctions pures ; tests « dorés » (dont la chaîne 10 bits de A2) |
| B3 | Stockage et travaux | SQLite + migrations depuis les JSON ; file persistée (reprise, annulation, délai borné des baux) |
| B4 | Identité et droits | Une seule porte `Identite` ; propriétaire et partage des projets ; secrets au trousseau ou chiffrés ; adaptateur futur OAuth 2.1 + PKCE karayan.tech |
| B5 | Calcul et décision | Trait `Calculateur` (azothd local, repli processeur, futur pool GPU distant) ; **décision bureau / SaaS / hybride** sur coûts GPU mesurés |
| B6 | RGPD | Registre et AIPD (`documentation/`), effacement vérifié (local + Drive/YouTube en option), export, durées de conservation, journaux sans données personnelles, révocation Google |
| B7 | Interface | Contrat de design 1.4.0 au vert (R-01, R-06, R-07, R-11), `app.js` découpé en modules, i18n fr/en, états R-08, clavier R-09 |
| B8 | Distribution | Selon B5 : Tauri (bureau, mises à jour signées) ou serveur + workers GPU |

Processus (patron STRADA) :
- `ARCHITECTURE_EDIVID.md`, `BACKLOG.md`, `docs/REGISTRE_BUGS.md` ;
- contrats versionnés (`CONTRAT_API.md`, `CONTRAT_CAPTEURS.md`), CHANGELOG ;
- recette réelle distincte des tests ;
- une phase à la fois, fusionnée seulement validée.

## Vérification d'ensemble

- **Partie A** : chaque tranche a sa mesure (SSIM, recadrages, comptage des bascules de voix, sonie) et le jugement
  du porteur du projet sur des extraits. Recette finale : un rendu complet d'« épisode 0 », comparé à l'original
  sur iPhone et après envoi sur YouTube.
- **Partie B** :
  - `just verifier` vert à chaque commit ;
  - non-régression du rendu contre `main` (`framemd5`, écart du son en dB) ;
  - tests d'API pour chaque faille de B0 ;
  - `/security-review` à chaque phase.
