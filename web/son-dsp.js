// Filtres audio de ffmpeg, traduits de leur code source (FFmpeg 8.0.1, libavfilter : af_biquads.c,
// af_agate.c, af_sidechaincompress.c, af_deesser.c), pour jouer en direct dans le navigateur la même chaîne
// de la voix que le rendu final. Le serveur envoie la liste des filtres (rendu::filtres_direct) ; ce module ne
// dépend d'aucune API du navigateur : node le vérifie contre ffmpeg (capteurs/../essais).
// Filtres connus : highpass, lowpass, equalizer, bass, treble, agate, acompressor, deesser, aexciter.

const enNombre = (v) => {
  if (typeof v !== 'string') return v;
  // « -22dB » : ffmpeg lit un seuil en décibels et le convertit en rapport linéaire.
  if (/db$/i.test(v)) return 10 ** (parseFloat(v) / 20);
  return parseFloat(v);
};

/** « highpass=f=70 » → { nom: 'highpass', o: { f: 70 } }. */
export function lireFiltre(texte) {
  const [nom, reste = ''] = texte.split(/=(.*)/s);
  const o = {};
  for (const morceau of reste.split(':').filter(Boolean)) {
    const [k, v] = morceau.split('=');
    o[k] = v === undefined ? v : (/^[a-z]+$/i.test(v) ? v : enNombre(v));
  }
  return { nom: nom.trim(), o };
}

const option = (o, noms, defaut) => {
  for (const n of noms) if (o[n] !== undefined) return o[n];
  return defaut;
};

// ------------------------------------------------------------------ af_biquads.c

const DEFAUTS_BIQUAD = {
  highpass: { f: 3000, w: 0.707 }, lowpass: { f: 500, w: 0.707 }, equalizer: { f: 0, w: 1 },
  bass: { f: 100, w: 0.5 }, treble: { f: 3000, w: 0.5 },
};

class Biquad {
  constructor(type, o, sr, canaux) {
    const d = DEFAUTS_BIQUAD[type];
    const f = option(o, ['f', 'frequency'], d.f);
    const largeur = option(o, ['w', 'width'], d.w);
    const genreLargeur = option(o, ['t', 'width_type'], 'q');
    const gain = option(o, ['g', 'gain'], 0);
    const A = 10 ** (gain / 40);
    const w0 = (2 * Math.PI * f) / sr;
    let alpha;
    switch (genreLargeur) {
      case 'h': alpha = Math.sin(w0) / (2 * f / largeur); break;
      case 'o': alpha = Math.sin(w0) * Math.sinh(Math.log(2) / 2 * largeur * w0 / Math.sin(w0)); break;
      case 's': alpha = Math.sin(w0) / 2 * Math.sqrt((A + 1 / A) * (1 / largeur - 1) + 2); break;
      default: alpha = Math.sin(w0) / (2 * largeur);
    }
    const c = Math.cos(w0);
    let a0, a1, a2, b0, b1, b2;
    let beta = 2 * Math.sqrt(A);
    switch (type) {
      case 'equalizer':
        a0 = 1 + alpha / A; a1 = -2 * c; a2 = 1 - alpha / A;
        b0 = 1 + alpha * A; b1 = -2 * c; b2 = 1 - alpha * A;
        break;
      case 'bass':
        beta = Math.sqrt((A * A + 1) - (A - 1) * (A - 1));
        a0 = (A + 1) + (A - 1) * c + beta * alpha;
        a1 = -2 * ((A - 1) + (A + 1) * c);
        a2 = (A + 1) + (A - 1) * c - beta * alpha;
        b0 = A * ((A + 1) - (A - 1) * c + beta * alpha);
        b1 = 2 * A * ((A - 1) - (A + 1) * c);
        b2 = A * ((A + 1) - (A - 1) * c - beta * alpha);
        break;
      case 'treble':
        beta = Math.sqrt((A * A + 1) - (A - 1) * (A - 1));
        a0 = (A + 1) - (A - 1) * c + beta * alpha;
        a1 = 2 * ((A - 1) - (A + 1) * c);
        a2 = (A + 1) - (A - 1) * c - beta * alpha;
        b0 = A * ((A + 1) + (A - 1) * c + beta * alpha);
        b1 = -2 * A * ((A - 1) + (A + 1) * c);
        b2 = A * ((A + 1) + (A - 1) * c - beta * alpha);
        break;
      case 'lowpass':
        a0 = 1 + alpha; a1 = -2 * c; a2 = 1 - alpha;
        b0 = (1 - c) / 2; b1 = 1 - c; b2 = (1 - c) / 2;
        break;
      default: // highpass
        a0 = 1 + alpha; a1 = -2 * c; a2 = 1 - alpha;
        b0 = (1 + c) / 2; b1 = -(1 + c); b2 = (1 + c) / 2;
    }
    this.k = [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0];
    // Une fréquence hors de la plage (au-delà de la moitié de la fréquence d'échantillonnage) : ffmpeg laisse passer.
    this.ignore = w0 > Math.PI || w0 <= 0 || largeur <= 0;
    this.etat = Array.from({ length: canaux }, () => new Float64Array(4));
  }

  traiter(ch, n) {
    if (this.ignore) return;
    const [b0, b1, b2, a1, a2] = this.k;
    for (let c = 0; c < ch.length; c++) {
      const x = ch[c];
      const e = this.etat[c];
      let [x1, x2, y1, y2] = e;
      for (let i = 0; i < n; i++) {
        const v = x[i];
        const y = b0 * v + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1; x1 = v; y2 = y1; y1 = y;
        x[i] = y;
      }
      e[0] = x1; e[1] = x2; e[2] = y1; e[3] = y2;
    }
  }
}

// ------------------------------------------------------------------ hermite.h

function hermite(x, x0, x1, p0, p1, m0, m1) {
  const largeur = x1 - x0;
  const t = (x - x0) / largeur;
  m0 *= largeur;
  m1 *= largeur;
  const t2 = t * t;
  const t3 = t2 * t;
  const ct2 = -3 * p0 - 2 * m0 + 3 * p1 - m1;
  const ct3 = 2 * p0 + m0 - 2 * p1 + m1;
  return ct3 * t3 + ct2 * t2 + m0 * t + p0;
}

/** Détecteur commun de la porte et du compresseur : moyenne des canaux (link=average), RMS (detection=rms). */
function detecter(ch, i) {
  let s = 0;
  for (let c = 0; c < ch.length; c++) s += Math.abs(ch[c][i]);
  s /= ch.length;
  return s * s;
}

// ------------------------------------------------------------------ af_agate.c (mode downward)

class Porte {
  constructor(o, sr) {
    this.range = option(o, ['range'], 0.06125);
    this.ratio = option(o, ['ratio'], 2);
    this.knee = option(o, ['knee'], 2.828427125);
    this.makeup = option(o, ['makeup'], 1);
    const attaque = option(o, ['attack'], 20);
    const relache = option(o, ['release'], 250);
    let seuil = option(o, ['threshold'], 0.125);
    seuil *= seuil;
    const k = Math.sqrt(this.knee);
    this.attaque = Math.min(1, 1 / (attaque * sr / 4000));
    this.relache = Math.min(1, 1 / (relache * sr / 4000));
    this.linKneeStop = seuil * k;
    this.linKneeStart = seuil / k;
    this.thres = Math.log(seuil);
    this.kneeStart = Math.log(this.linKneeStart);
    this.kneeStop = Math.log(this.linKneeStop);
    this.pente = 0;
  }

  gain(lin) {
    const slope = Math.log(lin);
    let g = (slope - this.thres) * this.ratio + this.thres;
    if (this.knee > 1 && slope > this.kneeStart) {
      g = hermite(slope, this.kneeStart, this.kneeStop, (this.kneeStart - this.thres) * this.ratio + this.thres, this.kneeStop, this.ratio, 1);
    }
    return Math.max(this.range, Math.exp(g - slope));
  }

  traiter(ch, n) {
    for (let i = 0; i < n; i++) {
      const a = detecter(ch, i);
      this.pente += (a - this.pente) * (a > this.pente ? this.attaque : this.relache);
      const g = this.pente > 0 && this.pente < this.linKneeStop ? this.gain(this.pente) : 1;
      const f = g * this.makeup;
      for (let c = 0; c < ch.length; c++) ch[c][i] *= f;
    }
  }
}

// ------------------------------------------------------------------ af_sidechaincompress.c (acompressor, downward)

class Compresseur {
  constructor(o, sr) {
    this.ratio = option(o, ['ratio'], 2);
    this.knee = option(o, ['knee'], 2.82843);
    this.makeup = option(o, ['makeup'], 1);
    this.mix = option(o, ['mix'], 1);
    const seuil = option(o, ['threshold'], 0.125);
    const attaque = option(o, ['attack'], 20);
    const relache = option(o, ['release'], 250);
    this.thres = Math.log(seuil);
    const linStart = seuil / Math.sqrt(this.knee);
    const linStop = seuil * Math.sqrt(this.knee);
    this.adjKneeStart = linStart * linStart;
    this.kneeStart = Math.log(linStart);
    this.kneeStop = Math.log(linStop);
    this.compStop = (this.kneeStop - this.thres) / this.ratio + this.thres;
    this.attaque = Math.min(1, 1 / (attaque * sr / 4000));
    this.relache = Math.min(1, 1 / (relache * sr / 4000));
    this.pente = 0;
  }

  gain(lin) {
    const slope = Math.log(lin) * 0.5;
    let g = (slope - this.thres) / this.ratio + this.thres;
    if (this.knee > 1 && slope < this.kneeStop) {
      g = hermite(slope, this.kneeStart, this.kneeStop, this.kneeStart, this.compStop, 1, 1 / this.ratio);
    }
    return Math.exp(g - slope);
  }

  traiter(ch, n) {
    for (let i = 0; i < n; i++) {
      const a = detecter(ch, i);
      this.pente += (a - this.pente) * (a > this.pente ? this.attaque : this.relache);
      const g = this.pente > 0 && this.pente > this.adjKneeStart ? this.gain(this.pente) : 1;
      const f = g * this.makeup * this.mix + (1 - this.mix);
      for (let c = 0; c < ch.length; c++) ch[c][i] *= f;
    }
  }
}

// ------------------------------------------------------------------ af_deesser.c (mode « o »)

class Sifflantes {
  constructor(o, sr, canaux) {
    const intensite = option(o, ['i'], 0);
    const max = option(o, ['m'], 0.5);
    const frequence = option(o, ['f'], 0.5);
    const echelle = sr < 44100 ? 44100 / sr : sr / 44100;
    this.intensite = intensite ** 5 * (8192 / echelle);
    this.maxdess = 1 / 10 ** (((max - 1) * 48) / 20);
    this.iir = frequence ** 2 / echelle;
    this.etat = Array.from({ length: canaux }, () => ({ s1: 0, s2: 0, s3: 0, m1: 0, m2: 0, ratioA: 1, ratioB: 1, iirA: 0, iirB: 0, flip: false }));
  }

  traiter(ch, n) {
    for (let c = 0; c < ch.length; c++) {
      const x = ch[c];
      const d = this.etat[c];
      for (let i = 0; i < n; i++) {
        let v = x[i];
        d.s3 = d.s2; d.s2 = d.s1; d.s1 = v;
        d.m1 = (d.s1 - d.s2) * ((d.s1 - d.s2) / 1.3);
        d.m2 = (d.s2 - d.s3) * ((d.s1 - d.s2) / 1.3);
        let sense = (d.m1 - d.m2) * ((d.m1 - d.m2) / 1.3);
        const vitesse = 7 + sense * 1024;
        sense = 1 + this.intensite * this.intensite * sense;
        sense = Math.min(sense, this.intensite);
        const recuperation = 1 + 0.01 / sense;
        const decalage = 1 - Math.abs(v);
        if (d.flip) {
          d.iirA = d.iirA * (1 - decalage * this.iir) + v * (decalage * this.iir);
          d.ratioA = d.ratioA < sense ? (d.ratioA * vitesse + sense) / (vitesse + 1) : 1 + (d.ratioA - 1) / recuperation;
          d.ratioA = Math.min(d.ratioA, this.maxdess);
          v = d.iirA + (v - d.iirA) / d.ratioA;
        } else {
          d.iirB = d.iirB * (1 - decalage * this.iir) + v * (decalage * this.iir);
          d.ratioB = d.ratioB < sense ? (d.ratioB * vitesse + sense) / (vitesse + 1) : 1 + (d.ratioB - 1) / recuperation;
          d.ratioB = Math.min(d.ratioB, this.maxdess);
          v = d.iirB + (v - d.iirB) / d.ratioB;
        }
        d.flip = !d.flip;
        x[i] = v;
      }
    }
  }
}

// ------------------------------------------------------------------ af_aexciter.c

const vrai0 = (x) => (Math.abs(x) > 1e-8 ? x : 0);
const racine = (x) => { x = Math.abs(x); return x > 1e-8 ? Math.sqrt(x) : 0; };

/** Excitateur d'harmoniques : fabrique des harmoniques à partir du contenu au-dessus de `freq` et les ajoute. */
class Excitateur {
  constructor(o, sr, canaux) {
    this.amount = option(o, ['amount'], 1);
    this.levelIn = option(o, ['level_in'], 1);
    this.levelOut = option(o, ['level_out'], 1);
    this.ceil = option(o, ['ceil'], 9999);
    const drive = option(o, ['drive'], 8.5);
    const blend = option(o, ['blend'], 0);
    const freq = option(o, ['freq'], 7500);
    const p = {};
    p.rdrive = 12 / drive;
    p.rbdr = p.rdrive / (10.5 - blend) * 780 / 33;
    p.kpa = racine(2 * p.rdrive * p.rdrive - 1) + 1;
    p.kpb = (2 - p.kpa) / 2;
    p.ap = (p.rdrive * p.rdrive - p.kpa + 1) / 2;
    p.kc = p.kpa / racine(2 * racine(2 * p.rdrive * p.rdrive - 1) - 2 * p.rdrive * p.rdrive);
    p.srct = (0.1 * sr) / (0.1 * sr + 1);
    p.sq = p.kc * p.kc + 1;
    p.knb = -1 * p.rbdr / racine(p.sq);
    p.kna = 2 * p.kc * p.rbdr / racine(p.sq);
    p.an = p.rbdr * p.rbdr / p.sq;
    p.imr = 2 * p.knb + racine(2 * p.kna + 4 * p.an - 1);
    p.pwrq = 2 / (p.imr + 1);
    const coeffs = (f, haut) => {
      const w0 = (2 * Math.PI * f) / sr;
      const alpha = Math.sin(w0) / (2 * 0.707);
      const a0 = 1 + alpha; const a1 = -2 * Math.cos(w0); const a2 = 1 - alpha;
      const b0 = haut ? (1 + Math.cos(w0)) / 2 : (1 - Math.cos(w0)) / 2;
      const b1 = haut ? -(1 + Math.cos(w0)) : 1 - Math.cos(w0);
      return [-a1 / a0, -a2 / a0, b0 / a0, b1 / a0, b0 / a0];
    };
    this.hp = coeffs(freq, true);
    this.lp = coeffs(this.ceil, false);
    this.p = p;
    this.etat = Array.from({ length: canaux }, () => ({ prevMed: 0, prevOut: 0, hw: [[0, 0], [0, 0], [0, 0], [0, 0]], lw: [[0, 0], [0, 0]] }));
  }

  static b(entree, c, w) {
    const sortie = c[2] * entree + w[0];
    w[0] = c[3] * entree + w[1] + c[0] * sortie;
    w[1] = c[4] * entree + c[1] * sortie;
    return sortie;
  }

  distorsion(e, x) {
    const p = this.p;
    let proc = Excitateur.b(Excitateur.b(x, this.hp, e.hw[0]), this.hp, e.hw[1]);
    const med = proc >= 0 ? (racine(p.ap + proc * (p.kpa - proc)) + p.kpb) * p.pwrq : (racine(p.an - proc * (p.kna + proc)) + p.knb) * p.pwrq * -1;
    proc = p.srct * (med - e.prevMed + e.prevOut);
    e.prevMed = vrai0(med);
    e.prevOut = vrai0(proc);
    proc = Excitateur.b(Excitateur.b(proc, this.hp, e.hw[2]), this.hp, e.hw[3]);
    if (this.ceil >= 10000) proc = Excitateur.b(Excitateur.b(proc, this.lp, e.lw[0]), this.lp, e.lw[1]);
    return proc;
  }

  traiter(ch, n) {
    for (let i = 0; i < n; i++) {
      for (let c = 0; c < ch.length; c++) {
        const src = ch[c][i];
        const v = this.distorsion(this.etat[c], src * this.levelIn);
        ch[c][i] = (v * this.amount + src) * this.levelOut;
      }
    }
  }
}

/** Chaîne de filtres, à `sr` Hz sur `canaux` canaux. Un filtre inconnu est signalé et laissé de côté. */
export function creerChaine(filtres, sr, canaux = 2) {
  const etapes = [];
  const inconnus = [];
  for (const texte of filtres) {
    const { nom, o } = lireFiltre(texte);
    if (nom in DEFAUTS_BIQUAD) etapes.push(new Biquad(nom, o, sr, canaux));
    else if (nom === 'agate') etapes.push(new Porte(o, sr));
    else if (nom === 'acompressor') etapes.push(new Compresseur(o, sr));
    else if (nom === 'deesser') etapes.push(new Sifflantes(o, sr, canaux));
    else if (nom === 'aexciter') etapes.push(new Excitateur(o, sr, canaux));
    else inconnus.push(nom);
  }
  return {
    inconnus,
    /** Traite sur place des canaux (tableaux de n échantillons). */
    traiter(ch, n) { for (const e of etapes) e.traiter(ch, n); },
  };
}
