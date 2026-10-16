// EDIVID — interface. JavaScript sans dépendance : tout passe par l'API du serveur local.

const $app = document.getElementById('app');

// ------------------------------------------------------------------ outils

function h(tag, attrs, ...enfants) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v === null || v === undefined || v === false) continue;
    if (k === 'class') el.className = v;
    else if (k === 'style' && typeof v === 'object') Object.assign(el.style, v);
    else if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2), v);
    else if (k === 'html') el.innerHTML = v;
    else if (v === true) el.setAttribute(k, '');
    else el.setAttribute(k, v);
  }
  for (const e of enfants.flat(Infinity)) {
    if (e === null || e === undefined || e === false) continue;
    el.append(e instanceof Node ? e : document.createTextNode(String(e)));
  }
  return el;
}

const TRACES = {
  lecture: '<path d="M7 5l12 7-12 7z" fill="currentColor" stroke="none"/>',
  televerser: '<path d="M12 16V4M7 9l5-5 5 5M4 20h16"/>',
  film: '<path d="M4 4h16v16H4zM8 4v16M16 4v16M4 9h4M4 15h4M16 9h4M16 15h4"/>',
  ciseaux: '<circle cx="6" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M8.6 7.6L20 18M8.6 16.4L20 6"/>',
  discussion: '<path d="M4 5h16v11H9l-5 4z"/>',
  reglages: '<path d="M4 7h10M18 7h2M16 5v4M4 17h4M12 17h8M10 15v4"/>',
  image: '<path d="M4 5h16v14H4zM4 15l4-4 4 4 3-3 5 5"/><circle cx="15" cy="9.5" r="1.5"/>',
  etincelles: '<path d="M12 3l1.8 4.7 4.7 1.8-4.7 1.8L12 16l-1.8-4.7-4.7-1.8 4.7-1.8zM19 15l.8 2.2 2.2.8-2.2.8L19 21l-.8-2.2-2.2-.8 2.2-.8z"/>',
  corbeille: '<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"/>',
  haut: '<path d="M12 19V5M6 11l6-6 6 6"/>',
  bas: '<path d="M12 5v14M6 13l6 6 6-6"/>',
  telecharger: '<path d="M12 4v12M7 11l5 5 5-5M4 20h16"/>',
  relancer: '<path d="M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7"/>',
  aide: '<circle cx="12" cy="12" r="9"/><path d="M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6V14M12 17.5v.01"/>',
  fermer: '<path d="M6 6l12 12M18 6L6 18"/>',
  ok: '<path d="M5 12l5 5 9-10"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  retour: '<path d="M15 5l-7 7 7 7"/>',
  envoyer: '<path d="M4 12l16-8-6 16-3-7z"/>',
  musique: '<path d="M9 18V6l10-2v12"/><circle cx="6" cy="18" r="3"/><circle cx="16" cy="16" r="3"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5v.01"/>',
  soleil: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
  lune: '<path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"/>',
  baguette: '<path d="M5 19L16 8M14 6l4 4M18 3v3M21 6h-3M10 3v2M3 10h2"/>',
  alerte: '<path d="M12 3l10 18H2zM12 10v5M12 18v.01"/>',
};

function icone(nom) {
  const s = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  s.setAttribute('viewBox', '0 0 24 24');
  s.setAttribute('fill', 'none');
  s.setAttribute('stroke', 'currentColor');
  s.setAttribute('stroke-width', '2');
  s.setAttribute('stroke-linecap', 'round');
  s.setAttribute('stroke-linejoin', 'round');
  s.setAttribute('aria-hidden', 'true');
  s.innerHTML = TRACES[nom] || '';
  return s;
}

// replaceChildren écrit « null » pour un enfant nul : on filtre d'abord.
function remplir(el, ...enfants) {
  el.replaceChildren(...enfants.flat(Infinity).filter((e) => e !== null && e !== undefined && e !== false));
}

function toast(texte, genre = '') {
  const t = h('div', { class: `toast ${genre}` }, texte);
  document.getElementById('toasts').append(t);
  setTimeout(() => t.remove(), genre === 'erreur' ? 7000 : 3500);
}

const fr = (n, d = 1) => Number(n).toLocaleString('fr-FR', { maximumFractionDigits: d });
function minutes(s) {
  s = Math.max(0, Math.round(s || 0));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}
function taille(o) {
  if (o > 1e9) return `${fr(o / 1e9)} Go`;
  if (o > 1e6) return `${fr(o / 1e6, 0)} Mo`;
  return `${fr(o / 1e3, 0)} ko`;
}

function stocker(cle, valeur) { try { localStorage.setItem(cle, valeur); } catch { /* navigation privée */ } }
function lire(cle) { try { return localStorage.getItem(cle); } catch { return null; } }

class ErreurApi extends Error {}

async function api(methode, chemin, corps) {
  const r = await fetch(chemin, {
    method: methode,
    headers: corps !== undefined ? { 'Content-Type': 'application/json' } : {},
    body: corps !== undefined ? JSON.stringify(corps) : undefined,
    credentials: 'same-origin',
  });
  if (r.status === 401 && chemin !== '/api/connexion') {
    afficherConnexion();
    throw new ErreurApi('session expirée');
  }
  const v = await r.json().catch(() => ({}));
  if (!r.ok) throw new ErreurApi(v.erreur || `erreur ${r.status}`);
  return v;
}

function appliquerTheme() {
  const t = lire('edivid_theme');
  if (t === 'clair') document.documentElement.dataset.theme = 'light';
  else if (t === 'sombre') document.documentElement.dataset.theme = 'dark';
  else delete document.documentElement.dataset.theme;
}
function boutonTheme() {
  const sombre = document.documentElement.dataset.theme === 'dark' ||
    (!document.documentElement.dataset.theme && matchMedia('(prefers-color-scheme: dark)').matches);
  return h('button', {
    class: 'btn icone fantome', title: sombre ? 'Passer en thème clair' : 'Passer en thème sombre',
    onclick: () => { stocker('edivid_theme', sombre ? 'clair' : 'sombre'); appliquerTheme(); route(); },
  }, icone(sombre ? 'soleil' : 'lune'));
}

function logo() {
  return h('span', { class: 'logo' }, h('span', { class: 'logo-marque' }, icone('lecture')), 'EDIVID');
}

function fenetre(titre, contenu, { large = false } = {}) {
  const fermer = () => voile.remove();
  const voile = h('div', { class: 'voile', onclick: (e) => { if (e.target === voile) fermer(); } },
    h('div', { class: 'carte fenetre', role: 'dialog', 'aria-modal': 'true', style: large ? { width: 'min(760px, 100%)' } : null },
      h('div', { class: 'fenetre-tete' }, h('h2', {}, titre),
        h('button', { class: 'btn icone fantome', title: 'Fermer', onclick: fermer }, icone('fermer'))),
      typeof contenu === 'function' ? contenu(fermer) : contenu));
  document.addEventListener('keydown', function echap(e) {
    if (e.key === 'Escape') { fermer(); document.removeEventListener('keydown', echap); }
  });
  document.body.append(voile);
  return fermer;
}

function confirmer(titre, texte, libelle = 'Confirmer') {
  return new Promise((ok) => {
    fenetre(titre, (fermer) => h('div', {},
      h('p', { class: 'muet' }, texte),
      h('div', { style: { display: 'flex', gap: '8px', justifyContent: 'flex-end', marginTop: '18px' } },
        h('button', { class: 'btn', onclick: () => { fermer(); ok(false); } }, 'Annuler'),
        h('button', { class: 'btn primaire', onclick: () => { fermer(); ok(true); } }, libelle))));
  });
}

// ------------------------------------------------------------------ état

const E = {
  catalogue: null,
  projetId: null,
  vue: null,
  etat: null,
  onglet: 'assistant',
  lecteur: 'apercu',
  filtre: 'toutes',
  progression: null,
  televersements: new Map(),
  attenteReponse: false,
  aMettreAJour: false,
  source: null,
  noeuds: {},
};

// ------------------------------------------------------------------ routage

window.addEventListener('hashchange', route);
appliquerTheme();
route();

async function route() {
  let session;
  try { session = await api('GET', '/api/session'); } catch { session = { connecte: false }; }
  if (!session.connecte) return afficherConnexion();
  if (!E.catalogue) E.catalogue = await api('GET', '/api/catalogue');
  const m = location.hash.match(/^#\/projet\/([a-z0-9]+)/);
  if (m) return ouvrirProjet(m[1]);
  fermerFlux();
  E.projetId = null;
  afficherAccueil();
}

function fermerFlux() {
  if (E.source) { E.source.close(); E.source = null; }
}

// ------------------------------------------------------------------ connexion

function afficherConnexion() {
  fermerFlux();
  const champ = h('input', { class: 'champ code', placeholder: 'XXXX-XXXX', autocomplete: 'one-time-code', maxlength: '12', 'aria-label': "Code d'accès" });
  const erreur = h('p', { class: 'petit', style: { color: 'var(--erreur)', minHeight: '20px', margin: '8px 0 0' } });
  const valider = async (e) => {
    e.preventDefault();
    try {
      await api('POST', '/api/connexion', { code: champ.value });
      route();
    } catch (err) { erreur.textContent = err.message === 'code incorrect' ? 'Ce code ne correspond pas. Vérifiez-le et réessayez.' : err.message; }
  };
  remplir($app, h('div', { class: 'centre' },
    h('form', { class: 'carte connexion', onsubmit: valider },
      logo(),
      h('h1', { style: { margin: '22px 0 6px' } }, 'Bienvenue'),
      h('p', { class: 'muet', style: { margin: '0 0 18px' } },
        "EDIVID monte vos vidéos face caméra : vous déposez vos rushs, le LLM propose un montage rythmé, vous l'ajustez en discutant."),
      h('label', { class: 'petit', style: { fontWeight: 600, display: 'block', marginBottom: '6px' } }, "Code d'accès"),
      champ, erreur,
      h('button', { class: 'btn primaire', type: 'submit', style: { width: '100%', marginTop: '10px' } }, 'Entrer'),
      h('p', { class: 'tres-petit muet', style: { marginTop: '14px' } },
        "Le code vous a été donné par la personne qui gère cet ordinateur. Il n'est demandé qu'une fois par navigateur."))));
  champ.focus();
}

// ------------------------------------------------------------------ accueil

async function afficherAccueil() {
  const projets = await api('GET', '/api/projets');
  const etapes = [
    ['Déposez vos rushs', "Une vidéo entière ou plusieurs prises, directement depuis l'iPhone (HEVC, HDR acceptés)."],
    ['EDIVID les analyse', 'Transcription mot à mot, repérage du visage et des silences.'],
    ['Le LLM monte', "Il coupe, choisit les zooms et le rythme, puis relit son propre montage."],
    ['Vous ajustez', 'En discutant avec lui ou d’un clic, puis vous exportez la vidéo finale.'],
  ];
  remplir($app, h('div', { class: 'accueil' },
    h('div', { style: { display: 'flex', alignItems: 'center', gap: '12px' } }, logo(), h('span', { class: 'espace' }), boutonTheme()),
    h('div', { style: { display: 'flex', alignItems: 'end', justifyContent: 'space-between', gap: '12px', marginTop: '28px', flexWrap: 'wrap' } },
      h('div', {}, h('h1', {}, 'Vos projets'),
        h('p', { class: 'muet', style: { margin: '4px 0 0' } }, 'Un projet = une vidéo à monter.')),
      h('button', { class: 'btn primaire', onclick: nouveauProjet }, icone('plus'), 'Nouveau projet')),
    h('div', { class: 'etapes-accueil' }, etapes.map(([t, d], i) =>
      h('div', { class: 'carte etape-accueil' }, h('div', { class: 'num' }, i + 1), h('h3', {}, t), h('p', { class: 'petit muet', style: { margin: '4px 0 0' } }, d)))),
    projets.length === 0
      ? h('div', { class: 'carte carte-corps', style: { textAlign: 'center', padding: '36px 16px' } },
          h('h2', {}, 'Aucun projet pour l’instant'),
          h('p', { class: 'muet' }, 'Créez votre premier projet pour déposer une vidéo.'),
          h('button', { class: 'btn primaire', onclick: nouveauProjet }, icone('plus'), 'Créer un projet'))
      : h('div', { class: 'grille-projets' }, projets.map((p) =>
          h('button', { class: 'carte projet-carte', onclick: () => { location.hash = `#/projet/${p.id}`; } },
            h('h2', {}, p.nom),
            h('p', { class: 'tres-petit muet', style: { margin: '4px 0 0' } }, `Créé le ${new Date(p.cree).toLocaleDateString('fr-FR')}`),
            h('div', { class: 'pastilles' },
              h('span', { class: 'pastille' }, `${p.rushs} rush${p.rushs > 1 ? 's' : ''}`),
              p.monte ? h('span', { class: 'pastille accent' }, 'Monté') : null,
              p.rendu ? h('span', { class: 'pastille ok' }, 'Rendu prêt') : null))))));
}

function nouveauProjet() {
  fenetre('Nouveau projet', (fermer) => {
    const champ = h('input', { class: 'champ', placeholder: 'Par exemple : Présentation de la chaîne', maxlength: '80' });
    setTimeout(() => champ.focus(), 30);
    return h('form', {
      onsubmit: async (e) => {
        e.preventDefault();
        const { id } = await api('POST', '/api/projets', { nom: champ.value });
        fermer();
        location.hash = `#/projet/${id}`;
      },
    },
    h('label', { class: 'petit', style: { fontWeight: 600, display: 'block', marginBottom: '6px' } }, 'Nom du projet'),
    champ,
    h('div', { style: { display: 'flex', justifyContent: 'flex-end', gap: '8px', marginTop: '16px' } },
      h('button', { class: 'btn', type: 'button', onclick: fermer }, 'Annuler'),
      h('button', { class: 'btn primaire', type: 'submit' }, 'Créer')));
  });
}

// ------------------------------------------------------------------ projet

async function ouvrirProjet(id) {
  if (E.projetId !== id) {
    fermerFlux();
    E.projetId = id;
    E.vue = null;
    E.progression = null;
    E.aMettreAJour = false;
    E.onglet = 'assistant';
    construireProjet();
    E.source = new EventSource(`/api/projets/${id}/evenements`);
    E.source.addEventListener('progression', (e) => {
      E.progression = JSON.parse(e.data).donnees;
      dessinerBarre();
      if (E.vue?.projet.rushs.some((r) => ['preparation', 'analyse'].includes(r.etat))) dessinerRushs();
    });
    E.source.addEventListener('projet', () => rafraichirBientot());
    E.source.addEventListener('file', () => rafraichirEtat());
    E.source.addEventListener('erreur', (e) => {
      const d = JSON.parse(e.data).donnees;
      toast(`${d.travail} : ${d.message}`, 'erreur');
      E.progression = null;
      rafraichir();
    });
    E.source.addEventListener('fini', (e) => {
      const d = JSON.parse(e.data).donnees;
      E.progression = null;
      if (d.genre === 'rendu') { E.lecteur = 'rendu'; toast('Le rendu final est prêt : vous pouvez le télécharger.', 'ok'); }
      else { E.lecteur = 'apercu'; E.aMettreAJour = false; toast('Nouvel aperçu prêt.', 'ok'); }
      rafraichir();
    });
  }
  await rafraichir();
  await rafraichirEtat();
  if (!lire('edivid_guide_vu')) { stocker('edivid_guide_vu', '1'); afficherGuide(); }
}

let minuterie = null;
function rafraichirBientot() {
  clearTimeout(minuterie);
  minuterie = setTimeout(rafraichir, 150);
}

async function rafraichir() {
  if (!E.projetId) return;
  try {
    E.vue = await api('GET', `/api/projets/${E.projetId}`);
  } catch (e) {
    if (e.message === 'projet introuvable') { location.hash = '#/'; }
    return;
  }
  dessinerTout();
}

async function rafraichirEtat() {
  try { E.etat = await api('GET', '/api/etat'); } catch { return; }
  const miens = (E.etat.file || []).filter((t) => t.projet === E.projetId);
  if (!miens.some((t) => t.en_cours)) E.progression = null;
  dessinerBarre();
}

function construireProjet() {
  const n = E.noeuds = {};
  n.nom = h('input', {
    class: 'nom-projet', 'aria-label': 'Nom du projet', maxlength: '80',
    onchange: async () => { try { await api('PATCH', `/api/projets/${E.projetId}`, { nom: n.nom.value }); } catch (e) { toast(e.message, 'erreur'); } },
  });
  n.parcours = h('div', { class: 'parcours' });
  n.rushs = h('div', { class: 'carte' });
  n.lecteur = h('div', { class: 'carte' });
  n.phrases = h('div', { class: 'carte' });
  n.panneau = h('div', { class: 'carte panneau-droit' });
  n.barre = h('div', { class: 'barre-actions-int' });
  remplir($app, 
    h('header', { class: 'barre-haut' }, h('div', { class: 'barre-haut-int' },
      h('button', { class: 'btn icone fantome', title: 'Retour aux projets', onclick: () => { location.hash = '#/'; } }, icone('retour')),
      h('span', { class: 'masque-mobile' }, logo()),
      n.nom,
      h('span', { class: 'espace' }),
      h('button', { class: 'btn petit fantome', onclick: afficherGuide, title: 'Comment ça marche' }, icone('aide'), h('span', { class: 'masque-mobile' }, 'Guide')),
      boutonTheme(),
      h('button', { class: 'btn icone fantome danger', title: 'Supprimer le projet', onclick: supprimerProjet }, icone('corbeille')))),
    n.parcours,
    h('main', { class: 'mise-en-page' },
      h('div', { class: 'colonne' }, n.rushs, n.lecteur, n.phrases),
      n.panneau),
    h('div', { class: 'barre-actions' }, n.barre));
  construireLecteur();
  construirePanneau();
}

async function supprimerProjet() {
  const nom = E.vue?.projet?.nom || 'ce projet';
  if (!(await confirmer('Supprimer le projet ?', `« ${nom} », ses rushs, ses médias et ses rendus seront effacés de cet ordinateur. C’est définitif.`, 'Supprimer'))) return;
  try {
    await api('DELETE', `/api/projets/${E.projetId}`);
    location.hash = '#/';
  } catch (e) { toast(e.message, 'erreur'); }
}

function dessinerTout() {
  const p = E.vue.projet;
  if (document.activeElement !== E.noeuds.nom) E.noeuds.nom.value = p.nom;
  document.title = `${p.nom} — EDIVID`;
  dessinerParcours();
  dessinerRushs();
  dessinerLecteur();
  dessinerPhrases();
  dessinerPanneau();
  dessinerBarre();
}

function dessinerParcours() {
  const p = E.vue.projet;
  const prets = p.rushs.length > 0 && p.rushs.every((r) => r.etat === 'pret');
  const pas = [
    ['Rushs', p.rushs.length > 0],
    ['Analyse', prets],
    ['Montage', !!p.montage],
    ['Ajustements', !!p.montage && (p.discussion.length > 0 || !!p.apercu)],
    ['Export', !!p.rendu],
  ];
  const actif = pas.findIndex(([, fait]) => !fait);
  remplir(E.noeuds.parcours, ...pas.map(([t, fait], i) =>
    h('div', { class: `pas ${fait ? 'fait' : ''} ${i === actif ? 'actif' : ''}` },
      h('span', { class: 'rond' }, fait ? '✓' : i + 1), t)));
}

// ------------------------------------------------------------------ rushs

const ETATS_RUSH = {
  televerse: ['En file', 'pastille'],
  preparation: ['Préparation', 'pastille alerte'],
  prepare: ['Préparé', 'pastille'],
  analyse: ['Analyse', 'pastille alerte'],
  pret: ['Prêt', 'pastille ok'],
  erreur: ['Erreur', 'pastille erreur'],
};

function choisirFichiers(accepte, multiple, rappel) {
  const i = h('input', { type: 'file', accept: accepte, multiple, style: { display: 'none' } });
  i.addEventListener('change', () => { rappel([...i.files]); i.remove(); });
  document.body.append(i);
  i.click();
}

function zoneDepot(texte, sous, accepte, rappel) {
  const z = h('div', { class: 'depot', tabindex: '0', role: 'button',
    onclick: () => choisirFichiers(accepte, true, rappel),
    onkeydown: (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); choisirFichiers(accepte, true, rappel); } },
    ondragover: (e) => { e.preventDefault(); z.classList.add('survol'); },
    ondragleave: () => z.classList.remove('survol'),
    ondrop: (e) => { e.preventDefault(); z.classList.remove('survol'); rappel([...e.dataTransfer.files]); },
  }, icone('televerser'), h('strong', {}, texte), h('span', { class: 'petit' }, sous));
  return z;
}

let fileTeleversement = Promise.resolve();
function televerser(fichiers, genre) {
  for (const f of fichiers) {
    const cle = `${Date.now()}-${Math.random()}`;
    E.televersements.set(cle, { nom: f.name, pct: 0, genre, taille: f.size });
    fileTeleversement = fileTeleversement.then(() => new Promise((fini) => {
      const x = new XMLHttpRequest();
      x.open('PUT', `/api/projets/${E.projetId}/${genre}?nom=${encodeURIComponent(f.name)}`);
      x.upload.onprogress = (e) => {
        if (e.lengthComputable) { E.televersements.get(cle).pct = (100 * e.loaded) / e.total; dessinerTeleversements(); }
      };
      x.onload = () => {
        E.televersements.delete(cle);
        if (x.status >= 400) {
          let m = `erreur ${x.status}`;
          try { m = JSON.parse(x.responseText).erreur || m; } catch { /* réponse vide */ }
          toast(`${f.name} : ${m}`, 'erreur');
        } else {
          toast(genre === 'rushs' ? `${f.name} déposé : préparation puis analyse automatiques.` : `${f.name} ajouté à la médiathèque.`, 'ok');
        }
        rafraichir();
        fini();
      };
      x.onerror = () => { E.televersements.delete(cle); toast(`${f.name} : envoi interrompu`, 'erreur'); dessinerTeleversements(); fini(); };
      x.send(f);
    }));
  }
  dessinerTeleversements();
}

function dessinerTeleversements() {
  for (const [genre, noeud] of [['rushs', E.noeuds.televersementsRushs], ['medias', E.noeuds.televersementsMedias]]) {
    if (!noeud) continue;
    remplir(noeud, ...[...E.televersements.values()].filter((t) => t.genre === genre).map((t) =>
      h('div', { class: 'rush' },
        h('div', { class: 'vignette' }, icone('televerser')),
        h('div', { style: { minWidth: 0 } }, h('div', { class: 'titre' }, t.nom),
          h('div', { class: 'tres-petit muet' }, `Envoi… ${fr(t.pct, 0)} % de ${taille(t.taille)}`),
          h('div', { class: 'jauge' }, h('div', { style: { width: `${t.pct}%` } }))),
        h('span'))));
  }
}

function dessinerRushs() {
  const p = E.vue.projet;
  E.noeuds.televersementsRushs = h('div', { class: 'liste-rushs' });
  const deplacer = async (i, d) => {
    const ids = p.rushs.map((r) => r.id);
    [ids[i], ids[i + d]] = [ids[i + d], ids[i]];
    await api('POST', `/api/projets/${E.projetId}/rushs/ordre`, { ids });
  };
  const progRush = (r) => (E.progression && ['preparation', 'analyse'].includes(r.etat) && E.etat?.file?.some((t) => t.en_cours && t.projet === E.projetId))
    ? E.progression : null;
  remplir(E.noeuds.rushs, 
    h('div', { class: 'carte-tete' },
      h('h2', {}, '1. Vos rushs'),
      p.rushs.length > 1 ? h('span', { class: 'tres-petit muet' }, 'Montés dans cet ordre') : null),
    h('div', { class: 'carte-corps' },
      p.rushs.length === 0 ? h('div', { class: 'aide', style: { marginBottom: '12px' } }, icone('info'),
        h('span', {}, "Déposez une vidéo entière, ou plusieurs prises : EDIVID les prépare (rotation, couleurs HDR de l'iPhone, cadence), puis les transcrit et repère le visage. Comptez environ une minute d’analyse pour cinq minutes de vidéo.")) : null,
      zoneDepot('Glissez vos vidéos ici', 'ou cliquez pour les choisir — MOV, MP4, HEVC, HDR', 'video/*,.mov,.mp4,.m4v,.mkv', (f) => televerser(f, 'rushs')),
      h('div', { class: 'liste-rushs' }, p.rushs.map((r, i) => {
        const [libelle, classe] = ETATS_RUSH[r.etat] || [r.etat, 'pastille'];
        const prog = progRush(r);
        const s = r.source;
        return h('div', { class: 'rush' },
          h('div', { class: 'vignette' }, icone('film')),
          h('div', { style: { minWidth: 0 } },
            h('div', { class: 'titre', title: r.nom }, r.nom),
            h('div', { class: 'tres-petit muet', style: { display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap', marginTop: '2px' } },
              h('span', { class: classe }, libelle),
              s ? `${minutes(s.duree)} · ${s.largeur}×${s.hauteur}${s.hdr ? ` · HDR ${s.hdr} converti` : ''}` : taille(r.taille)),
            r.erreur ? h('div', { class: 'tres-petit', style: { color: 'var(--erreur)', marginTop: '4px' } }, r.erreur) : null,
            prog ? h('div', {}, h('div', { class: 'tres-petit muet', style: { marginTop: '4px' } }, `${prog.etape} — ${fr(prog.pct, 0)} %`),
              h('div', { class: 'jauge' }, h('div', { style: { width: `${prog.pct}%` } }))) : null),
          h('div', { class: 'actions' },
            r.etat === 'erreur' ? h('button', { class: 'btn icone fantome', title: 'Relancer', onclick: () => api('POST', `/api/projets/${E.projetId}/rushs/${r.id}/relancer`) }, icone('relancer')) : null,
            i > 0 ? h('button', { class: 'btn icone fantome', title: 'Monter plus tôt', onclick: () => deplacer(i, -1) }, icone('haut')) : null,
            i < p.rushs.length - 1 ? h('button', { class: 'btn icone fantome', title: 'Monter plus tard', onclick: () => deplacer(i, 1) }, icone('bas')) : null,
            h('button', { class: 'btn icone fantome danger', title: 'Retirer ce rush', onclick: async () => {
              if (await confirmer('Retirer ce rush ?', `« ${r.nom} » et son analyse seront effacés.`, 'Retirer')) {
                try { await api('DELETE', `/api/projets/${E.projetId}/rushs/${r.id}`); } catch (e) { toast(e.message, 'erreur'); }
              }
            } }, icone('corbeille'))));
      })),
      E.noeuds.televersementsRushs));
  dessinerTeleversements();
}

// ------------------------------------------------------------------ lecteur et rythme

function construireLecteur() {
  const n = E.noeuds;
  n.video = h('video', { controls: true, playsinline: true, preload: 'metadata' });
  n.video.addEventListener('timeupdate', suivreLecture);
  n.ecran = h('div', { class: 'lecteur' }, n.video);
  n.ongletsLecteur = h('div', { class: 'onglets-petits' });
  n.rythme = h('div', { class: 'rythme' });
  remplir(n.lecteur, 
    h('div', { class: 'carte-tete' }, h('h2', {}, '2. Aperçu du montage'), n.ongletsLecteur),
    h('div', { class: 'carte-corps' }, n.ecran, n.rythme));
}

function sortieAffichee() {
  const p = E.vue.projet;
  if (E.lecteur === 'rendu' && p.rendu) return p.rendu;
  return p.apercu || (E.lecteur === 'apercu' ? p.rendu : null);
}

function dessinerLecteur() {
  const p = E.vue.projet;
  const n = E.noeuds;
  remplir(n.ongletsLecteur, 
    h('button', { class: E.lecteur === 'apercu' ? 'actif' : '', onclick: () => { E.lecteur = 'apercu'; dessinerLecteur(); } }, 'Aperçu'),
    h('button', { class: E.lecteur === 'rendu' ? 'actif' : '', disabled: !p.rendu, title: p.rendu ? '' : 'Pas encore de rendu final', onclick: () => { E.lecteur = 'rendu'; dessinerLecteur(); } }, 'Rendu final'));
  const s = sortieAffichee();
  const vide = n.ecran.querySelector('.vide');
  if (!s) {
    n.video.classList.add('cache');
    n.video.removeAttribute('src');
    const message = !p.rushs.length ? 'Déposez un rush pour commencer.'
      : !E.vue.pret_a_monter ? 'Analyse des rushs en cours…'
        : !p.montage ? 'Les rushs sont prêts. Cliquez sur « Proposer un montage » en bas de l’écran.'
          : 'Le premier aperçu se prépare…';
    if (vide) vide.lastChild.textContent = message;
    else n.ecran.append(h('div', { class: 'vide' }, icone('film'), h('p', {}, message)));
    n.ecran.classList.remove('vertical');
    remplir(n.rythme);
    return;
  }
  vide?.remove();
  n.video.classList.remove('cache');
  const url = `/api/projets/${E.projetId}/sorties/${s.fichier}`;
  if (n.video.dataset.url !== url) { n.video.dataset.url = url; n.video.src = url; }
  n.ecran.classList.toggle('vertical', (s.rapport.hauteur || 0) > (s.rapport.largeur || 1));
  dessinerRythme(s);
}

function dessinerRythme(s) {
  const r = s.rapport;
  const plans = r.plans_detail || [];
  const total = r.duree_montage || 1;
  const bande = h('div', { class: 'bande', role: 'img', 'aria-label': 'Rythme du montage, plan par plan' },
    plans.map((pl) => h('div', {
      class: `bloc ${pl.regime} ${pl.accroche ? 'accroche' : ''} ${pl.transition ? 'trans' : ''}`,
      style: { flexGrow: pl.d, flexBasis: 0 },
      title: `${minutes(pl.t)} · ${{ respiration: 'Respiration', rafale: 'Rafale', impact: 'Impact' }[pl.regime]} · cadre ${pl.cadre} · ${fr(pl.d)} s${pl.accroche ? ' · accroche' : ''}${pl.transition ? ` · transition ${pl.transition}` : ''}`,
      onclick: () => { E.noeuds.video.currentTime = pl.t + 0.01; E.noeuds.video.play(); },
    })));
  E.noeuds.tete = h('div', { class: 'tete', style: { left: '0%' } });
  bande.append(E.noeuds.tete);
  const notes = E.vue.projet.montage?.notes || [];
  const avert = r.avertissements || [];
  remplir(E.noeuds.rythme, 
    bande,
    h('div', { class: 'legende' },
      h('span', {}, h('i', { style: { background: 'var(--impact)' } }), 'Impact : zoom serré, coupe sèche'),
      h('span', {}, h('i', { style: { background: 'var(--rafale)' } }), 'Rafale : coupes rapides'),
      h('span', {}, h('i', { style: { background: 'var(--respiration)' } }), 'Respiration : on laisse vivre'),
      h('span', {}, h('i', { style: { background: 'repeating-linear-gradient(45deg,#999 0 3px,transparent 3px 6px)' } }), 'Accroche')),
    h('div', { class: 'chiffres' },
      h('div', { class: 'chiffre' }, h('b', {}, minutes(total)), h('span', {}, `sur ${minutes(r.duree_source)} de rushs`)),
      h('div', { class: 'chiffre' }, h('b', {}, r.plans), h('span', {}, 'plans')),
      h('div', { class: 'chiffre' }, h('b', {}, `${fr(r.duree_moyenne_plan)} s`), h('span', {}, 'par plan en moyenne')),
      h('div', { class: 'chiffre' }, h('b', {}, r.zooms_animes + r.transitions), h('span', {}, `zooms animés et transitions`))),
    avert.length ? h('div', { class: 'aide', style: { marginTop: '12px', background: 'var(--alerte-doux)' } }, icone('alerte'), h('span', {}, avert.join(' · '))) : null,
    notes.length ? h('div', { class: 'notes' }, h('h3', {}, 'Notes du monteur'), h('ul', { style: { margin: '6px 0 0', paddingLeft: '18px' } }, notes.map((t) => h('li', {}, t)))) : null);
}

function planCourant() {
  const s = sortieAffichee();
  const t = E.noeuds.video.currentTime;
  return (s?.rapport?.plans_detail || []).find((pl) => t >= pl.t && t < pl.t + pl.d);
}

function suivreLecture() {
  const s = sortieAffichee();
  if (!s || !E.noeuds.tete) return;
  E.noeuds.tete.style.left = `${(100 * E.noeuds.video.currentTime) / (s.rapport.duree_montage || 1)}%`;
  const pl = planCourant();
  const cle = pl && !pl.accroche ? pl.phrase : null;
  if (cle === E.phraseJouee) return;
  E.phraseJouee = cle;
  E.noeuds.listePhrases?.querySelectorAll('.phrase.joue').forEach((x) => x.classList.remove('joue'));
  if (cle) {
    const el = E.noeuds.listePhrases?.querySelector(`[data-cle="${CSS.escape(cle)}"]`);
    if (el) {
      el.classList.add('joue');
      const liste = E.noeuds.listePhrases;
      if (el.offsetTop < liste.scrollTop || el.offsetTop > liste.scrollTop + liste.clientHeight - 60) {
        liste.scrollTo({ top: el.offsetTop - 80, behavior: 'smooth' });
      }
    }
  }
}

function allerALaPhrase(cle) {
  const s = sortieAffichee();
  const pl = (s?.rapport?.plans_detail || []).find((x) => x.phrase === cle && !x.accroche);
  if (pl) { E.noeuds.video.currentTime = pl.t + 0.01; E.noeuds.video.play(); }
  else toast('Cette phrase n’est pas dans l’aperçu actuel (coupée, ou aperçu à mettre à jour).');
}

// ------------------------------------------------------------------ transcription

const REGIMES = ['rafale', 'impact', 'respiration'];
const CADRES = ['large', 'moyen', 'visage', 'yeux'];
const NOMS_REGIME = { respiration: 'Respiration', rafale: 'Rafale', impact: 'Impact' };
const NOMS_CADRE = { large: 'Plan large', moyen: 'Plan moyen', visage: 'Gros plan', yeux: 'Yeux' };

async function changerPhrase(cle, changement) {
  try {
    const r = await api('POST', `/api/projets/${E.projetId}/phrase`, { cle, ...changement });
    if (r.fait) { E.aMettreAJour = true; dessinerBarre(); }
  } catch (e) { toast(e.message, 'erreur'); }
}

function dessinerPhrases() {
  const { phrases, projet: p } = E.vue;
  const monte = !!p.montage;
  const visibles = phrases.filter((ph) => E.filtre === 'toutes' || !ph.decision
    || (E.filtre === 'gardees' ? ph.decision.garder : !ph.decision.garder));
  const nbCoupees = phrases.filter((ph) => ph.decision && !ph.decision.garder).length;
  const debut = E.noeuds.listePhrases?.scrollTop || 0;
  E.noeuds.listePhrases = h('div', { class: 'phrases' }, visibles.map((ph) => {
    const d = ph.decision;
    const classe = d ? (d.garder ? d.regime : 'coupee') : '';
    return h('div', { class: `phrase ${classe}`, 'data-cle': ph.cle },
      h('div', { class: 'temps' }, minutes(ph.t0)),
      h('div', { style: { minWidth: 0 } },
        h('div', { class: 'texte', style: { cursor: monte ? 'pointer' : 'default' }, title: monte ? 'Écouter ce passage dans l’aperçu' : '', onclick: () => monte && allerALaPhrase(ph.cle) }, ph.texte),
        d && d.garder ? h('div', { class: 'etiquettes' },
          h('button', { class: `puce ${d.regime}`, title: 'Régime de la phrase — cliquez pour changer', onclick: () => changerPhrase(ph.cle, { regime: REGIMES[(REGIMES.indexOf(d.regime) + 1) % 3] }) }, NOMS_REGIME[d.regime]),
          h('button', { class: 'puce', title: 'Cadre — cliquez pour changer', onclick: () => changerPhrase(ph.cle, { cadre: CADRES[(CADRES.indexOf(d.cadre) + 1) % 4] }) }, NOMS_CADRE[d.cadre]),
          p.incrustations.filter((i) => i.phrase === ph.cle).length ? h('span', { class: 'puce', style: { cursor: 'default' } }, `✦ ${p.incrustations.filter((i) => i.phrase === ph.cle).length} incrustation(s)`) : null) : null),
      monte ? h('div', { class: 'boutons' },
        h('button', { class: 'btn icone fantome', title: 'Ajouter une incrustation (image, GIF, vidéo, effet)', onclick: () => dialogueIncrustation(ph) }, icone('baguette')),
        h('button', { class: 'btn icone fantome', title: d && !d.garder ? 'Rétablir cette phrase' : 'Couper cette phrase', onclick: () => changerPhrase(ph.cle, { garder: !(d ? d.garder : true) }) },
          icone(d && !d.garder ? 'relancer' : 'ciseaux'))) : h('span'));
  }));
  remplir(E.noeuds.phrases, 
    h('div', { class: 'carte-tete' },
      h('h2', {}, '3. Le texte de la vidéo'),
      monte ? h('div', { class: 'onglets-petits' },
        [['toutes', 'Toutes'], ['gardees', 'Gardées'], ['coupees', `Coupées (${nbCoupees})`]].map(([v, l]) =>
          h('button', { class: E.filtre === v ? 'actif' : '', onclick: () => { E.filtre = v; dessinerPhrases(); } }, l))) : null),
    h('div', { class: 'carte-corps' },
      phrases.length === 0 ? h('p', { class: 'muet petit', style: { margin: 0 } }, 'La transcription apparaîtra ici après l’analyse.')
        : !monte ? h('div', { class: 'aide', style: { marginBottom: '10px' } }, icone('info'), h('span', {}, 'Voici ce qui est dit dans vos rushs. Proposez un montage : le LLM décidera, phrase par phrase, ce qu’il garde, le rythme et le cadre.'))
          : h('p', { class: 'tres-petit muet', style: { margin: '0 0 8px' } }, 'Cliquez une phrase pour l’écouter ; ses étiquettes pour changer le régime ou le cadre ; les ciseaux pour la couper ; la baguette pour une incrustation.'),
      E.noeuds.listePhrases));
  E.noeuds.listePhrases.scrollTop = debut;
  E.phraseJouee = null;
  suivreLecture();
}

// ------------------------------------------------------------------ panneau droit

function construirePanneau() {
  const n = E.noeuds;
  n.onglets = h('div', { class: 'onglets', role: 'tablist' });
  n.corpsPanneau = h('div', { style: { display: 'flex', flexDirection: 'column', minHeight: 0, flex: 1 } });
  n.messages = h('div', { class: 'messages' });
  n.saisie = h('textarea', { rows: '1', placeholder: 'Demandez un changement…', 'aria-label': 'Message pour EDIVID' });
  n.saisie.addEventListener('keydown', (e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); envoyer(); } });
  n.saisie.addEventListener('input', () => { n.saisie.style.height = 'auto'; n.saisie.style.height = `${Math.min(140, n.saisie.scrollHeight)}px`; });
  n.envoyer = h('button', { class: 'btn primaire icone', title: 'Envoyer', onclick: () => envoyer() }, icone('envoyer'));
  n.suggestions = h('div', { class: 'suggestions' });
  n.discussion = h('div', { class: 'discussion' }, n.messages, n.suggestions, h('div', { class: 'saisie' }, n.saisie, n.envoyer));
  remplir(n.panneau, n.onglets, n.corpsPanneau);
}

function dessinerPanneau() {
  const n = E.noeuds;
  const p = E.vue.projet;
  const onglets = [
    ['assistant', 'discussion', 'Assistant', 0],
    ['reglages', 'reglages', 'Réglages', 0],
    ['medias', 'image', 'Médias', p.medias.length],
    ['effets', 'baguette', 'Effets', p.incrustations.length],
  ];
  remplir(n.onglets, ...onglets.map(([id, ic, l, compte]) =>
    h('button', { role: 'tab', class: E.onglet === id ? 'actif' : '', 'aria-selected': E.onglet === id ? 'true' : 'false',
      onclick: () => { E.onglet = id; dessinerPanneau(); } }, icone(ic), l, compte ? h('span', { class: 'compte' }, compte) : null)));
  if (E.onglet === 'assistant') {
    dessinerDiscussion();
    if (n.corpsPanneau.firstChild !== n.discussion) remplir(n.corpsPanneau, n.discussion);
    return;
  }
  const corps = h('div', { class: 'onglet-corps' });
  if (E.onglet === 'reglages') dessinerReglages(corps);
  if (E.onglet === 'medias') dessinerMedias(corps);
  if (E.onglet === 'effets') dessinerEffets(corps);
  const defilement = n.corpsPanneau.firstChild?.scrollTop || 0;
  remplir(n.corpsPanneau, corps);
  corps.scrollTop = defilement;
}

const SUGGESTIONS = [
  'Rends le montage plus nerveux',
  'Moins de zooms, plus sobre',
  'Mets plus d’émotion dans le récit du début',
  'Ajoute une secousse comique sur une phrase drôle',
  'Coupe davantage, vise 3 minutes',
  'Pourquoi as-tu choisi cette accroche ?',
];

function dessinerDiscussion() {
  const n = E.noeuds;
  const p = E.vue.projet;
  const bas = n.messages.scrollHeight - n.messages.scrollTop - n.messages.clientHeight < 40;
  const bulles = [];
  if (p.discussion.length === 0) {
    bulles.push(h('div', { class: 'bulle edivid' },
      'Bonjour ! Je suis EDIVID, votre assistant de montage.\n\n',
      p.montage
        ? 'Le montage est prêt. Dites-moi ce que vous voulez changer, avec vos mots : le rythme, les zooms, une phrase à couper, un GIF à poser sur un moment drôle… Je modifie, et un nouvel aperçu se prépare tout seul.'
        : 'Commencez par déposer vos rushs à gauche. Une fois analysés, cliquez sur « Proposer un montage » : je monterai la vidéo, puis nous l’ajusterons ensemble ici.'));
  }
  for (const m of p.discussion) {
    bulles.push(h('div', { class: `bulle ${m.role}` }, m.texte,
      m.actions?.length ? h('div', { class: 'faits' }, m.actions.map((a) => h('span', { class: `fait ${a.startsWith('non appliqué') ? 'non' : ''}` }, a))) : null));
  }
  if (E.attenteReponse) bulles.push(h('div', { class: 'ecrit' }, h('i'), h('i'), h('i'), h('span', { style: { marginLeft: '6px' } }, 'EDIVID réfléchit…')));
  remplir(n.messages, ...bulles);
  if (bas || E.attenteReponse) n.messages.scrollTop = n.messages.scrollHeight;
  remplir(n.suggestions, ...(p.montage && !E.attenteReponse ? SUGGESTIONS.slice(0, 4) : []).map((s) =>
    h('button', { onclick: () => envoyer(s) }, s)));
  n.saisie.disabled = E.attenteReponse;
  n.envoyer.disabled = E.attenteReponse;
}

async function envoyer(texte) {
  const n = E.noeuds;
  const message = (texte ?? n.saisie.value).trim();
  if (!message || E.attenteReponse) return;
  n.saisie.value = '';
  n.saisie.style.height = 'auto';
  E.attenteReponse = true;
  E.vue.projet.discussion.push({ role: 'moi', texte: message, actions: [] });
  dessinerDiscussion();
  try {
    await api('POST', `/api/projets/${E.projetId}/discussion`, { message });
  } catch (e) {
    if (!(e instanceof ErreurApi && e.message === 'session expirée')) toast(e.message, 'erreur');
  } finally {
    E.attenteReponse = false;
    await rafraichir();
    n.saisie.focus();
  }
}

function controleReglage(r, valeur) {
  const changer = async (v) => {
    try {
      const res = await api('POST', `/api/projets/${E.projetId}/reglages`, { [r.cle]: v });
      if (res.faits?.length) {
        E.aMettreAJour = true;
        if (r.remontage) toast(`${res.faits.join(', ')} — pris en compte au prochain montage par le LLM.`);
      }
    } catch (e) { toast(e.message, 'erreur'); }
  };
  if (r.genre === 'interrupteur') {
    return h('label', { class: 'interrupteur', title: r.titre },
      h('input', { type: 'checkbox', checked: !!valeur, onchange: (e) => changer(e.target.checked), 'aria-label': r.titre }), h('span'));
  }
  if (r.genre === 'choix') {
    if (r.options.length <= 4) {
      return h('div', { class: 'segments', role: 'radiogroup', 'aria-label': r.titre }, r.options.map((o) =>
        h('button', { class: o.valeur === valeur ? 'actif' : '', role: 'radio', 'aria-checked': o.valeur === valeur ? 'true' : 'false', onclick: () => changer(o.valeur) }, o.libelle)));
    }
    return h('select', { class: 'champ', onchange: (e) => changer(e.target.value), 'aria-label': r.titre },
      r.options.map((o) => h('option', { value: o.valeur, selected: o.valeur === valeur }, o.libelle)));
  }
  if (r.genre === 'curseur') {
    const sortie = h('b', { class: 'petit' }, `${valeur} %`);
    return h('div', { style: { display: 'flex', alignItems: 'center', gap: '10px', marginTop: '8px' } },
      h('input', { type: 'range', min: r.bornes.min, max: r.bornes.max, step: '5', value: valeur, 'aria-label': r.titre,
        oninput: (e) => { sortie.textContent = `${e.target.value} %`; }, onchange: (e) => changer(Number(e.target.value)) }), sortie);
  }
  if (r.genre === 'media_son') {
    const sons = E.vue.projet.medias.filter((m) => m.genre === 'son');
    if (!sons.length) return h('p', { class: 'tres-petit muet', style: { margin: '6px 0 0' } }, 'Ajoutez un fichier son dans l’onglet Médias pour le choisir ici.');
    return h('select', { class: 'champ', onchange: (e) => changer(e.target.value), 'aria-label': r.titre },
      h('option', { value: '', selected: !valeur }, 'Aucune'),
      sons.map((m) => h('option', { value: m.id, selected: m.id === valeur }, m.nom)));
  }
  return null;
}

function dessinerReglages(corps) {
  const p = E.vue.projet;
  const sections = [...new Set(E.catalogue.map((r) => r.section))];
  corps.append(h('div', { class: 'aide', style: { marginBottom: '14px' } }, icone('info'),
    h('span', {}, 'Chaque réglage est enregistré aussitôt. Cliquez ensuite sur « Mettre à jour l’aperçu ». Vous pouvez aussi simplement demander à l’assistant.')));
  for (const s of sections) {
    corps.append(h('div', { class: 'section-reglages' }, h('h3', {}, s),
      E.catalogue.filter((r) => r.section === s).map((r) => {
        const valeur = p.reglages[r.cle];
        const enLigne = r.genre === 'interrupteur';
        return h('div', { class: 'reglage' },
          h('div', { class: 'reglage-tete' },
            h('span', { class: 'reglage-titre' }, r.titre, r.remontage ? h('span', { class: 'marque', title: 'Lu par le LLM au moment du montage' }, 'au prochain montage') : null),
            enLigne ? controleReglage(r, valeur) : null),
          h('div', { class: 'reglage-aide' }, r.aide),
          enLigne ? null : controleReglage(r, valeur));
      })));
  }
}

const GENRES_MEDIA = { image: 'Image', gif: 'GIF animé', video: 'Vidéo', son: 'Son' };

function vignetteMedia(m) {
  const url = `/api/projets/${E.projetId}/medias/${m.id}`;
  if (m.genre === 'image' || m.genre === 'gif') return h('img', { src: url, alt: m.nom, loading: 'lazy' });
  if (m.genre === 'video') return h('video', { src: url, muted: true, preload: 'metadata', onmouseenter: (e) => e.target.play(), onmouseleave: (e) => e.target.pause() });
  return icone('musique');
}

function dessinerMedias(corps) {
  const p = E.vue.projet;
  E.noeuds.televersementsMedias = h('div', { class: 'liste-rushs' });
  corps.append(
    h('div', { class: 'aide', style: { marginBottom: '12px' } }, icone('info'),
      h('span', {}, 'Vos images, GIF, vidéos et sons. Les images et vidéos se posent sur une phrase (la baguette dans le texte, ou en le demandant à l’assistant) ; les sons servent de musique de fond (onglet Réglages).')),
    zoneDepot('Ajoutez des médias', 'Images, GIF, vidéos courtes, musiques', 'image/*,video/*,audio/*,.gif', (f) => televerser(f, 'medias')),
    E.noeuds.televersementsMedias,
    p.medias.length ? h('div', { class: 'grille-medias' }, p.medias.map((m) =>
      h('div', { class: 'media' },
        h('div', { class: 'apercu-media' }, vignetteMedia(m)),
        h('div', { class: 'infos' }, h('b', { title: m.nom }, m.nom),
          h('span', { class: 'muet' }, `${GENRES_MEDIA[m.genre] || m.genre}${m.duree ? ` · ${fr(m.duree)} s` : ''}`),
          m.genre === 'son' ? h('button', { class: 'btn petit doux', style: { marginTop: '6px', width: '100%' }, onclick: async () => {
            await api('POST', `/api/projets/${E.projetId}/reglages`, { musique: m.id });
            E.aMettreAJour = true;
            toast(`« ${m.nom} » devient la musique de fond.`, 'ok');
          } }, p.reglages.musique === m.id ? 'Musique de fond ✓' : 'Utiliser en musique') : null),
        h('button', { class: 'supprimer', title: 'Retirer ce média', onclick: async () => {
          if (await confirmer('Retirer ce média ?', `« ${m.nom} » et ses incrustations seront retirés.`, 'Retirer')) {
            await api('DELETE', `/api/projets/${E.projetId}/medias/${m.id}`);
          }
        } }, icone('fermer'))))) : null);
  dessinerTeleversements();
}

const POSITIONS = [
  ['haut_gauche', 'Haut gauche'], ['plein', 'Plein écran'], ['haut_droite', 'Haut droite'],
  ['bas_gauche', 'Bas gauche'], ['centre', 'Centre'], ['bas_droite', 'Bas droite'],
];
const NOMS_POSITION = Object.fromEntries(POSITIONS);
const EFFETS = { secousse: ['Secousse', 'L’image tremble : effet comique, surprise.'], noir_blanc: ['Noir et blanc', 'Le temps d’un instant dramatique.'] };

function dessinerEffets(corps) {
  const p = E.vue.projet;
  const texte = Object.fromEntries(E.vue.phrases.map((ph) => [ph.cle, ph]));
  corps.append(h('div', { class: 'aide', style: { marginBottom: '12px' } }, icone('info'),
    h('span', {}, 'Une incrustation pose une image, un GIF, une vidéo ou un effet sur une phrase précise. Pour en ajouter une : la baguette à côté d’une phrase, ou demandez à l’assistant (« mets le GIF du chat quand elle parle de montgolfière »).')));
  if (!p.incrustations.length) {
    corps.append(h('p', { class: 'muet petit' }, 'Aucune incrustation pour l’instant.'));
    return;
  }
  for (const inc of p.incrustations) {
    const media = p.medias.find((m) => m.id === inc.media);
    const quoi = inc.effet ? EFFETS[inc.effet]?.[0] : media?.nom || '(média retiré)';
    const ph = texte[inc.phrase];
    corps.append(h('div', { class: 'incrustation' },
      h('div', { style: { minWidth: 0 } },
        h('b', { class: 'petit' }, `✦ ${quoi}`),
        h('div', { class: 'tres-petit muet', style: { marginTop: '3px' } }, ph ? `« ${ph.texte.slice(0, 90)}${ph.texte.length > 90 ? '…' : ''} »` : 'phrase introuvable'),
        h('div', { class: 'tres-petit muet' }, `${inc.decalage ? `après ${fr(inc.decalage)} s · ` : ''}${fr(inc.duree)} s${inc.effet ? '' : ` · ${NOMS_POSITION[inc.position] || inc.position}`}${inc.fond_vert ? ' · fond vert retiré' : ''}`)),
      h('div', {},
        ph ? h('button', { class: 'btn icone fantome', title: 'Voir ce moment', onclick: () => allerALaPhrase(inc.phrase) }, icone('lecture')) : null,
        h('button', { class: 'btn icone fantome danger', title: 'Retirer', onclick: async () => {
          await api('DELETE', `/api/projets/${E.projetId}/incrustations/${inc.id}`);
          E.aMettreAJour = true;
        } }, icone('corbeille')))));
  }
}

function dialogueIncrustation(ph) {
  const p = E.vue.projet;
  const visuels = p.medias.filter((m) => m.genre !== 'son');
  const choix = { genre: visuels.length ? 'media' : 'effet', media: visuels[0]?.id || '', effet: 'secousse', position: 'centre', taille: 40, duree: 2, decalage: 0, fond_vert: false, son: false };
  const dureePhrase = Math.max(0.5, ph.t1 - ph.t0);
  fenetre('Ajouter une incrustation', (fermer) => {
    const zone = h('div');
    const redessiner = () => {
      const media = p.medias.find((m) => m.id === choix.media);
      remplir(zone, 
        h('p', { class: 'petit muet', style: { marginTop: 0 } }, `Sur la phrase : « ${ph.texte} »`),
        h('div', { class: 'segments' },
          h('button', { class: choix.genre === 'media' ? 'actif' : '', onclick: () => { choix.genre = 'media'; redessiner(); } }, 'Un média'),
          h('button', { class: choix.genre === 'effet' ? 'actif' : '', onclick: () => { choix.genre = 'effet'; redessiner(); } }, 'Un effet')),
        choix.genre === 'media'
          ? (visuels.length
            ? h('div', { class: 'grille-medias', style: { gridTemplateColumns: 'repeat(3, 1fr)' } }, visuels.map((m) =>
                h('button', { class: `media ${choix.media === m.id ? 'choisi' : ''}`, style: { cursor: 'pointer', padding: 0, textAlign: 'left' }, onclick: () => { choix.media = m.id; redessiner(); } },
                  h('div', { class: 'apercu-media' }, vignetteMedia(m)), h('div', { class: 'infos' }, h('b', {}, m.nom)))))
            : h('div', { class: 'aide', style: { marginTop: '12px' } }, icone('info'), h('span', {}, 'Ajoutez d’abord des images, GIF ou vidéos dans l’onglet Médias.')))
          : h('div', { class: 'grille-medias' }, Object.entries(EFFETS).map(([k, [t, d]]) =>
              h('button', { class: `carte ${choix.effet === k ? 'media choisi' : 'media'}`, style: { cursor: 'pointer', padding: '12px', textAlign: 'left' }, onclick: () => { choix.effet = k; redessiner(); } },
                h('b', {}, t), h('div', { class: 'tres-petit muet', style: { marginTop: '4px' } }, d)))),
        choix.genre === 'media' ? h('div', { class: 'ligne-champ' }, h('label', {}, 'Position'),
          h('div', { class: 'grille-positions' }, POSITIONS.map(([v, l]) =>
            h('button', { class: choix.position === v ? 'actif' : '', onclick: () => { choix.position = v; redessiner(); } }, l)))) : null,
        choix.genre === 'media' && choix.position !== 'plein' ? h('div', { class: 'ligne-champ' }, h('label', {}, `Taille : ${choix.taille} % de la largeur`),
          h('input', { type: 'range', min: '10', max: '100', step: '5', value: choix.taille, oninput: (e) => { choix.taille = Number(e.target.value); e.target.previousSibling.textContent = `Taille : ${choix.taille} % de la largeur`; } })) : null,
        h('div', { class: 'ligne-champ' }, h('label', {}, `Durée : ${fr(choix.duree)} s`),
          h('input', { type: 'range', min: '0.5', max: '8', step: '0.5', value: choix.duree, oninput: (e) => { choix.duree = Number(e.target.value); e.target.previousSibling.textContent = `Durée : ${fr(choix.duree)} s`; } })),
        h('div', { class: 'ligne-champ' }, h('label', {}, `Commence ${choix.decalage ? `${fr(choix.decalage)} s après` : 'au'} début de la phrase`),
          h('input', { type: 'range', min: '0', max: String(Math.floor(dureePhrase * 2) / 2), step: '0.5', value: choix.decalage, oninput: (e) => { choix.decalage = Number(e.target.value); e.target.previousSibling.textContent = `Commence ${choix.decalage ? `${fr(choix.decalage)} s après` : 'au'} début de la phrase`; } })),
        choix.genre === 'media' ? h('div', { class: 'reglage' }, h('div', { class: 'reglage-tete' }, h('span', { class: 'reglage-titre' }, 'Retirer le fond vert'),
          h('label', { class: 'interrupteur' }, h('input', { type: 'checkbox', checked: choix.fond_vert, onchange: (e) => { choix.fond_vert = e.target.checked; } }), h('span'))),
          h('div', { class: 'reglage-aide' }, 'Pour les mèmes et effets filmés sur fond vert.')) : null,
        choix.genre === 'media' && media?.genre === 'video' && media.a_son ? h('div', { class: 'reglage' }, h('div', { class: 'reglage-tete' }, h('span', { class: 'reglage-titre' }, 'Garder son son'),
          h('label', { class: 'interrupteur' }, h('input', { type: 'checkbox', checked: choix.son, onchange: (e) => { choix.son = e.target.checked; } }), h('span')))) : null,
        h('div', { style: { display: 'flex', justifyContent: 'flex-end', gap: '8px', marginTop: '20px' } },
          h('button', { class: 'btn', onclick: fermer }, 'Annuler'),
          h('button', { class: 'btn primaire', disabled: choix.genre === 'media' && !choix.media, onclick: async () => {
            try {
              await api('POST', `/api/projets/${E.projetId}/incrustations`, {
                media: choix.genre === 'media' ? choix.media : '', effet: choix.genre === 'effet' ? choix.effet : '',
                phrase: ph.cle, decalage: choix.decalage, duree: choix.duree, position: choix.position,
                taille: choix.taille / 100, fond_vert: choix.fond_vert, son: choix.son,
              });
              E.aMettreAJour = true;
              fermer();
              toast('Incrustation ajoutée. Mettez à jour l’aperçu pour la voir.', 'ok');
            } catch (e) { toast(e.message, 'erreur'); }
          } }, icone('plus'), 'Ajouter')));
    };
    redessiner();
    return zone;
  }, { large: true });
}

// ------------------------------------------------------------------ barre d'actions

function dessinerBarre() {
  const n = E.noeuds;
  if (!n.barre || !E.vue) return;
  const p = E.vue.projet;
  const file = (E.etat?.file || []);
  const miens = file.filter((t) => t.projet === E.projetId);
  const enCours = miens.find((t) => t.en_cours);
  const autres = file.find((t) => t.en_cours && t.projet !== E.projetId);
  const attente = miens.filter((t) => !t.en_cours).length;
  const prets = p.rushs.some((r) => r.etat === 'pret');
  let titre;
  let detail = '';
  let pct = null;
  if (enCours) {
    titre = E.progression?.etape || enCours.libelle;
    pct = E.progression?.pct ?? null;
    detail = E.progression?.detail || (attente ? `puis ${attente} autre${attente > 1 ? 's' : ''} travail${attente > 1 ? 'x' : ''}` : enCours.libelle);
  } else if (autres) {
    titre = 'En attente';
    detail = `Un autre projet utilise EDIVID (${autres.libelle}) ; votre demande suivra.`;
  } else if (!p.rushs.length) {
    titre = 'Prochaine étape : déposez vos rushs';
  } else if (!E.vue.pret_a_monter) {
    titre = 'Analyse des rushs…';
  } else if (!p.montage) {
    titre = 'Rushs prêts : proposez un montage';
    detail = 'Le LLM monte, puis relit son montage (quelques minutes).';
  } else if (E.aMettreAJour) {
    titre = 'Des changements attendent un nouvel aperçu';
  } else if (!p.rendu) {
    titre = 'Ajustez, puis lancez le rendu final';
    detail = 'Pleine définition, netteté IA sur les zooms, son normalisé pour YouTube.';
  } else {
    titre = 'Rendu final prêt';
    detail = `${p.rendu.rapport.largeur}×${p.rendu.rapport.hauteur} · ${minutes(p.rendu.rapport.duree_montage)}`;
  }
  const occupe = !!enCours;
  // Le LLM ne donne pas d'avancement réel : barre animée plutôt qu'un pourcentage figé.
  const indetermine = !!enCours && (pct === null || /LLM|attente/i.test(titre));
  if (indetermine && /LLM/.test(titre)) detail = 'Le LLM réfléchit : comptez quelques minutes pour une vidéo de dix minutes.';
  remplir(n.barre,
    h('div', { class: `progression ${pct === null && !indetermine ? 'repos' : ''}` },
      h('div', { class: 'ligne' }, h('b', {}, titre), pct !== null && !indetermine ? h('span', { class: 'muet' }, `${fr(pct, 0)} %`) : null),
      indetermine ? h('div', { class: 'jauge indetermine' }, h('div'))
        : pct !== null ? h('div', { class: 'jauge' }, h('div', { style: { width: `${pct}%` } })) : null,
      detail ? h('span', { class: 'tres-petit muet' }, detail) : null),
    h('button', {
      class: `btn ${!p.montage && E.vue.pret_a_monter ? 'primaire' : ''}`, disabled: !E.vue.pret_a_monter || occupe,
      title: 'Le LLM décide coupes, rythme, zooms, accroche et parties',
      onclick: async () => {
        if (p.montage && !(await confirmer('Refaire le montage ?', 'Le LLM remonte toute la vidéo avec les réglages actuels. Vos changements phrase par phrase seront remplacés (les incrustations sont gardées).', 'Refaire'))) return;
        try { await api('POST', `/api/projets/${E.projetId}/monter`); } catch (e) { toast(e.message, 'erreur'); }
      },
    }, icone('etincelles'), h('span', { class: 'long' }, p.montage ? 'Refaire le montage' : 'Proposer un montage'), h('span', { class: 'court' }, 'Monter')),
    h('button', {
      class: `btn ${E.aMettreAJour ? 'primaire' : ''}`, disabled: !prets || !p.montage || occupe,
      title: 'Aperçu en demi-définition, sans netteté IA : rapide',
      onclick: async () => { try { await api('POST', `/api/projets/${E.projetId}/apercu`); } catch (e) { toast(e.message, 'erreur'); } },
    }, icone('relancer'), h('span', { class: 'long' }, 'Mettre à jour l’aperçu'), h('span', { class: 'court' }, 'Aperçu')),
    h('button', {
      class: `btn ${p.montage && !E.aMettreAJour && !p.rendu ? 'primaire' : ''}`, disabled: !p.montage || occupe,
      title: 'Pleine définition, netteté IA, son prêt pour YouTube',
      onclick: async () => { try { await api('POST', `/api/projets/${E.projetId}/rendu`); toast('Rendu final lancé. Vous pouvez continuer à travailler.'); } catch (e) { toast(e.message, 'erreur'); } },
    }, icone('film'), h('span', { class: 'long' }, 'Rendu final'), h('span', { class: 'court' }, 'Rendu')),
    p.rendu ? h('a', { class: 'btn doux', href: `/api/projets/${E.projetId}/sorties/${p.rendu.fichier}`, download: `${p.nom}.mp4` }, icone('telecharger'), 'Télécharger') : null);
}

// ------------------------------------------------------------------ guide

function afficherGuide() {
  const etapes = [
    ['Déposez vos rushs', 'Glissez vos vidéos dans « Vos rushs ». Les fichiers de l’iPhone sont acceptés tels quels : EDIVID corrige la rotation, convertit les couleurs HDR et prépare le son.'],
    ['Laissez analyser', 'EDIVID transcrit chaque mot, mesure les silences et suit le visage. La transcription apparaît dans « Le texte de la vidéo ».'],
    ['Proposez un montage', 'Le LLM coupe les redites et les silences, choisit pour chaque phrase un régime (impact, rafale, respiration) et un cadre (du plan large aux yeux), monte une accroche, puis relit son propre travail.'],
    ['Ajustez en discutant', 'Dites à l’assistant ce que vous voulez, avec vos mots. Ou changez une phrase d’un clic, un réglage dans l’onglet Réglages, posez un GIF avec la baguette.'],
    ['Exportez', '« Rendu final » produit la vidéo en pleine définition, avec la netteté IA sur les zooms et un son prêt pour YouTube. Téléchargez-la ensuite.'],
  ];
  fenetre('Comment ça marche', (fermer) => h('div', {},
    h('div', { class: 'guide-liste' }, etapes.map(([t, d], i) =>
      h('div', { class: 'guide-item' }, h('div', { class: 'num' }, i + 1), h('div', {}, h('h3', {}, t), h('p', { class: 'petit muet', style: { margin: '3px 0 0' } }, d))))),
    h('div', { class: 'aide', style: { marginTop: '14px' } }, icone('info'),
      h('span', {}, 'L’aperçu est volontairement rapide (demi-définition, sans netteté IA) : servez-vous-en pour juger le rythme. Le rendu final, plus long, soigne l’image.')),
    h('div', { style: { display: 'flex', justifyContent: 'flex-end', marginTop: '16px' } },
      h('button', { class: 'btn primaire', onclick: fermer }, 'C’est parti'))), { large: true });
}
