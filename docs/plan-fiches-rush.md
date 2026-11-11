# Plan — fiches de rush, mode Séance, lumière stable (2026-11-11)

Origine : projet « Entraînement court et complet avec une chaise » (10 rushs, 22 min). Les consignes du
porteur du projet (coupes à un temps donné par rush, recadrage par rush, séances de sport en plan fixe sans
coupe des silences, son peu traité, lumière normalisée) ne pouvaient pas être tenues par le LLM : le montage ne
se construisait qu'à partir des phrases parlées, et ni le recadrage par rush ni la coupe à un temps donné
n'existaient.

Principe : ce qui est une contrainte mesurable devient un réglage appliqué par le code (priorité absolue) ; le
LLM ne garde que les choix éditoriaux. Les consignes écrites priment sur les règles générales, relecture
comprise.

## 1. Fiche de rush (`Rush.fiche`)
- `seance` (bool) : Parole (défaut) ou Séance.
- `debut`, `fin` (secondes, optionnels) : plage gardée, fixée à la main.
- `rognage` [gauche, droite, haut, bas] (part de l'image, 0 à 0,45).
- Plage effective = valeur manuelle, sinon plage automatique (§3).
- API : `GET/POST /api/projets/{id}/rushs/{rid}/fiche`. Interface : bouton « Réglages du rush » sur chaque rush.

## 2. Plage et phrases
- `phrases_projet` ne garde que les mots situés dans la plage effective de leur rush.
- Rush en Séance sans aucune parole dans sa plage : une phrase vide (« séance sans parole ») le représente.

## 3. Entrée et sortie de champ (plage automatique)
- Début : le plus tôt entre « visage stable pendant 2 s » (taille proche de la médiane du rush) et « premier mot
  − 0,5 s ». Fin : le plus tard entre la fin du dernier visage stable et « dernier mot + 0,6 s ». Prudent : on ne
  perd jamais une parole.

## 4. Mode Séance (construction des plans)
- Toutes les phrases du rush gardées ; chacune couvre la plage jusqu'à la suivante (limite au milieu du
  silence) : la séance passe en continu, silences compris, plans jointifs à l'image près.
- Cadre « large » fixe, ancré une fois pour tout le rush ; ni punch, ni poussée, ni coupe de silence ; transition
  seulement à l'entrée du rush. Jamais dans l'accroche. Titres de chapitre permis.

## 5. Recadrage
- La grille de cadrage de chaque rush part de la zone utile (rognage de la fiche + `rognage_haut`), au format de
  sortie, centrée sur la personne.

## 6. LLM
- Transcription : phrases de séance marquées ; consigne : les garder, plan large, pas de choix.
- Relecture (chef monteur) : les consignes de la personne priment sur sa liste de vérifications.

## 7. Lumière stable (réglage `lumiere_stable`)
- Mesure de la luminance moyenne du proxy à 2 i/s (`travail/<rush>/lumiere.json`), lissée sur 3 s.
- Correction d'exposition (±0,4 IL au plus) vers la médiane du montage, suivie dans le temps (sendcmd +
  exposure) à l'assemblage, aperçu compris. Non appliquée en sortie HDR.

## 8. Ce projet
- 8693, 8729 : Parole, rognage gauche 25 %. 8698 et suivants : Séance.
- Fins : 8700 6:32, 8702 1:44, 8721 1:02, 8704 0:16, 8705 3:03, 8708 0:11.
- Son minimal (micro DJI) : voix de studio, déréverbération, réverbération, déesseur coupés, timbre naturel.
- Lumière stable activée.
