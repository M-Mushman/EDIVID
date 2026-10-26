// Textes et images en direct sur l'aperçu : sous-titres, titres des parties, textes posés (module Texte) et images
// ou vidéos posées sur une phrase. Le serveur décide de tout le contenu (groupes de mots, instants, positions,
// entrées : /programme, mêmes calculs que le rendu) ; ce module ne fait que le dessiner, avec la mise en forme du
// rendu final (src/rendu.rs : sous_titres, textes, incrustations) :
//  - mesures en pixels du rendu (wo × ho), ramenées à la taille du lecteur ;
//  - polices du rendu (Anton, DejaVu Sans grasse), lettres à l'échelle de libass (FACTEUR, mesuré contre ffmpeg) ;
//  - entrées et sorties : mêmes durées et mêmes courbes que les filtres ffmpeg et les balises ASS.
// Les effets d'image (secousse, noir et blanc) et les médias à fond vert restent inscrits dans l'aperçu.

/** libass met la hauteur de la police (ascendante + descendante) au corps ASS ; le navigateur, la largeur du carré
 *  typographique. Corps du navigateur = corps ASS × unités par carré / (ascendante + descendante) : Anton 0,578,
 *  DejaVu Sans 0,859 (2048 / (1901 + 483)). La ligne fait alors exactement le corps ASS, comme dans libass. */
const FACTEUR = { Anton: 0.578, 'DejaVu Sans': 0.859 };

const rgba = (hex, a = 1) => `rgba(${parseInt(hex.slice(1, 3), 16)},${parseInt(hex.slice(3, 5), 16)},${parseInt(hex.slice(5, 7), 16)},${a})`;
const JAUNE = '#ffe500';

/** Un texte dans le style d'une ligne ASS : police, corps, couleurs, contour, ombre (pixels du rendu × s). */
function styler(n, { police, fs, s, couleur, contour, bord, ombre, ombreCouleur, gras = false, majuscules = false, espacement = 0 }) {
  Object.assign(n.style, {
    fontFamily: `'${police}', sans-serif`, fontSize: `${fs * FACTEUR[police] * s}px`, lineHeight: `${fs * s}px`, fontWeight: gras ? '700' : '400',
    color: couleur, whiteSpace: 'nowrap', textTransform: majuscules ? 'uppercase' : 'none', letterSpacing: `${espacement * s}px`,
    WebkitTextStroke: bord > 0 ? `${2 * bord * s}px ${contour}` : '0', paintOrder: 'stroke fill',
    textShadow: ombre > 0 ? `${ombre * s}px ${ombre * s}px 0 ${ombreCouleur}` : 'none',
  });
}

/** Position d'un texte ASS : sa ligne ancrée (an) au point (x, y) du rendu. libass pose les lettres environ 0,1 ×
 *  corps plus bas que le navigateur dans la même ligne (mesuré contre ffmpeg le 2026-10-26 : 0,09 ancré en haut,
 *  0,11 en bas) ; un texte sur pavé (fs = 0) est ancré par son pavé, sans décalage. */
function placer(n, { an, x, y, s, fs = 0 }) {
  const dx = { 1: '0%', 4: '0%', 7: '0%', 2: '-50%', 5: '-50%', 8: '-50%', 3: '-100%', 6: '-100%', 9: '-100%' }[an];
  const dy = [7, 8, 9].includes(an) ? '0%' : [1, 2, 3].includes(an) ? '-100%' : '-50%';
  const bas = ([7, 8, 9].includes(an) ? 0.09 : [1, 2, 3].includes(an) ? 0.11 : 0.1) * fs;
  Object.assign(n.style, { position: 'absolute', left: `${x * s}px`, top: `${(y + bas) * s}px` });
  n.dataset.base = `translate(${dx}, ${dy})`;
}

const lisse = (x) => Math.max(0, Math.min(1, x));

/** Textes et images en direct dans `conteneur` (le cadre du lecteur), sur l'image de `video`. */
export function creerTextesDirect(video, conteneur) {
  const calque = document.createElement('div');
  calque.className = 'textes-direct cache';
  const edition = conteneur.querySelector('.calque');
  if (edition) conteneur.insertBefore(calque, edition); else conteneur.append(calque);
  let etat = null;
  let cache = new Map();
  let rappel = null;
  let actif = false;
  // Voile des fondus d'ouverture et de fin (rendu : fade sur toute la vidéo), par-dessus le logo et les textes.
  const voile = document.createElement('div');
  voile.className = 'voile-fondu';

  /** Élément d'une entrée, créé une fois puis réutilisé tant que l'état ne change pas. */
  const element = (cle, fabriquer) => {
    let n = cache.get(cle);
    if (!n) { n = fabriquer(); n.dataset.cle = cle; calque.append(n); cache.set(cle, n); }
    return n;
  };

  const dessiner = () => {
    if (!actif || !etat) return;
    const lv = video.videoWidth;
    const hv = video.videoHeight;
    const lc = conteneur.clientWidth;
    const hc = conteneur.clientHeight;
    if (!lv || !hv || !lc || !hc) return;
    const k = Math.min(lc / lv, hc / hv);
    const L = lv * k;
    const H = hv * k;
    Object.assign(calque.style, { width: `${L}px`, height: `${H}px`, left: `${(lc - L) / 2}px`, top: `${(hc - H) / 2}px` });
    const { programme: prog, reglages: r, decalage, urlMedia } = etat;
    const wo = prog.largeur;
    const ho = prog.hauteur;
    const s = L / wo;
    const max = Math.max(wo, ho);
    const vertical = ho > wo;
    const t = video.currentTime - decalage;
    const vus = new Set();

    // Sous-titres (style « Mot » du rendu).
    const dynamique = r.style_sous_titres === 'dynamique';
    const milieu = r.position_sous_titres === 'milieu';
    // Arrondis comme dans le fichier de sous-titres du rendu (corps et marge au pixel, contour au dixième).
    const fsMot = Math.round(dynamique ? (vertical ? 0.054 : 0.075) * ho : (vertical ? 0.034 : 0.05) * ho);
    const margeV = Math.round(milieu ? 0 : (vertical ? 0.21 : 0.09) * ho);
    const policeMot = dynamique ? 'Anton' : 'DejaVu Sans';
    (prog.sous_titres_groupes || []).forEach(([debut, fin, mots], i) => {
      if (t < debut || t >= fin) return;
      const cle = `st${i}`;
      vus.add(cle);
      const n = element(cle, () => {
        const d = document.createElement('div');
        mots.forEach(([mot, fort], j) => {
          if (j) d.append(' ');
          const m = document.createElement('span');
          m.textContent = dynamique ? mot.toUpperCase() : mot;
          if (dynamique && fort) Object.assign(m.style, { color: JAUNE, fontSize: '115%' });
          d.append(m);
        });
        styler(d, { police: policeMot, fs: fsMot, s, couleur: '#fff', contour: '#000', bord: Math.round((dynamique ? 0.0036 : 0.002) * max * 10) / 10,
          ombre: dynamique ? 3 : 1, ombreCouleur: 'rgba(0,0,0,.61)', gras: !dynamique, espacement: 1 });
        placer(d, { an: milieu ? 5 : 2, x: wo / 2, y: milieu ? ho / 2 : ho - margeV, s, fs: fsMot });
        return d;
      });
      // Le mot apparaît à 85 % et prend sa taille en 90 ms (style dynamique).
      const echelle = dynamique ? 0.85 + 0.15 * lisse((t - debut) / 0.09) : 1;
      n.style.transform = `${n.dataset.base} scale(${echelle})`;
      n.style.transformOrigin = milieu ? '50% 50%' : '50% 100%';
    });

    // Titres des parties (style « Titre ») : pavé jaune en haut, de t + 0,1 à t + 1,9 s, fondu 120 / 200 ms.
    if (r.titres_chapitres !== false) {
      const fsTitre = Math.round((vertical ? 0.048 : 0.07) * ho);
      (prog.titres || []).forEach(({ t: t0, titre }, i) => {
        const a = t0 + 0.1;
        const b = t0 + 1.9;
        if (t < a || t >= b) return;
        const cle = `ti${i}`;
        vus.add(cle);
        const n = element(cle, () => {
          const d = document.createElement('div');
          d.textContent = titre;
          styler(d, { police: 'Anton', fs: fsTitre, s, couleur: '#000', bord: 0, ombre: 0, majuscules: true, espacement: 2 });
          // Pavé (BorderStyle 3) : la ligne ancrée en haut à la marge, le pavé déborde de l'épaisseur du contour.
          const p = Math.round(0.011 * max) * s;
          placer(d, { an: 8, x: wo / 2, y: Math.round(0.12 * ho), s });
          Object.assign(d.style, { background: JAUNE, padding: `${p}px`, marginTop: `${-p}px` });
          return d;
        });
        n.style.opacity = Math.min(lisse((t - a) / 0.12), lisse((b - t) / 0.2));
        n.style.transform = n.dataset.base;
      });
    }

    for (const inc of prog.incrustations || []) {
      if (t < inc.debut || t >= inc.fin || inc.effet || inc.fond_vert) continue;
      const dt = t - inc.debut;
      const reste = inc.fin - t;
      const cle = `in${inc.id}`;
      if (inc.texte?.trim()) {
        // Texte posé (styles « Texte » et « TexteBoite ») : centré sur sa position, entrée selon son effet.
        vus.add(cle);
        const fs = ho * (0.02 + 0.10 * Math.max(0.1, Math.min(1, inc.taille)));
        const [cx, cy] = inc.x != null && inc.y != null ? [inc.x, inc.y] : (CENTRES_TEXTE[inc.position] || [0.5, 0.22]);
        const boite = inc.fond === 'boite' || inc.fond === 'bandeau';
        const n = element(cle, () => {
          const d = document.createElement('div');
          const lettres = [...inc.texte.trim()];
          if (inc.entree === 'machine') lettres.forEach((c) => { const sp = document.createElement('span'); sp.textContent = c; d.append(sp); });
          else d.textContent = inc.texte.trim();
          const couleur = inc.couleur === 'jaune' ? JAUNE : inc.couleur === 'noir' ? '#000' : '#fff';
          const cadre = inc.couleur === 'noir' ? '#fff' : '#000';
          styler(d, { police: 'Anton', fs, s, couleur, contour: cadre, bord: boite ? 0 : 0.0035 * max, ombre: boite ? 0 : 2,
            ombreCouleur: rgba(cadre === '#fff' ? '#ffffff' : '#000000', 0.53), espacement: 1 });
          if (boite) {
            const p = (inc.fond === 'bandeau' ? 0.03 * ho : 0.012 * max) * s;
            Object.assign(d.style, { background: inc.couleur === 'noir' ? 'rgba(255,255,255,.69)' : 'rgba(0,0,0,.69)', padding: `${p}px` });
          }
          placer(d, { an: 5, x: cx * wo, y: cy * ho, s, fs: boite ? 0 : fs });
          return d;
        });
        let opacite = 1;
        let echelle = 1;
        let dx = 0;
        let dy = 0;
        const lin = lisse(dt / 0.3);
        switch (inc.entree) {
          case 'splash': echelle = dt < 0.16 ? 0.3 + 0.85 * (dt / 0.16) : 1.15 - 0.15 * lisse((dt - 0.16) / 0.12); opacite = lisse(reste / 0.2); break;
          case 'zoom': echelle = 0.1 + 0.9 * lisse(dt / 0.25); opacite = lisse(reste / 0.2); break;
          case 'glisse_gauche': dx = (-0.4 * wo - cx * wo) * (1 - lin); opacite = lisse(reste / 0.2); break;
          case 'glisse_droite': dx = (1.4 * wo - cx * wo) * (1 - lin); opacite = lisse(reste / 0.2); break;
          case 'glisse_haut': dy = (-0.15 * ho - cy * ho) * (1 - lin); opacite = lisse(reste / 0.2); break;
          case 'glisse_bas': dy = (1.15 * ho - cy * ho) * (1 - lin); opacite = lisse(reste / 0.2); break;
          case 'aucune': break;
          case 'machine': {
            const lettres = n.children;
            const pas = Math.min(0.07, (0.6 * Math.max(0.2, inc.fin - inc.debut)) / Math.max(1, lettres.length));
            for (let i = 0; i < lettres.length; i++) lettres[i].style.opacity = dt >= i * pas ? 1 : 0;
            break;
          }
          default: opacite = Math.min(lisse(dt / 0.25), lisse(reste / 0.25));
        }
        n.style.opacity = opacite;
        n.style.transform = `translate(${dx * s}px, ${dy * s}px) ${n.dataset.base} scale(${echelle})`;
        n.style.transformOrigin = '50% 50%';
      } else if (inc.media && urlMedia(inc.media)) {
        // Image, GIF ou vidéo posée (overlay du rendu) : largeur = taille × image, entrée et sortie du rendu.
        vus.add(cle);
        const m = urlMedia(inc.media);
        const n = element(cle, () => {
          const d = document.createElement(m.genre === 'video' ? 'video' : 'img');
          d.src = m.url;
          if (m.genre === 'video') { d.muted = true; d.playsInline = true; d.loop = true; d.preload = 'auto'; }
          d.style.position = 'absolute';
          return d;
        });
        const plein = inc.position === 'plein' && inc.x == null;
        const largeur0 = Math.max(16, Math.floor(wo * inc.taille / 2) * 2);
        let f = 1;
        if (!plein && inc.entree === 'splash') f = dt < 0.21 ? Math.max(0.1, 1.15 * dt / 0.21) : dt < 0.3 ? 1.15 - 0.15 * (dt - 0.21) / 0.09 : 1;
        if (!plein && inc.entree === 'zoom') f = dt < 0.25 ? Math.max(0.1, 1 - (1 - dt / 0.25) ** 2) : 1;
        const rapport = (n.naturalHeight || n.videoHeight || 1) / (n.naturalWidth || n.videoWidth || 1);
        const w = plein ? wo : largeur0 * f;
        const hgt = plein ? ho : w * rapport;
        const marge = Math.round(0.04 * wo);
        let x;
        let y;
        if (plein) { x = 0; y = 0; } else if (inc.x != null && inc.y != null) { x = inc.x * wo - w / 2; y = inc.y * ho - hgt / 2; } else {
          [x, y] = {
            haut_gauche: [marge, marge], haut_droite: [wo - w - marge, marge], bas_gauche: [marge, ho - hgt - marge],
            bas_droite: [wo - w - marge, ho - hgt - marge], haut: [(wo - w) / 2, marge], bas: [(wo - w) / 2, ho - hgt - marge * 6],
          }[inc.position] || [(wo - w) / 2, (ho - hgt) / 2];
        }
        const r2 = Math.max(0, 1 - dt / 0.3) ** 2;
        if (inc.entree === 'glisse_gauche') x -= (x + w) * r2;
        if (inc.entree === 'glisse_droite') x += (wo - x) * r2;
        if (inc.entree === 'glisse_haut') y -= (y + hgt) * r2;
        if (inc.entree === 'glisse_bas') y += (ho - y) * r2;
        if (inc.entree === 'secousse') {
          const amorti = Math.max(0, 1 - dt / 0.45);
          x += wo * 0.025 * Math.sin(dt * 70) * amorti;
          y += ho * 0.02 * Math.cos(dt * 83) * amorti;
        }
        const d = inc.fin - inc.debut;
        const entree = inc.entree || 'fondu';
        const opacite = entree === 'aucune' ? 1 : entree === 'fondu' ? Math.min(lisse(dt / 0.15), lisse((d - dt) / 0.15)) : lisse((d - dt) / 0.15);
        Object.assign(n.style, { left: `${x * s}px`, top: `${y * s}px`, width: `${w * s}px`, height: plein ? `${ho * s}px` : 'auto',
          objectFit: plein ? 'cover' : '', opacity: opacite });
        if (m.genre === 'video') {
          const voulu = dt % Math.max(0.1, n.duration || Infinity);
          if (Math.abs(n.currentTime - voulu) > 0.15) n.currentTime = voulu;
          if (video.paused && !n.paused) n.pause();
          if (!video.paused && n.paused) n.play().catch(() => {});
        }
      }
    }
    for (const [cle, n] of cache) n.style.display = vus.has(cle) ? '' : 'none';

    // Fondus d'ouverture et de fin, sur toute la vidéo (générique compris), comme le fade du rendu (linéaire).
    const total = video.duration || 0;
    const dF = Math.min(Math.max(Number(r.duree_fondus) || 1, 0.5), 4, total / 2);
    const tv = video.currentTime;
    let opaciteVoile = 0;
    let teinte = '#000';
    if (r.fondu_ouverture && r.fondu_ouverture !== 'aucun' && tv < dF) { opaciteVoile = 1 - tv / dF; teinte = r.fondu_ouverture === 'blanc' ? '#fff' : '#000'; }
    if (r.fondu_fin && r.fondu_fin !== 'aucun' && total && tv > total - dF) {
      opaciteVoile = Math.max(opaciteVoile, Math.min(1, (tv - (total - dF)) / dF));
      teinte = r.fondu_fin === 'blanc' ? '#fff' : '#000';
    }
    if (voile.parentNode !== calque) calque.append(voile);
    Object.assign(voile.style, { background: teinte, opacity: opaciteVoile, display: opaciteVoile > 0 ? '' : 'none' });
  };

  const boucle = () => {
    dessiner();
    rappel = actif ? requestAnimationFrame(boucle) : null;
  };
  for (const ev of ['seeked', 'loadedmetadata', 'pause']) video.addEventListener(ev, dessiner);
  new ResizeObserver(() => { cache.forEach((n) => n.remove()); cache = new Map(); dessiner(); }).observe(conteneur);

  return {
    /** etat : { programme, reglages, decalage (générique), urlMedia(id) → { url, genre } } ; actif : à montrer. */
    regler(nouveau, oui) {
      cache.forEach((n) => n.remove());
      cache = new Map();
      etat = nouveau;
      actif = oui;
      calque.classList.toggle('cache', !oui);
      if (oui && rappel === null) boucle();
      dessiner();
    },
  };
}

/** Centres nommés des textes posés (rendu : centre_nomme, textes). */
const CENTRES_TEXTE = {
  haut_gauche: [0.27, 0.16], haut_droite: [0.73, 0.16], bas_gauche: [0.27, 0.72], bas_droite: [0.73, 0.72],
  centre: [0.5, 0.5], plein: [0.5, 0.5], haut: [0.5, 0.18], bas: [0.5, 0.7],
};
