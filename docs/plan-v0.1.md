# EDIVID v0.1 — plan technique

Suite de P0 (`p0/`), validé à l'œil le 2026-10-16 : « pas mal, des maladresses ». Ce plan couvre la
première version utilisable : une interface unique, joignable par le tailnet, pilotée par un seul LLM.

## Retours de P0 à corriger

| Constat | Correction |
|---|---|
| Les coupes mangent parfois le dernier mot | Points de coupe calés sur l'**enveloppe sonore** (10 ms) et non sur les horodatages de Whisper, qui finissent trop tôt : la fin glisse jusqu'au vrai silence (≤ 0,4 s), plus une traîne. |
| Pas assez nerveux (plan moyen 5 s) | Les silences internes sont trouvés dans l'enveloppe, puis coupés selon le rythme choisi (ample / équilibré / nerveux). |
| Répétitions maladroites au début | L'accroche est prise après la première minute et ne doit pas redire l'introduction. Garde mécanique : une phrase d'accroche qui reviendrait dans les premières secondes du corps est retirée de l'accroche. Consigne : sur deux prises d'une même idée, garder la meilleure. |
| Zooms flous (source 576 px) | **Netteté IA** : Real-ESRGAN (realesr-general-x4v3) sur la seule zone recadrée, et en option GFPGAN 1.4 sur le visage. Utilisée au rendu final seulement. |

## Architecture

```
navigateur (tailnet) ──HTTP──▶ edivid (Rust, axum) ──▶ Ollama qwen3.5:35b-a3b (seul LLM)
                                   │                     
                                   ├─▶ capteurs/analyse.py   (Whisper large-v3, enveloppe, visage YuNet)
                                   ├─▶ capteurs/nettete.py   (Real-ESRGAN, GFPGAN)
                                   ├─▶ ffmpeg / ffprobe      (préparation, plans, assemblage)
                                   └─▶ azothd /api/v1/admission (bail de carte avant tout travail GPU)
```

- **Rust** pour le serveur, la file de travaux, la logique de coupe et de cadrage, la construction des
  commandes ffmpeg, les appels au LLM. **Python** seulement pour les adaptateurs de modèles.
- **Un seul LLM** : il monte (liste de coupes), se relit (critique), et répond dans la conversation
  par des actions typées (réglages, phrases, incrustations, remonter, aperçu). Réflexion décidée par
  l'orchestrateur : montage et critique avec réflexion ; tours de conversation sans.
- **Carte graphique** : chaque travail GPU prend un bail auprès du contrôleur d'admission d'azothd
  (`other:EDIVID`) et le rend ensuite ; refus = attente annoncée dans l'interface. Le LLM garde son
  bail tant qu'il est chargé, et Ollama le décharge après 5 min d'inactivité.
- **File unique** de travaux (préparation, analyse, montage, aperçu, rendu), progression en SSE.

## Interface (un seul écran)

1. **Rushs** : glisser-déposer, ordre, état (préparé, analysé).
2. **Lecteur** + **bande de rythme** colorée par régime ; transcription cliquable (garder/couper,
   régime, cadre, incrustation).
3. Panneau à onglets : **Assistant** (conversation), **Réglages** (chaque réglage expliqué en une
   ligne), **Médias** (images, GIF, vidéos, sons, musique).
4. Barre d'actions : Proposer un montage · Aperçu rapide · Rendu final · progression.

Réglages principaux : autocut (silences, hésitations, redites), rythme, zooms (intensité, yeux,
animation, punch-in), transitions (coupe, flash, fondu, zoom, glissé, glitch, auto) et bruitage,
accroche, titres de chapitre, sous-titres (style, position), filtre image, netteté IA, son
(débruitage, voix, dé-esseur, musique avec atténuation automatique), format de sortie.

Incrustations : un média de la médiathèque (ou un effet intégré : secousse, noir et blanc) ancré
sur une phrase, avec position, taille, durée, fond vert retiré, son.

## Préparation des fichiers iPhone

ffprobe → rotation appliquée, HDR (HLG / PQ, Dolby Vision 8.4 traité en HLG) converti en SDR Rec.709
par zscale + tonemap, cadence fixe 30 i/s, son PCM 48 kHz → fichier de travail `mezz.mkv` (x264
CRF 16, une image clé par seconde pour des coupes rapides).

## Accès

`edivid` écoute sur `127.0.0.1:8030` et sur l'adresse tailnet d'azoth `100.124.48.19:8030`
(http://azoth.tailaae476.ts.net:8030). Code d'accès généré au premier lancement, demandé une fois
par navigateur. Un nouvel ordinateur ajouté au tailnet y accède avec ce code ; les ACL Tailscale
restent la première porte. Service utilisateur systemd `edivid.service`.

## Vérification

Bout en bout sur la vidéo de P0 par l'API (téléversement, analyse, montage, aperçu, rendu final avec
netteté IA), contrôle visuel des images, capture de l'interface par Chromium sans écran ;
mesure : plan moyen, coupes en fin de mot (aucune), durée de rendu.
