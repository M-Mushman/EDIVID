# Essais

Tout ce qui sert à mesurer ou à comparer, hors du code : rendus d'essai, extraits d'écoute, images comparées,
journaux. Rien ici n'est nécessaire au fonctionnement d'EDIVID, et rien n'est suivi par git (sauf ce fichier).

- Un dossier par campagne, nommé `AAAA-MM-JJ-sujet`. Une campagne se supprime d'un coup :
  `rm -rf essais/2026-10-26-qualite`.
- `banc/` : données exportées d'un projet pour le banc d'essai de l'interface (`outils/banc/banc.cjs`).
  On les refait avec le test ignoré `cadres_d_un_projet` (variable `EDIVID_SORTIE`).
- Les fichiers recalculables d'un projet (fichier de travail, voix traitées, aperçus, extraits) ne sont pas
  ici : ils se libèrent dans EDIVID, rubrique « Fichiers du projet ».

Campagnes :

| Dossier | Contenu |
|---|---|
| `2026-10-16-p0` | Preuve de concept d'origine : analyses, montages et rendus (les scripts sont dans `p0/`). |
| `2026-10-17-rush-essai` | Court rush d'essai iPhone. |
| `2026-10-18-nettete` | Netteté IA : Real-ESRGAN seul, avec GFPGAN, comparaison. |
| `2026-10-26-interface-et-son` | Écoutes : chaîne de la voix, voix IA (Resemble, VoiceFixer), ambiances ; capture de l'interface. |
| `2026-10-26-qualite` | Diagnostic de la qualité de captation (épisode 0) : images avant / après, quatre extraits d'écoute de même niveau (brut, débruité 70 %, 100 %, reconstruit), rendu final 4K 60 i/s HEVC 10 bits, aperçu, journal du rendu. |
