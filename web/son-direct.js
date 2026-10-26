// Son en direct de l'aperçu : la vidéo porte la voix de base (niveau, bruit, écho retiré, calage) ; le navigateur
// joue en direct la suite de la chaîne du rendu final — porte de traîne, timbre, sifflantes (filtres ffmpeg
// traduits, son-worklet.js), lieu (convolution normalisée comme afir irnorm=2) —, y mêle le fond (musique,
// bruitages) et le générique, lus en même temps que la vidéo, puis applique le gain final mesuré par ffmpeg.
//
//   vidéo ─ voix-edivid ─┬─ sec ──────────┬─ maître (gain mesuré) ─ limiteur ─┬─ sortie
//                        └─ lieu ─ dosage ─┘        fond ─┘                     │
//                                                              générique ──────┘

/** Le graphe audio, sur un contexte quelconque (en ligne ou hors ligne : les essais l'utilisent aussi). */
export async function construireGraphe(ctx, voixSource, base = '', depart = {}) {
  await ctx.audioWorklet.addModule(`${base}/son-worklet.js`);
  const n = {
    voix: new AudioWorkletNode(ctx, 'voix-edivid', { outputChannelCount: [2], processorOptions: depart }),
    sec: ctx.createGain(),
    lieu: ctx.createConvolver(),
    mouille: ctx.createGain(),
    maitre: ctx.createGain(),
    limiteur: new AudioWorkletNode(ctx, 'limiteur-edivid', { outputChannelCount: [2], processorOptions: { actif: depart.limiteur ?? true } }),
    generique: ctx.createGain(),
  };
  n.lieu.normalize = false;
  n.mouille.gain.value = 0;
  voixSource.connect(n.voix);
  n.voix.connect(n.sec).connect(n.maitre);
  n.voix.connect(n.lieu).connect(n.mouille).connect(n.maitre);
  n.maitre.connect(n.limiteur).connect(ctx.destination);
  n.generique.connect(ctx.destination);
  n.voix.port.onmessage = (e) => { if (e.data.inconnus) console.warn('Son en direct : filtres inconnus', e.data.inconnus); };
  return n;
}

/** Réponse d'un lieu, mise à l'échelle comme afir irnorm=2 : chaque canal divisé par sa norme quadratique, le
 *  plus petit facteur appliqué à tous (irlink). */
export function normaliserLieu(ctx, tampon) {
  let g = Infinity;
  for (let c = 0; c < tampon.numberOfChannels; c++) {
    const d = tampon.getChannelData(c);
    let s = 0;
    for (let i = 0; i < d.length; i++) s += d[i] * d[i];
    g = Math.min(g, 1 / Math.sqrt(s));
  }
  const copie = ctx.createBuffer(tampon.numberOfChannels, tampon.length, tampon.sampleRate);
  for (let c = 0; c < tampon.numberOfChannels; c++) {
    const d = tampon.getChannelData(c);
    const o = copie.getChannelData(c);
    for (let i = 0; i < d.length; i++) o[i] = d[i] * g;
  }
  return copie;
}

/** Règle le graphe : filtres de la voix, lieu (tampon déjà normalisé ou null), gain final (dB), actif. */
export function reglerGraphe(n, { filtres, lieu, part, gainDb, actif }) {
  n.voix.port.postMessage({ filtres: filtres || [], actif });
  if (lieu && n.lieu.buffer !== lieu) n.lieu.buffer = lieu;
  n.mouille.gain.value = actif && lieu ? part : 0;
  n.maitre.gain.value = 10 ** ((gainDb || 0) / 20);
  n.limiteur.port.postMessage({ actif });
}

/** Son en direct sur l'élément vidéo du lecteur. Le contexte audio naît au premier « play » (geste de la personne). */
export function creerSonDirect(video) {
  let ctx = null;
  let n = null;
  let demarrage = null;
  const pistes = { fond: new Audio(), generique: new Audio() };
  for (const p of Object.values(pistes)) { p.preload = 'auto'; p.crossOrigin = 'anonymous'; }
  let config = { actif: false, filtres: [], lieuUrl: '', part: 0, gainDb: 0, pistes: {} };
  const lieux = new Map();

  const appliquer = async () => {
    if (!n) return;
    let lieu = null;
    if (config.actif && config.lieuUrl) {
      if (!lieux.has(config.lieuUrl)) {
        const r = await fetch(config.lieuUrl);
        lieux.set(config.lieuUrl, normaliserLieu(ctx, await ctx.decodeAudioData(await r.arrayBuffer())));
      }
      lieu = lieux.get(config.lieuUrl);
    }
    reglerGraphe(n, { filtres: config.filtres, lieu, part: config.part, gainDb: config.actif ? config.gainDb : 0, actif: config.actif });
  };

  const demarrer = () => {
    if (!demarrage) {
      demarrage = (async () => {
        ctx = new AudioContext({ sampleRate: 48000, latencyHint: 'playback' });
        n = await construireGraphe(ctx, ctx.createMediaElementSource(video));
        ctx.createMediaElementSource(pistes.fond).connect(n.maitre);
        ctx.createMediaElementSource(pistes.generique).connect(n.generique);
        await appliquer();
      })();
    }
    return demarrage;
  };

  // Les pistes à part suivent la vidéo : lecture, pause, position, vitesse ; un écart de plus de 60 ms est rattrapé.
  const suivre = () => {
    for (const [cle, p] of Object.entries(pistes)) {
      if (!config.actif || !config.pistes[cle]) { if (!p.paused) p.pause(); continue; }
      if (Math.abs(p.currentTime - video.currentTime) > 0.06) p.currentTime = video.currentTime;
      p.playbackRate = video.playbackRate;
      if (video.paused && !p.paused) p.pause();
      if (!video.paused && p.paused) p.play().catch(() => {});
    }
  };
  video.addEventListener('play', async () => { await demarrer(); await ctx.resume(); suivre(); });
  for (const ev of ['pause', 'seeked', 'ratechange', 'waiting', 'playing']) video.addEventListener(ev, suivre);
  setInterval(() => { if (!video.paused) suivre(); }, 250);

  return {
    /** actif : l'aperçu a ses pistes à part et la personne veut le son en direct ; sinon la vidéo passe telle quelle. */
    async regler(c) {
      config = { ...config, ...c };
      for (const [cle, p] of Object.entries(pistes)) {
        const url = config.actif ? config.pistes[cle] || '' : '';
        if ((p.dataset.url || '') !== url) {
          p.dataset.url = url;
          if (url) p.src = url; else p.removeAttribute('src');
        }
      }
      await appliquer();
      suivre();
    },
  };
}
