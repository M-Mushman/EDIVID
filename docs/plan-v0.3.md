# EDIVID v0.3 — plan technique

Demandes du 2026-10-16, après la v0.2.

## 1. Panneaux repliables

Chaque carte (1. rushs, 2. aperçu, 3. texte) et le panneau de droite se replient d'un clic sur leur
titre ; l'état est gardé par navigateur.

## 2. Deux timelines, la vidéo d'origine gardée

- **Proxy** de chaque rush (540 p, H.264 + AAC, lisible par tout navigateur), fait à la préparation ;
  les rushs déjà prêts reçoivent le leur au démarrage.
- Le lecteur bascule entre **Origine**, **Aperçu** et **Rendu final**.
- **Timeline d'origine** : une ligne par rush, à l'échelle de sa durée ; les passages gardés par le
  script (montage actuel, sans attendre un nouvel aperçu) sont colorés par régime, le reste est gris.
- **Timeline du montage** : les plans de l'aperçu, comme aujourd'hui.
- Chacune s'allume ou s'éteint ; les têtes de lecture se suivent (un instant du montage est montré à
  sa place dans l'origine). `GET /api/projets/{id}/programme` donne le script courant.

## 3. Commentaire sur un instant

En aperçu, pause → « Commenter ce moment » : la personne écrit la correction voulue. Le LLM :
1. reformule ce qu'il a compris ;
2. propose une correction limitée à une plage de phrases adaptée (coupes, régime, cadre,
   incrustations, textes) ;
3. la plage est montrée sur les deux timelines.
La personne **valide** (application, puis nouvel aperçu), **corrige** (précise ; le LLM repropose
en tenant compte de sa proposition précédente) ou **annule**. Tout est consigné dans la conversation.

## 4. Textes et glisser-déposer

- Module « Texte » : contenu, couleur, fond (aucun, boîte, bandeau), taille, effet d'entrée (fondu,
  splash, glissé gauche/droite/haut, machine à écrire), durée ; aperçu du bloc en direct.
- Le bloc se **glisse sur la vidéo** : l'endroit du dépôt fixe la position, l'instant de la lecture fixe
  le moment (phrase + décalage). Les médias de la médiathèque se glissent de la même façon.
- Calque d'aperçu au-dessus du lecteur : les textes et médias actifs à l'instant affiché, déplaçables
  à la souris.
- Rendu : textes en ASS (`\pos`, `\move`, `\fad`, `\t` pour le splash, alpha par lettre pour la
  machine à écrire) ; médias avec position libre et effets d'entrée (fondu, splash par `scale`
  animé, glissé par `overlay` animé).

## 5. Connexion avec Google

- « Se connecter avec Google » sur l'écran d'accueil : OAuth 2.0 (openid, email, profil, Drive).
- Comptes autorisés : liste dans Paramètres ; un compte s'ajoute en le reliant une première fois
  depuis une session ouverte par code. Le code d'accès reste en secours.
- La connexion relie le Drive du compte : jeton gardé sur azoth (`donnees/comptes.json`, 600),
  distant rclone créé automatiquement (`edivid_<compte>`) ; chaque compte peut activer la veille de
  son propre Drive ; les vidéos montées retournent au Drive d'où elles viennent, le courriel part au
  compte concerné.
- Prérequis à faire par le porteur du projet : un client OAuth « Application Web » dans Google Cloud,
  et l'HTTPS du tailnet (`tailscale serve`), Google n'acceptant une adresse de retour qu'en HTTPS.
