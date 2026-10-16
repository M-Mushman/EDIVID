# EDIVID v0.2 — plan technique

Demandes du 2026-10-16, après la v0.1 : bibliothèque de demandes, courriel en fin de montage,
dépôt depuis l'iPhone par Google Drive, encodage matériel.

## 1. Bibliothèque de demandes et consignes

- **Demandes intégrées** (`bibliotheque/demandes.json`, versionné) : formulations éprouvées pour
  l'assistant, rangées par thème (rythme, coupes, zooms, structure, son, effets, formats,
  comprendre). Les `[crochets]` marquent ce qu'on remplace ; un clic insère la demande dans la
  saisie, le premier crochet sélectionné.
- **Mes demandes** : enregistrées depuis la saisie, partagées par tous les utilisateurs de cette
  instance (`donnees/bibliotheque.json`).
- **Consignes au monteur**, par projet : texte libre ajouté aux consignes du LLM à chaque montage et
  dans la conversation (« coupes franches, jamais de glitch »). Exemples dans la bibliothèque.
- **Coulisses** : les consignes réelles envoyées au LLM (monteur, chef monteur, assistant), en
  lecture, pour s'en inspirer.

## 2. Courriel

SMTP configuré dans l'interface (Paramètres) ; mot de passe gardé dans `donnees/parametres.json`
(droits 600, hors git, jamais renvoyé au navigateur). Envois : montage prêt (après le premier
aperçu d'un montage), rendu final prêt (lien vers le projet, chemin Drive s'il y a dépôt), erreur.
Bouton d'essai. Crate `lettre` (rustls).

## 3. Google Drive

- rclone (binaire officiel dans `~/.local/bin`) ; le compte Google est relié une fois par
  `rclone config` (jeton OAuth gardé par rclone).
- Veille toutes les deux minutes sur un dossier d'entrée (`EDIVID/À monter`) :
  - un fichier vidéo = un projet ; un sous-dossier = un projet avec tous ses rushs ;
  - pris seulement quand sa taille ne bouge plus entre deux passages (envoi fini) ;
  - téléchargé, puis déplacé dans `À monter/Pris en charge` pour ne jamais être repris.
- Le projet suit seul la chaîne : préparation → analyse → montage → aperçu → rendu final (option)
  → dépôt dans le dossier de sortie (`EDIVID/Montages`) → courriel.
- Projet modèle (option) : ses réglages et ses consignes sont copiés dans chaque projet venu du
  Drive.
- Bouton « Envoyer sur Google Drive » sur tout projet rendu.

## 4. Encodage matériel

NVENC détecté au démarrage (paquet `libnvidia-encode-595-server`). Utilisé pour les plans et
l'assemblage s'il est présent ET si le contrôleur d'admission accorde un petit bail (1,5 Go) ;
sinon x264, comme aujourd'hui.

## Vérification

Courriel : serveur SMTP local de test (réception réelle du message). Drive : chaîne complète avec
un « distant » rclone local (dossier) qui joue le rôle du Drive, puis sur le vrai Drive une fois le
compte relié. Interface : captures Chromium.
