# EDIVID v0.4 — plan technique

Demandes du 2026-10-16 au soir.

## 1. Synchronisation son/image

Mesure faite avec une vidéo-témoin (flash blanc et bip simultanés chaque seconde) passée par le
vrai moteur : aucune dérive, mais le son arrive 25 ms en retard dès que la réduction du bruit est
active. Cause isolée : `afftdn` a une latence fixe de 1 200 échantillons (48 kHz). Corrections :
- compensation exacte après `afftdn` (on retire les 1 200 premiers échantillons, on complète la fin) ;
- réglage « Calage son/image » (± 300 ms) pour un décalage présent dans les rushs eux-mêmes ;
- encodage final strictement compatible iPhone : cadence fixe imposée, H.264 High 4.1, AAC-LC 48 kHz.

## 2. Export « prêt pour YouTube »

Le rendu final reste le fichier maître. « Exporter » en tire des versions par réencodage rapide :
- **YouTube** (recommandé) : H.264 High, 30 i/s fixes, débit recommandé par YouTube (1080 p :
  12 Mb/s ; 720 p : 7,5 Mb/s ; 2160 p : 45 Mb/s), AAC 384 kb/s, faststart ;
- **Léger** (iPhone, WhatsApp, courriel) : 720 p, ~3 Mb/s, AAC 128 kb/s ;
- **Qualité maximale** (archive) : CRF 15.
Définition au choix (720 p, 1080 p, 2160 p si la source le permet).

## 3. Publication sur YouTube

Portée `youtube.upload` ajoutée à la connexion Google ; jetons de rafraîchissement gardés sur azoth
(`donnees/comptes.json`, 600). Envoi par l'API YouTube Data v3 (envoi reprenable), titre,
description, mots-clés, visibilité (privée par défaut), catégorie. Lien de la vidéo en retour, et par
courriel. Limite connue : tant que le projet Google n'a pas passé l'audit de l'API YouTube, Google
verrouille en privé les vidéos envoyées par l'API.

## 4. Transcription modifiable

Chaque phrase du panneau 3 se corrige en place (orthographe, mots mal transcrits). Les mots
corrigés reprennent le minutage des mots d'origine (un pour un si le nombre est égal, sinon réparti
selon la longueur). « Remplacer partout » pour une erreur répétée. Les corrections nourrissent les
sous-titres et le LLM.

## 5. Onglet PostProd et bibliothèque de looks

Traitements appliqués au montage entier, à l'aperçu comme au rendu :
- couleur (préréglages de colorimétrie) ;
- lissage du teint (filtre bilatéral mélangé à l'original : la peau s'adoucit, les contours restent nets) ;
- diffusion (« Pro-Mist » : hautes lumières adoucies) ;
- netteté (unsharp) ;
- grain (bruit temporel fin) ;
- vignette ;
- bandes cinéma (format 2,39:1).
Chaque traitement a son intensité. Une bibliothèque de looks « mode et beauté » règle tout d'un clic :
Beauté naturelle, Éditorial mode, Glow peau, Cinéma chaud, Couture noir et blanc, Film 35 mm,
Pastel doux, Clean YouTube.
Image avant/après à l'instant affiché (`GET /postprod/image`), pour juger sans rendre.

## Vérification

Témoin flash/bip après correction (écart nul) ; export YouTube contrôlé à ffprobe ; correction de
texte visible dans les sous-titres ; images avant/après de chaque look ; envoi YouTube vérifiable
seulement une fois le compte Google relié (sinon refus clair).
