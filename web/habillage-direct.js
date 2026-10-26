// Habillage en direct : logos et textes posés sur toute la durée, dessinés par-dessus la vidéo du lecteur avec la
// même géométrie que le rendu final (src/rendu.rs : graphe_logo, habillage_textes). Toutes les mesures sont des
// parts de la largeur de l'image, comme au rendu : elles valent à toute taille du lecteur.
//  - Logo : largeur = taille × largeur de l'image, à 4 % du bord. Ombre portée et halo par un filtre SVG qui suit
//    le graphe ffmpeg : halo = silhouette blanche floutée (σ = 0,04 × largeur du logo × (0,6 + halo)), opacité
//    × (1 + 3 × halo) ; ombre = silhouette noire floutée (σ = 0,025 × logo), décalée de 0,03 × logo, opacité
//    × 0,9 × ombre ; puis le logo.
//  - Texte : police Anton, corps du rendu = taille × largeur (0,578 fois ce corps dans le navigateur, ligne de la
//    hauteur du corps, comme libass), espacement d'un pixel du rendu, contour de 6 % du corps, ombre d'un pixel.

const SVG = 'http://www.w3.org/2000/svg';

function el(nom, attrs = {}, parent = null) {
  const e = document.createElementNS(SVG, nom);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  parent?.append(e);
  return e;
}

/** Filtre SVG d'un logo (ombre et halo) ; largeur du logo à l'écran en pixels CSS. */
function filtreLogo(defs, id, hab, largeur) {
  const f = el('filter', { id, x: '-60%', y: '-60%', width: '220%', height: '220%', 'color-interpolation-filters': 'sRGB' }, defs);
  const fusion = [];
  if (hab.halo > 0) {
    el('feGaussianBlur', { in: 'SourceAlpha', stdDeviation: 0.04 * largeur * (0.6 + hab.halo), result: 'hb' }, f);
    const ct = el('feComponentTransfer', { in: 'hb', result: 'ha' }, f);
    el('feFuncA', { type: 'linear', slope: 1 + 3 * hab.halo }, ct);
    el('feFlood', { 'flood-color': '#fff', result: 'hc' }, f);
    el('feComposite', { in: 'hc', in2: 'ha', operator: 'in', result: 'halo' }, f);
    fusion.push('halo');
  }
  if (hab.ombre > 0) {
    el('feGaussianBlur', { in: 'SourceAlpha', stdDeviation: Math.max(0.5, 0.025 * largeur), result: 'ob' }, f);
    const d = Math.max(1, 0.03 * largeur);
    el('feOffset', { in: 'ob', dx: d, dy: d, result: 'od' }, f);
    const ct = el('feComponentTransfer', { in: 'od', result: 'oa' }, f);
    el('feFuncA', { type: 'linear', slope: 0.9 * hab.ombre }, ct);
    el('feFlood', { 'flood-color': '#000', result: 'oc' }, f);
    el('feComposite', { in: 'oc', in2: 'oa', operator: 'in', result: 'ombre' }, f);
    fusion.push('ombre');
  }
  const m = el('feMerge', {}, f);
  for (const r of [...fusion, 'SourceGraphic']) el('feMergeNode', { in: r }, m);
}

const ANCRES = {
  haut_gauche: { left: 1, top: 1 }, haut_droite: { right: 1, top: 1 }, bas_gauche: { left: 1, bottom: 1 },
  bas_droite: { right: 1, bottom: 1 }, haut: { centre: 1, top: 1 }, bas: { centre: 1, bottom: 1 },
};

/** Habillage en direct dans `conteneur` (le cadre du lecteur), calé sur l'image de `video`. */
export function creerHabillageDirect(video, conteneur) {
  const calque = document.createElement('div');
  calque.className = 'habillage-direct cache';
  const svg = el('svg', { width: 0, height: 0, style: 'position:absolute' });
  const defs = el('defs', {}, svg);
  calque.append(svg);
  // Au-dessus de la vidéo et de sa PostProd en direct (comme au rendu, l'habillage n'est pas retouché), sous le
  // calque d'édition des incrustations.
  const edition = conteneur.querySelector('.calque');
  if (edition) conteneur.insertBefore(calque, edition); else conteneur.append(calque);
  let elements = [];
  let actif = false;

  const dessiner = () => {
    const lv = video.videoWidth;
    const hv = video.videoHeight;
    const lc = conteneur.clientWidth;
    const hc = conteneur.clientHeight;
    if (!actif || !lv || !hv || !lc || !hc) return;
    const k = Math.min(lc / lv, hc / hv);
    const L = lv * k;
    const H = hv * k;
    Object.assign(calque.style, { width: `${L}px`, height: `${H}px`, left: `${(lc - L) / 2}px`, top: `${(hc - H) / 2}px` });
    const marge = 0.04 * L;
    defs.replaceChildren();
    for (const n of [...calque.children]) if (n !== svg) n.remove();
    elements.forEach((hab, i) => {
      const a = ANCRES[hab.position] || ANCRES.haut_droite;
      let n;
      if (hab.genre === 'logo' && hab.url) {
        const largeur = Math.max(16, hab.taille * L);
        n = document.createElement('img');
        n.src = hab.url;
        n.alt = '';
        n.style.width = `${largeur}px`;
        if (hab.ombre > 0 || hab.halo > 0) {
          filtreLogo(defs, `hab-direct-${i}`, hab, largeur);
          n.style.filter = `url(#hab-direct-${i})`;
        }
      } else if (hab.genre === 'texte' && hab.texte?.trim()) {
        // libass met la hauteur de la police au corps ASS : le navigateur prend 0,578 de ce corps pour Anton, et une
        // ligne de la hauteur du corps (textes-direct.js, FACTEUR). Le contour est en pixels du rendu.
        const fs = Math.max(12 * (L / 1920), hab.taille * L);
        const corps = fs * 0.578;
        const couleur = { noir: '#000', jaune: '#ffe500' }[hab.couleur] || '#fff';
        const contour = hab.couleur === 'noir' ? '#fff' : '#000';
        n = document.createElement('div');
        n.className = 'texte-habillage';
        n.textContent = hab.texte.trim();
        Object.assign(n.style, { fontSize: `${corps}px`, lineHeight: `${fs}px`, letterSpacing: `${L / 1920}px`, color: couleur, WebkitTextStroke: `${Math.max(1, fs * 0.12)}px ${contour}`,
          textShadow: `${L / 1920}px ${L / 1920}px 0 rgba(0,0,0,.5)` });
      } else return;
      n.style.opacity = hab.opacite || 0.9;
      n.style.position = 'absolute';
      if (a.left) n.style.left = `${marge}px`;
      if (a.right) n.style.right = `${marge}px`;
      // Ancré en haut, libass pose les lettres 0,09 × corps plus bas que le navigateur (mesuré le 2026-10-26).
      if (a.top) n.style.top = `${marge + (hab.genre === 'texte' ? 0.09 * Math.max(12 * (L / 1920), hab.taille * L) : 0)}px`;
      // Ancré en bas, libass pose le texte 0,11 × corps plus bas que sa boîte (textes-direct.js, placer).
      if (a.bottom) n.style.bottom = `${marge - (hab.genre === 'texte' ? 0.11 * Math.max(12 * (L / 1920), hab.taille * L) : 0)}px`;
      if (a.centre) { n.style.left = '50%'; n.style.transform = 'translateX(-50%)'; }
      calque.append(n);
    });
  };
  for (const ev of ['loadedmetadata', 'resize']) video.addEventListener(ev, dessiner);
  new ResizeObserver(dessiner).observe(conteneur);

  return {
    /** elements : l'habillage du projet, chaque logo avec l'adresse de son image (url) ; actif : à montrer. */
    regler(liste, oui) {
      elements = liste;
      actif = oui;
      calque.classList.toggle('cache', !oui);
      dessiner();
    },
  };
}
