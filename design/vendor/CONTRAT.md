# Contrat de design KARAYAN — v1.3.0

**Propriétaire et seul éditeur** : la session `[WEB]` du VPS, dépôt `/home/ubuntu/karayan-web`,
`packages/tokens`. **Distribution** : `https://mcp.mcp-zone.net/design/1.3.0/` et
`/design/latest/` ; découverte par l'outil Nexus `archi_get_design`. **Consommateurs** : tout dépôt
du parc qui rend une interface (Next/React, Tauri, FastAPI/Jinja, Rust natif), sur toute machine
(VPS, athanor, azoth, laptop).

## 0. Ce que ce contrat est, et n'est pas

- Il **impose** : les tokens (couleurs, polices, échelle typographique, espacements, rayons,
  mouvement), dix règles `R-` ci-dessous, et la manière de prouver qu'on les respecte.
- Il **n'impose pas** de bibliothèque de composants. `@karayan/ui` (React, shadcn) est proposé aux
  stacks React, jamais exigé. Les patrons HTML (`patterns/`) sont la référence de forme pour les
  autres stacks.
- Il **ne se charge jamais à chaud** depuis le VPS : il est copié dans le dépôt (`design/vendor/`),
  verrouillé (`design/karayan-design.lock`), vérifié (`design-check.py`). Une app doit rendre son
  interface VPS éteint.
- L'autorité est **l'artefact versionné**, pas un agent : une session Claude, sur n'importe quelle
  machine, applique la version que porte le lock du dépôt, rien d'autre.

## 1. Installation (une fois par dépôt)

```bash
curl -fsSLO https://mcp.mcp-zone.net/design/latest/design-sync.py
python3 design-sync.py --brand karayan --stack fastapi    # stack : web | tauri | fastapi | rust
                                                          # --surface marketing | produit | admin (R-06)
git add design/ && git commit -m "Adopte le contrat de design KARAYAN 1.3.0"
```

Crée `design/vendor/` (les fichiers utiles à la stack), `design/karayan-design.lock` (version,
marque, stack, sha256 de chaque fichier). Sur le VPS même, ou hors ligne :
`--from /home/ubuntu/karayan-web/packages/tokens/dist`. Dans le monorepo karayan-web :
`--workspace` (pas de copie, le lock pointe sur `packages/tokens/dist`).

**Mise à jour** : `python3 design/vendor/design-sync.py --version 1.1.0` — jamais implicite, toujours
consignée au registre du dépôt (entrée `D-` « contrat de design passé de X à Y »).

**Copie miroir** : quand la stack exige le fichier ailleurs (ex. `apps/api/static/karayan.css`
servi par FastAPI), `--mirror apps/api/static/karayan.css=brands/karayan.css` copie et inscrit le
miroir dans le lock ; `design-check` vérifie ensuite qu'il est identique.

## 2. Vérification (dans la recette du dépôt)

```bash
python3 design/vendor/design-check.py        # écrit design/reports/<AAAA-MM-JJ>.txt ; code 1 si erreur
```

- Une étape d'interface n'est `resolue` que si le rapport est **VERT**, ou si chaque erreur est
  couverte par `design/karayan-design.allow` citant une entrée du registre du dépôt (§4, R-10).
- Le registre du dépôt (`registre.yaml` TestLoop, ou `ARCHITECTURE.md` §5) porte une clé
  `design_contract: {version, verifie_le, rapport}` ; le rapport cité est le fichier daté.
- Le `CLAUDE.md` du dépôt porte la ligne : « Avant tout travail d'interface : `archi_get_design` ;
  ce dépôt porte `design/karayan-design.lock` ; une étape UI n'est `resolue` que si
  `design-check.py` est vert ou l'écart consigné. »

## 3. Marques

Une app = **une** marque, déclarée dans le lock. Marques du contrat : `karayan` (flagship),
`swihn`, `aristote`, `morfo`, `gnoz` (dérivés, gabarit contraint). Une app qui n'est pas une
propriété web (STRADA, Callas, Azoth Forge, outils d'athanor, backoffice) est de marque `karayan`.

## 4. Règles

Chaque règle dit comment elle est vérifiée : **mécanique** (`design-check.py`, avec sa sévérité) ou
**recette humaine** (à citer dans le registre comme `R-` TestLoop).

| Règle | Énoncé | Vérification |
|---|---|---|
| **R-01 Couleurs** | Aucune couleur littérale (`#hex`, `rgb()`, `hsl()`, `oklch()`…) ni classe de la palette Tailwind par défaut (`bg-blue-500`…) dans les sources d'interface : seulement les variables du contrat (`--primary`, `--brand-primary-500`, `--canvas-1`…) ou les utilitaires qui les portent (`bg-primary`). | mécanique, **erreur**. Exclus : `.svg`, fichiers vendorés. |
| **R-02 Polices** | Uniquement `font.display` / `font.body` / `font.mono` de la marque, via `--font-display` / `--font-body` / `--font-mono`, **auto-hébergées** (fontsource, fichiers locaux). Jamais `fonts.googleapis.com`, `fonts.gstatic.com`, `next/font/google`. | mécanique, **erreur**. |
| **R-03 Thème** | Clair et sombre obligatoires, commutés par `data-theme="light\|dark"` sur l'élément racine, préférence persistée sous la clé `karayan_theme`, `prefers-color-scheme` comme défaut initial. Jamais une classe `.dark`. | mécanique (`.dark`, autre clé) **erreur** ; absence de tout commutateur : avertissement ; les deux rendus : recette humaine. |
| **R-04 Contraste** | Texte/fond ≥ 4.5:1. Garanti par le build des tokens pour les paires sémantiques (fg/bg, primaryFg/primary, mutedFg/muted). | build (VPS) ; toute combinaison hors tokens : recette humaine. |
| **R-05 Langues** | `fr` et `en` au minimum ; aucune chaîne d'interface en dur hors du mécanisme i18n du dépôt. | recette humaine (dépend de la stack). |
| **R-06 Souveraineté** | Aucun nom de fournisseur d'IA sur une surface de marque (marketing, onboarding, compte, offre). Un sélecteur de fournisseur dans un client multi-fournisseurs (STRADA) est fonctionnel, pas marketing : autorisé. | mécanique : **erreur** pour `surface: marketing` (défaut de `stack: web`), avertissement pour `produit` / `admin` (déclarée par `--surface`, inscrite dans le lock). Jamais évaluée sur le backend `.py`/`.rs`. |
| **R-07 Échelle** | Tailles de texte, espacements, rayons = tokens (`--text-*`, `--space-*`, `--radius-*`) ; pas de `px`/`rem` inventés pour ces trois familles. | mécanique, avertissement ; recette humaine. |
| **R-08 États** | Toute liste et tout formulaire ont un état *chargement*, un état *vide*, un état *erreur*, conformes à `patterns/states.html`. | recette humaine. |
| **R-09 Clavier** | Focus visible (`--ring`) ; tout ce qui est cliquable est atteignable au clavier ; `<dialog>` piège le focus et se ferme à Échap. | recette humaine. |
| **R-10 Intégrité** | Fichiers vendorés identiques au manifeste (sha256) ; miroirs identiques ; une seule marque ; version du lock = version du manifeste vendoré ; chaque exception du `.allow` cite une entrée existante du registre. | mécanique, **erreur**. |

**Exceptions** (`design/karayan-design.allow`, JSON) : `[{"regle": "R-01", "chemin": "apps/api/app/templates/x.html", "motif": "…", "registre": "E-040"}]`. Une exception sans `registre` valide est une erreur R-10.

## 5. Patrons (`patterns/`)

`patterns.css` (classes `kd-*`, uniquement des variables du contrat) + `theme.js` (R-03 en 15
lignes) + fragments : `page.html` (coquille en-tête / contenu / pied), `header.html`, `form.html`
(champ, aide, erreur, boutons), `dialog.html` (`<dialog>` natif), `states.html` (chargement,
vide, erreur, squelette), `splash.html` + `splash.js` (écran d'accueil animé, 1.2.0). HTML + CSS purs, copiables tels quels dans un template Jinja ou une
webview. Pour React, `@karayan/ui` couvre les mêmes formes. Rendu vivant : `/contrat` du styleguide.

`.kd-divider` (1.1.0) : séparateur « ou » entre deux moyens d'action (ex. connexion fédérée /
formulaire), utilisé par les écrans d'authentification (karayan.tech `/oauth/authorize`).

**`.kd-splash` (1.2.0) — écran d'accueil, d'installation, de chargement, page « À propos ».**
Décision produit du 2026-10-09 : c'est le modèle commun de « splash screen » du parc. `splash.html`
(fragment) + `splash.js` (fond animé). Le fond est une aurore fluide en WebGL aux couleurs **de la
marque du contexte** : `splash.js` lit `--background`, `--brand-primary-500`, `--brand-accent-700`
et `--brand-primary-300` au montage (aucune couleur écrite dans le script, R-01) et suit `data-theme`
en fondu (R-03). Le nuage dérive seul ; le pointeur ne le déplace pas, il le **remue** localement
comme un fluide (traînée de tourbillons qui s'estompent). Trame fine par-dessus. **Le logo au
centre est celui de chaque application** (`.kd-splash__logo` : STRADA, morfo, gnoz…) ; karayan.tech
garde celui de KARAYAN. Un logo **en couleur** (emblème carré, comme celui de STRADA) prend en plus
`.kd-splash__logo--couleur` (1.3.0) : il n'est pas inversé la nuit et sa taille suit la fenêtre
(`min(40vmin, 100%)`) au lieu d'une hauteur de ligne. L'état d'avancement va dans `.kd-splash__etat` (`role="status"`).
`.kd-splash--plein` couvre la fenêtre ; sinon le patron remplit son conteneur (webview « À propos »).
Sans WebGL ou avec `prefers-reduced-motion`, le dégradé CSS reste ; rien ne casse. Montage :
automatique sur `canvas[data-kd-splash]`, ou `KdSplash.mount(canvas, {quality})` qui rend
`{destroy()}` pour une vue qui se ferme. Référence vivante : `https://karayan.tech/`.

## 6. Changer le contrat

Une machine **ne modifie jamais** `design/vendor/`. Elle déclare un besoin Nexus tagué `[DESIGN]`
au VPS (ce qu'il lui faut, pourquoi, capture à l'appui). Le VPS tranche (avec le porteur du projet), publie une
version, poste un message `[WEB] [DESIGN]` qui dit **cassant / non cassant**. Semver : *patch* =
valeurs (une couleur, une taille), *minor* = ajout (marque, variable, patron, règle en
avertissement), *major* = clé retirée, renommée, ou règle passée d'avertissement à erreur.

## 7. Adaptations locales existantes

À l'adoption, chaque dépôt liste ses écarts (premier rapport `design-check`). Chacun devient l'un
des trois : une **demande d'évolution** (besoin `[DESIGN]`, adoptée → version suivante), un
**écart `E-`** au registre à résorber avec date, ou une **exception** `.allow` motivée et citée.
Jamais un silence : un écart appliqué mais absent du registre est un bug.
