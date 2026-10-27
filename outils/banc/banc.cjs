// Banc d'essai de l'interface d'EDIVID : les vrais fichiers de web/, l'API remplacée par les données exportées
// d'un projet (vue, programme, catalogue), dans Chromium. Capture d'écran et mesures des colonnes.
// node outils/banc/banc.cjs <largeur> <hauteur> <capture.png> [onglet] ; QUALITE=1 : ouvre aussi « Ce plan en qualité finale » ;
// CLIC=<texte> : clique un bouton (une fenêtre) avant la capture.
// Playwright : EDIVID_PLAYWRIGHT=<chemin du paquet> (sinon cherché comme module installé).
const { chromium } = require(process.env.EDIVID_PLAYWRIGHT || 'playwright');
const fs = require('fs');
const path = require('path');
const RACINE = path.resolve(__dirname, '..', '..');
// Données exportées d'un projet (test ignoré cadres_d_un_projet, EDIVID_SORTIE) : jetables, hors dépôt.
const D = process.env.EDIVID_BANC_DONNEES || path.join(RACINE, 'essais', 'banc', 'donnees');
const PROJET = '9c8892ad3890';
const lire = (f) => fs.readFileSync(f);
const json = (v) => ({ status: 200, contentType: 'application/json', body: typeof v === 'string' ? v : JSON.stringify(v) });
const types = { '.js': 'text/javascript', '.css': 'text/css', '.html': 'text/html', '.ttf': 'font/ttf', '.png': 'image/png', '.mp4': 'video/mp4', '.m4a': 'audio/mp4', '.wav': 'audio/wav' };
const fichier = (f) => ({ status: 200, contentType: types[path.extname(f)] || 'application/octet-stream', body: lire(f) });

(async () => {
  const [largeur, hauteur, capture, onglet] = [Number(process.argv[2] || 1600), Number(process.argv[3] || 900), process.argv[4] || 'ui.png', process.argv[5]];
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: largeur, height: hauteur } });
  p.on('console', (m) => console.log('console :', m.text()));
  p.on('pageerror', (e) => console.log('erreur page :', e.message, (e.stack || '').split('\n').slice(1, 3).join(' / ')));
  await p.addInitScript(() => { try { localStorage.setItem('edivid_guide_vu', '1'); } catch {} });
  const vue = JSON.parse(lire(path.join(D, 'vue.json')));
  await p.route('**/*', (r) => {
    const u = new URL(r.request().url());
    const c = u.pathname;
    if (!c.startsWith('/api/')) {
      if (c.startsWith('/polices/')) return r.fulfill(fichier(path.join(RACINE, 'modeles', c)));
      const f = path.join(RACINE, 'web', c === '/' ? 'index.html' : c);
      return fs.existsSync(f) ? r.fulfill(fichier(f)) : r.fulfill({ status: 404, body: '' });
    }
    if (c === '/api/session') return r.fulfill(json({ connecte: true, compte: { email: 'lea@exemple.fr', nom: 'Léa' }, google: true }));
    if (c === '/api/catalogue') return r.fulfill(json(lire(path.join(D, 'catalogue.json')).toString()));
    if (c === '/api/etat') return r.fulfill(json({ file: [] }));
    if (c === '/api/parametres') return r.fulfill(json({ drive: { actif: false }, etat_drive: {}, comptes: [], google: { configure: true } }));
    if (c === '/api/jeux') return r.fulfill(json({ jeux: [] }));
    if (c === '/api/bibliotheque') return r.fulfill(json({ integree: JSON.parse(lire(path.join(RACINE, 'bibliotheque', 'demandes.json'))), perso: [] }));
    if (c === '/api/son/ambiances') return r.fulfill(json(lire(path.join(RACINE, 'bibliotheque', 'ambiances.json')).toString()));
    if (c === '/api/postprod/looks') return r.fulfill(json({ looks: JSON.parse(lire(path.join(RACINE, 'bibliotheque', 'looks.json'))).looks, couleurs: [{ cle: 'naturel', nom: 'Naturel' }] }));
    if (c === `/api/projets/${PROJET}`) return r.fulfill(json(vue));
    if (c === `/api/projets/${PROJET}/programme`) return r.fulfill(json(lire(path.join(D, 'programme.json')).toString()));
    // Vidéos et pistes : inutiles pour la mise en page (et trop lourdes à servir d'un bloc).
    if (c === `/api/projets/${PROJET}/plan/qualite`) return r.fulfill(json({"debut":41.53,"duree":3.1,"fichier":"qualite-20261026-171947.mp4","hauteur":1080,"largeur":1920,"plans":[{"agrandissement":3.0,"nettete":true},{"agrandissement":3.0,"nettete":true},{"agrandissement":3.0,"nettete":true}]}));
    if (c.startsWith(`/api/projets/${PROJET}/sorties/`)) { const f = path.join(RACINE, 'donnees/projets', PROJET, 'sorties', decodeURIComponent(path.basename(c))); return fs.existsSync(f) && c.includes('qualite-') ? r.fulfill(fichier(f)) : r.fulfill({ status: 404, body: '' }); }
    if (c.includes('/rushs/')) return r.fulfill({ status: 404, body: '' });
    if (c.startsWith(`/api/projets/${PROJET}/medias/`)) {
      const m = vue.projet.medias.find((x) => x.id === path.basename(c));
      return m ? r.fulfill(fichier(path.join(RACINE, 'donnees/projets', PROJET, 'medias', m.fichier))) : r.fulfill({ status: 404, body: '' });
    }
    if (c.endsWith('/dossier')) return r.fulfill(json(lire(path.join(D, 'dossier.json')).toString()));
    if (c.endsWith('/stockage')) return r.fulfill(json({ total: 40e9, categories: [
      { cle: 'rushs', titre: "Rushs d'origine", aide: '', supprimable: false, octets: 8.9e9 },
      { cle: 'travail', titre: 'Fichiers de travail', aide: 'Recréés au prochain aperçu.', supprimable: true, octets: 21.8e9 }] }));
    if (c.endsWith('/evenements')) return r.fulfill({ status: 204, body: '' });
    return r.fulfill(json({}));
  });
  await p.goto(`http://edivid.essai/#/projet/${PROJET}${process.env.PAGE ? '/' + process.env.PAGE : ''}`);
  if (process.env.PAGE) { await p.waitForTimeout(1500); await p.screenshot({ path: capture, fullPage: true }); await b.close(); return; }
  await p.waitForSelector('.atelier', { timeout: 20000 });
  if (onglet) await p.evaluate((o) => { [...document.querySelectorAll('.onglets button')].find((x) => x.title === o)?.click(); }, onglet);
  await p.waitForTimeout(1500);
  // CLIC=<texte> : clique le premier bouton portant ce texte, puis capture la fenêtre ouverte.
  if (process.env.CLIC) {
    await p.evaluate((t) => [...document.querySelectorAll('button')].find((x) => x.textContent.includes(t))?.click(), process.env.CLIC);
    await p.waitForTimeout(800);
    await p.screenshot({ path: capture }); await b.close(); return;
  }
  if (process.env.QUALITE) {
    await p.evaluate(() => { const v = document.querySelector('video'); v.dataset.url = v.dataset.url || 'x'; v.dispatchEvent(new Event('pause')); });
    await p.waitForTimeout(500);
    const b = await p.$('button:has-text("Ce plan en qualité finale")');
    console.log('bouton :', b ? await b.isEnabled() : 'absent');
    if (b) { await b.evaluate((e) => e.click()); await p.waitForTimeout(4000); }
    console.log('fenêtre :', await p.evaluate(() => !!document.querySelector('.comparaison-qualite')), await p.evaluate(() => [...document.querySelectorAll('.toast')].map((t) => t.textContent)));
    await p.screenshot({ path: capture }); await b?.dispose(); await (await p.context()).browser().close(); return;
  }
  const mesures = await p.evaluate(() => {
    const r = (s) => { const e = document.querySelector(s); if (!e) return null; const b = e.getBoundingClientRect(); return { haut: Math.round(b.top + scrollY), bas: Math.round(b.bottom + scrollY), hauteur: Math.round(b.height), largeur: Math.round(b.width) }; };
    const corps = document.querySelector('.atelier > .carte:first-child .carte-corps');
    return {
      page: document.documentElement.scrollHeight, fenetre: innerHeight,
      rushs: r('.colonne > .carte:first-child'), lecteur: r('.atelier > .carte:first-child'), texte: r('.atelier > .carte:last-child'),
      tiroir: r('.panneau-droit'), dock: r('.dock'),
      sous_la_video: corps ? [...corps.children].map((e) => `${e.className || e.tagName} ${Math.round(e.getBoundingClientRect().height)}px`) : null,
    };
  });
  console.log(JSON.stringify(mesures, null, 1));
  await p.screenshot({ path: capture, fullPage: true });
  await b.close();
})().catch((e) => { console.log('ÉCHEC', e.message); process.exit(1); });
