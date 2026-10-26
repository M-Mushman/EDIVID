// Processeurs audio du son en direct (fil audio du navigateur).
//  - « voix-edivid » : la partie directe de la chaîne de la voix (filtres ffmpeg traduits, son-dsp.js).
//  - « limiteur-edivid » : borne les crêtes à −1,5 dB, comme la normalisation du rendu final (TP −1,5).
import { creerChaine } from './son-dsp.js';

class VoixEdivid extends AudioWorkletProcessor {
  constructor(options) {
    super();
    this.chaine = null;
    this.actif = false;
    const depart = options?.processorOptions;
    if (depart?.filtres) this.chaine = creerChaine(depart.filtres, sampleRate, 2);
    if (depart && 'actif' in depart) this.actif = depart.actif;
    this.port.onmessage = (e) => {
      const d = e.data;
      if (d.filtres) {
        this.chaine = creerChaine(d.filtres, sampleRate, 2);
        if (this.chaine.inconnus.length) this.port.postMessage({ inconnus: this.chaine.inconnus });
      }
      if ('actif' in d) this.actif = d.actif;
    };
  }

  process(entrees, sorties) {
    const ent = entrees[0];
    const sor = sorties[0];
    if (!ent || !ent.length) return true;
    for (let c = 0; c < sor.length; c++) sor[c].set(ent[Math.min(c, ent.length - 1)]);
    if (this.actif && this.chaine) this.chaine.traiter(sor, sor[0].length);
    return true;
  }
}

class LimiteurEdivid extends AudioWorkletProcessor {
  constructor(options) {
    super();
    this.plafond = 10 ** (-1.5 / 20);
    this.g = 1;
    // Remontée du gain en 50 ms environ après une crête.
    this.remontee = 1 - Math.exp(-1 / (0.05 * sampleRate));
    this.actif = options?.processorOptions?.actif ?? true;
    this.port.onmessage = (e) => { if ('actif' in e.data) this.actif = e.data.actif; };
  }

  process(entrees, sorties) {
    const ent = entrees[0];
    const sor = sorties[0];
    if (!ent || !ent.length) return true;
    const n = sor[0].length;
    for (let i = 0; i < n; i++) {
      let crete = 0;
      for (let c = 0; c < ent.length; c++) crete = Math.max(crete, Math.abs(ent[c][i]));
      const voulu = this.actif && crete > this.plafond ? this.plafond / crete : 1;
      this.g = voulu < this.g ? voulu : this.g + (voulu - this.g) * this.remontee;
      for (let c = 0; c < sor.length; c++) sor[c][i] = ent[Math.min(c, ent.length - 1)][i] * this.g;
    }
    return true;
  }
}

registerProcessor('voix-edivid', VoixEdivid);
registerProcessor('limiteur-edivid', LimiteurEdivid);
