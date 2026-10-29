# EDIVID v0.5 — intro et fin de bibliothèque, vignette YouTube

Demandes du 2026-10-29.

## 0. Fait avant ce plan

- Fin de vidéo (`outils/intro/fin.py`) : titre, signature by Edina tracée, adresse du site, dans les six formats de
  sortie, son intégré (transition de 8 s du jingle A_pop_solaire). Fichiers : `donnees/habillage/fin/`.
- Intros : `outils/intro/composer.py` sort les six formats (carré et vertical 4K ajoutés) et complète les intros
  existantes sans refaire les formats déjà écrits. Carré tiré de la verticale, fenêtre placée d'après le visage.

## 1. Bibliothèque d'habillage

Un dossier commun à tous les projets, hors git : `donnees/habillage/`.
- `intros/<id>/` : `intro_<l>x<h>.mp4` dans les six formats, `vignette.jpg`, `fiche.json` (nom, photo, tirage,
  jingle d'origine, date) ;
- `fins/<id>/` : même chose (`fin_<l>x<h>.mp4`) ;
- `jingles/<id>/` : `jingle_3s.wav`, `transition_8s.wav`, `fin_16s.wav` quand ils existent, `fiche.json` ;
- `styles/<id>/` : images de référence d'un style de vignette ;
- `photos/` : les photos d'Edina (sources des intros et des vignettes).

Reprise de l'existant : les 59 intros de `essais/2026-10-27-intro` (28 photos × 2 tirages, plus les 3 variantes de
paris11), la fin by Edina, les jingles A, B, C (deux tirages chacun) et D_davinci, les photos. Les essais restent
jetables : la bibliothèque n'en dépend plus.

API : liste de chaque rayon avec ses vignettes, lecture d'un fichier ; ajout et retrait plus tard.

## 2. Générique : intro et fin choisies dans la bibliothèque

Une séquence du générique gagne un troisième genre, « bibliothèque » : l'identifiant de l'intro ou de la fin, et un
jingle facultatif.
- Panneau Générique : deux grilles de vignettes (Intro, Fin), avec « Aucune » ; survol = lecture de l'intro.
- Format : le rendu prend le fichier du format de sortie (`intro_1080x1920.mp4` pour un montage vertical 1080, etc.).
  S'il manque, le format le plus proche, recadré sur fond flou comme aujourd'hui.
- Son : intros et fins ont leur jingle intégré, gardé par défaut. Choix « Son : celui de l'intro / un jingle de la
  bibliothèque » ; un jingle choisi remplace le son, coupé à la durée de la séquence, avec fondu.
- L'aperçu (960 × 540) suit les mêmes règles.

## 3. Raccords en fondu, image et son

Aujourd'hui chaque séquence du générique reçoit un fondu au noir de 0,35 s, puis tout est mis bout à bout : l'intro
finit au blanc, passe un instant au noir, et le son s'arrête net avant la voix.
À la place, un fondu enchaîné de 0,5 s entre l'intro et le montage, et entre le montage et la fin, pour l'image et
pour le son ensemble : le jingle s'éteint pendant que la voix monte. Les séquences de bibliothèque ne reçoivent pas
de fondu au noir (elles ont le leur, au blanc). Les décalages qui dépendent de la durée du générique (chapitres
YouTube, sous-titres, commentaires sur un instant) tiennent compte du chevauchement.

## 4. Fiche de la vidéo : titre et description

Le projet gagne un titre et une description, dans un panneau « Fiche de la vidéo » visible en haut du projet.
Bouton « Proposer » : le LLM les tire du contenu (la proposition faite aujourd'hui pour YouTube, reprise). La
publication YouTube et la vignette partent de cette fiche.

## 5. Vignette YouTube

Entrées : le contenu de la vidéo (transcription), le titre et la description de la fiche, un style de la
bibliothèque, une photo d'Edina.
1. Le LLM décrit la scène pour le modèle d'image (lieu, geste, humeur tirés du contenu) et propose un texte de
   vignette : le titre, raccourci s'il est long (une vignette se lit en une seconde), modifiable.
2. FLUX.2 klein 4B (déjà installé pour les décors des intros) dessine une image 1280 × 720 à partir de deux
   références : la photo d'Edina (ressemblance) et une image du style. Quatre propositions, sous bail de la carte,
   passation annoncée.
3. Le titre est posé par le code (police du titre de l'intro, contour et ombre pour la lisibilité), du côté libre de
   l'image ; jamais par le modèle, qui déforme les lettres. Le logo by Edina en option.
4. Choix parmi les quatre dans l'interface ; JPG 1280 × 720 de moins de 2 Mo, gardé dans le projet ; envoyé à
   YouTube avec la vidéo (`thumbnails.set`, chaîne validée par téléphone).

Risque : la ressemblance d'Edina une fois passée en style manga. Premier essai avant toute intégration : 3 photos ×
2 styles × 4 tirages, jugés à l'œil.

## Ordre

1 → 2 → 3 → 4 → 5. Seul le 5 utilise la carte graphique, et seulement après accord.

## Vérification

Rendu final réel d'un projet témoin en horizontal, carré et vertical, avec intro et fin de bibliothèque : ffprobe
(format, durée = intro + montage + fin − 2 × 0,5 s), images autour de chaque raccord, écoute des raccords. Vignettes :
les quatre propositions d'un projet réel, regardées en taille YouTube (petite et grande).

## Réalisé (2026-10-29)

- Bibliothèque : 29 intros (une par photo : tirage g31, variante 21 pour paris11, les tirages se valent), la fin
  by-edina, 7 jingles, le style manga_selfie (7 images), 29 photos, le logo. Fichiers liés (liens physiques) depuis
  `essais/2026-10-27-intro`, sans copie.
- Générique de bibliothèque, fondu enchaîné, aperçu : vérifiés sur une copie d'« épisode 0 » (montage réduit à
  36 s) par une instance d'essai (port 8039, `EDIVID_NVENC=0`) : durée exacte (2,5 + 36,3 + 7,23 s), version au
  format pris en horizontal comme en vertical, raccords en fondu image et son.
- Défaut corrigé au passage : le son du générique était calé à −14 LUFS avant le gain final, la voix à −23 ; une
  intro sortait ~10 LU au-dessus de la voix dans la vidéo finale (vidéos de générique comprises). Désormais −23 au
  rendu final (jingle −13,1, voix −14,9 LUFS mesurés), −14 dans l'aperçu (le navigateur ne relève que la voix).
- Vignettes : premier essai (scène entière d'après photo + image de style) raté : style « BD » et non anime, Edina
  méconnaissable et rajeunie. Second essai : style décrit en toutes lettres et âge précisé ; la méthode « retouche »
  (photo redessinée, décor changé) garde tenue, bijoux, coiffure. Les deux méthodes sont proposées (deux vignettes
  chacune). Texte placé à l'opposé du visage repéré par YuNet. Fabrication complète dans EDIVID : ~1 min de carte.

## Retouches (2026-10-29, après essais réels)

- Vidéo finale : une seule qualité proposée, la maximale (4K, cadence du rush, HEVC 10 bits) ; en 1080p, les zooms
  sont trop mous pour YouTube. 4K par défaut pour les nouveaux projets ; les réglages avancés restent repliés.
- Fin by Edina : le courriel ciao@by-edina.ch sous l'adresse du site. Son : la transition de 4 mesures était coupée
  au milieu du morceau avec un fondu. Les morceaux d'ACE-Step finissent tous sur un accord tenu ; chaque jingle a
  désormais une version `final` (phrase de 4 mesures prise à son premier temps, puis l'accord final jusqu'à son
  extinction, ~9,7 s). La fin dure donc ~9,7 s au lieu de 7,7 s et repart au blanc pendant que l'accord s'éteint.
  Un jingle choisi sur une fin prend cette version, calée sur la fin de la séquence.
- Intros : même défaut, le jingle de 3 s s'arrêtait en fondu au milieu d'une mesure. Nouvelle version `intro` de
  chaque jingle : la dernière mesure du morceau, puis l'accord final tenu 1,6 s (~3,5 s). Les 29 intros sont
  recomposées dessus (3,57 s ; l'animation de 3,1 s garde sa dernière image pendant le passage au blanc). La durée
  des générique déjà réglés suit toute seule (relue dans la bibliothèque).
