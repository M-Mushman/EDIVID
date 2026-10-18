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
  crayon: '<path d="M4 20h4L19 9l-4-4L4 16zM14 6l4 4"/>',
  chevron: '<path d="M6 9l6 6 6-6"/>',
  google: '<path d="M20.5 12.2c0-.6-.1-1.2-.2-1.7H12v3.3h4.8a4.1 4.1 0 0 1-1.8 2.7v2.2h2.9c1.7-1.6 2.6-3.9 2.6-6.5z"/><path d="M12 21c2.4 0 4.5-.8 5.9-2.2l-2.9-2.2c-.8.5-1.8.9-3 .9-2.3 0-4.3-1.6-5-3.7H4v2.3A9 9 0 0 0 12 21z"/><path d="M7 13.8a5.4 5.4 0 0 1 0-3.5V8H4a9 9 0 0 0 0 8.1z"/><path d="M12 6.6c1.3 0 2.5.5 3.4 1.3l2.6-2.6A9 9 0 0 0 4 8l3 2.3c.7-2.1 2.7-3.7 5-3.7z"/>',
  engrenage: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>',
  livre: '<path d="M4 4h6a3 3 0 0 1 3 3v13a2 2 0 0 0-2-2H4zM20 4h-6a3 3 0 0 0-3 3v13a2 2 0 0 1 2-2h7z"/>',
  etoile: '<path d="M12 3l2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17l-5.4 2.9 1-6.1-4.4-4.3 6.1-.9z"/>',
  nuage: '<path d="M7 18a4 4 0 0 1-.6-8 6 6 0 0 1 11.5 1.5A3.5 3.5 0 0 1 17.5 18z"/>',
  courriel: '<path d="M4 6h16v12H4z"/><path d="M4 7l8 6 8-6"/>',
  copier: '<path d="M9 9h11v11H9z"/><path d="M5 15H4V4h11v1"/>',
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

// Un appel au LLM (conversation, commentaire, proposition YouTube) peut durer ; le reste répond en une fraction de seconde.
const LONGS = ['/discussion', '/commentaire', '/youtube/proposer', '/parametres/courriel/essai', '/parametres/drive/verifier'];

async function api(methode, chemin, corps) {
  const delai = LONGS.some((l) => chemin.includes(l)) ? 600000 : 25000;
  const arret = new AbortController();
  const minuterieDelai = setTimeout(() => arret.abort(), delai);
  let r;
  try {
    r = await fetch(chemin, {
      method: methode,
      headers: corps !== undefined ? { 'Content-Type': 'application/json' } : {},
      body: corps !== undefined ? JSON.stringify(corps) : undefined,
      credentials: 'same-origin',
      signal: arret.signal,
    });
  } catch (e) {
    if (e.name === 'AbortError') {
      throw new ErreurApi('EDIVID ne répond pas. Fermez les onglets EDIVID en trop, ou ouvrez https://azoth.tailaae476.ts.net.');
    }
    throw new ErreurApi('Connexion à EDIVID impossible (réseau ou tailnet).');
  } finally {
    clearTimeout(minuterieDelai);
  }
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

// ------------------------------------------------------------------ panneaux repliables

function estReplie(nom) { return lire(`edivid_replie_${nom}`) === '1'; }

/** En-tête de carte cliquable : replie ou déplie le corps ; l'état est gardé par navigateur. */
function enteteRepliable(nom, titre, ...extras) {
  const replie = estReplie(nom);
  const basculer = () => {
    stocker(`edivid_replie_${nom}`, estReplie(nom) ? '0' : '1');
    const carte = tete.parentElement;
    carte?.classList.toggle('replie', estReplie(nom));
    chevron.setAttribute('aria-expanded', estReplie(nom) ? 'false' : 'true');
  };
  const chevron = h('button', { class: 'btn icone fantome chevron', title: 'Replier ou déplier ce panneau', 'aria-expanded': replie ? 'false' : 'true',
    onclick: (e) => { e.stopPropagation(); basculer(); } }, icone('chevron'));
  const tete = h('div', { class: 'carte-tete repliable', onclick: (e) => { if (e.target === tete || e.target.tagName === 'H2') basculer(); } },
    h('h2', {}, titre), h('span', { class: 'espace' }), ...extras, chevron);
  return tete;
}

function appliquerReplie(carte, nom) { carte.classList.toggle('replie', estReplie(nom)); }

// ------------------------------------------------------------------ état

const E = {
  catalogue: null,
  projetId: null,
  vue: null,
  etat: null,
  onglet: 'assistant',
  lecteur: 'apercu',
  rushLecteur: null,
  programme: null,
  proposition: null,
  commentaire: null,
  correction: false,
  voirTlOrigine: lire('edivid_voirTlOrigine') !== '0',
  voirTlMontage: lire('edivid_voirTlMontage') !== '0',
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
  E.session = session;
  messageGoogle();
  if (!session.connecte) return afficherConnexion();
  if (!E.catalogue) E.catalogue = await api('GET', '/api/catalogue');
  if (location.hash === '#/parametres') { fermerFlux(); E.projetId = null; return afficherParametres(); }
  const m = location.hash.match(/^#\/projet\/([a-z0-9]+)/);
  if (m) return ouvrirProjet(m[1]);
  fermerFlux();
  E.projetId = null;
  afficherAccueil();
}

/** Après un aller-retour chez Google, l'adresse porte ?google=… : on l'explique, puis on l'efface. */
function messageGoogle() {
  const q = new URLSearchParams(location.search);
  const g = q.get('google');
  if (!g) return;
  const messages = {
    expire: 'La demande de connexion a expiré : recommencez.',
    annule: 'Connexion avec Google annulée.',
    session_requise: 'Ouvrez d’abord EDIVID avec le code d’accès pour relier un compte Google.',
    non_configure: 'La connexion avec Google n’est pas encore configurée (Paramètres).',
    erreur: `Connexion avec Google impossible : ${q.get('detail') || 'erreur inconnue'}`,
  };
  setTimeout(() => toast(messages[g] || 'Connexion avec Google : réponse inattendue.', g === 'annule' ? '' : 'erreur'), 300);
  history.replaceState(null, '', location.pathname + location.hash);
}

/** Pastille du compte connecté, avec la déconnexion. */
function puceCompte() {
  const c = E.session?.compte;
  if (!c) return null;
  const initiale = (c.nom || c.email).trim()[0]?.toUpperCase() || '?';
  return h('button', { class: 'btn petit fantome puce-compte', title: `${c.nom} — ${c.email}\nCliquer pour se déconnecter`, onclick: async () => {
    if (!(await confirmer('Se déconnecter ?', `Vous êtes connecté avec ${c.email}.`, 'Se déconnecter'))) return;
    await api('POST', '/api/deconnexion');
    location.hash = '#/';
    location.reload();
  } }, h('span', { class: 'avatar' }, initiale), h('span', { class: 'masque-mobile' }, c.nom.split(' ')[0]));
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
      E.session?.google ? h('div', {},
        h('a', { class: 'btn bouton-google', href: '/api/google/connexion?mode=connexion' }, icone('google'), 'Se connecter avec Google'),
        h('p', { class: 'tres-petit muet', style: { margin: '8px 0 0' } }, 'Votre compte doit avoir été autorisé. La connexion relie aussi votre Google Drive.'),
        h('div', { class: 'separateur' }, h('span', {}, 'ou'))) : null,
      h('label', { class: 'petit', style: { fontWeight: 600, display: 'block', marginBottom: '6px' } }, "Code d'accès"),
      champ, erreur,
      h('button', { class: 'btn primaire', type: 'submit', style: { width: '100%', marginTop: '10px' } }, 'Entrer'),
      h('p', { class: 'tres-petit muet', style: { marginTop: '14px' } },
        "Les appareils du tailnet entrent sans code par l'adresse en https. Sinon, le code vous a été donné par la personne qui gère cet ordinateur ; il n'est demandé qu'une fois par navigateur."))));
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
    h('div', { style: { display: 'flex', alignItems: 'center', gap: '12px' } }, logo(), h('span', { class: 'espace' }),
      h('a', { class: 'btn petit fantome', href: '#/parametres', title: 'Courriel, Google Drive, encodage' }, icone('engrenage'), 'Paramètres'), puceCompte(), boutonTheme()),
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
              p.rendu ? h('span', { class: 'pastille ok' }, 'Rendu prêt') : null,
              p.origine ? h('span', { class: 'pastille' }, icone('nuage'), p.origine) : null))))));
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

/** Ouvre le flux d'événements du projet affiché. */
function ouvrirFlux(id) {
  fermerFlux();
  E.source = new EventSource(`/api/projets/${id}/evenements`);
  brancherFlux(E.source);
}

// Un onglet caché rend sa connexion d'événements ; il la reprend, et se remet à jour, quand on y revient.
document.addEventListener('visibilitychange', () => {
  if (!E.projetId) return;
  if (document.hidden) fermerFlux();
  else if (!E.source) { ouvrirFlux(E.projetId); rafraichir(); rafraichirEtat(); }
});

async function ouvrirProjet(id) {
  if (E.projetId !== id) {
    fermerFlux();
    E.projetId = id;
    E.vue = null;
    E.progression = null;
    E.aMettreAJour = false;
    E.onglet = 'assistant';
    construireProjet();
    if (!document.hidden) ouvrirFlux(id);
  }
  await rafraichir();
  await rafraichirEtat();
  api('GET', '/api/parametres').then((P) => { E.driveActif = !!(P.drive.actif && P.etat_drive?.rclone); dessinerBarre(); }).catch(() => {});
  if (!lire('edivid_guide_vu')) { stocker('edivid_guide_vu', '1'); afficherGuide(); }
}

function brancherFlux(source) {
  {
    source.addEventListener('progression', (e) => {
      E.progression = JSON.parse(e.data).donnees;
      dessinerBarre();
      if (E.vue?.projet.rushs.some((r) => ['preparation', 'analyse'].includes(r.etat))) dessinerRushs();
    });
    source.addEventListener('projet', () => rafraichirBientot());
    source.addEventListener('file', () => rafraichirEtat());
    source.addEventListener('erreur', (e) => {
      const d = JSON.parse(e.data).donnees;
      toast(`${d.travail} : ${d.message}`, 'erreur');
      E.progression = null;
      rafraichir();
    });
    source.addEventListener('drive', (e) => {
      toast(`Vidéo déposée sur le Google Drive : ${JSON.parse(e.data).donnees.chemin}`, 'ok');
      rafraichir();
    });
    source.addEventListener('youtube', (e) => {
      toast(`Vidéo en ligne sur YouTube : ${JSON.parse(e.data).donnees.lien}`, 'ok');
      E.progression = null;
      rafraichir();
    });
    source.addEventListener('fini', (e) => {
      const d = JSON.parse(e.data).donnees;
      E.progression = null;
      if (d.genre === 'export') { toast('Export prêt : téléchargez-le ou publiez-le depuis « Exporter ».', 'ok'); rafraichir(); return; }
      if (d.genre === 'rendu') { E.lecteur = 'rendu'; toast('Le rendu final est prêt : vous pouvez le télécharger.', 'ok'); }
      else { E.lecteur = 'apercu'; E.aMettreAJour = false; toast('Nouvel aperçu prêt.', 'ok'); }
      rafraichir();
    });
  }
}

let minuterie = null;
function rafraichirBientot() {
  clearTimeout(minuterie);
  minuterie = setTimeout(rafraichir, 150);
}

async function rafraichir() {
  if (!E.projetId) return;
  try {
    const [vue, programme] = await Promise.all([
      api('GET', `/api/projets/${E.projetId}`),
      api('GET', `/api/projets/${E.projetId}/programme`).catch(() => null),
    ]);
    E.vue = vue;
    E.programme = programme;
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
      h('a', { class: 'btn icone fantome', href: '#/parametres', title: 'Paramètres : courriel, Google Drive' }, icone('engrenage')),
      puceCompte(),
      boutonTheme(),
      h('button', { class: 'btn icone fantome danger', title: 'Supprimer le projet', onclick: supprimerProjet }, icone('corbeille')))),
    n.parcours,
    h('main', { class: 'mise-en-page' },
      h('div', { class: 'colonne' }, n.rushs, n.lecteur, n.phrases),
      n.panneau),
    h('div', { class: 'barre-actions' }, n.barre));
  construireLecteur();
  remplir(n.lecteur, enteteRepliable('lecteur', '2. Aperçu du montage', n.ongletsLecteur), n.corpsLecteur);
  appliquerReplie(n.lecteur, 'lecteur');
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
      h('span', { class: 'rond' }, fait ? '✓' : i + 1), t)),
  p.auto ? h('div', { class: 'pas', title: 'Projet venu du Google Drive : EDIVID enchaîne les étapes tout seul.' },
    h('span', { class: 'rond' }, icone('nuage')), p.auto.rendu_final ? 'Automatique jusqu’au rendu' : 'Automatique jusqu’à l’aperçu') : null,
  p.depot_drive ? h('div', { class: 'pas fait', title: p.depot_drive }, h('span', { class: 'rond' }, '✓'), 'Déposé sur le Drive') : null);
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
  appliquerReplie(E.noeuds.rushs, 'rushs');
  remplir(E.noeuds.rushs,
    enteteRepliable('rushs', '1. Vos rushs',
      p.rushs.length > 1 ? h('span', { class: 'tres-petit muet' }, 'Montés dans cet ordre') : null,
      estReplie('rushs') && p.rushs.length ? h('span', { class: 'tres-petit muet' }, `${p.rushs.length} rush${p.rushs.length > 1 ? 's' : ''}`) : null),
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

// ------------------------------------------------------------------ lecteur et timelines

function construireLecteur() {
  const n = E.noeuds;
  n.video = h('video', { controls: true, playsinline: true, preload: 'metadata' });
  n.video.addEventListener('timeupdate', suivreLecture);
  n.video.addEventListener('pause', () => { dessinerOutilsLecteur(); dessinerCalque(); });
  n.video.addEventListener('play', () => { dessinerOutilsLecteur(); dessinerCalque(); });
  n.video.addEventListener('seeked', dessinerCalque);
  n.video.addEventListener('loadedmetadata', dessinerCalque);
  n.calque = h('div', { class: 'calque' });
  n.indicateurDepot = h('div', { class: 'indicateur-depot cache' }, 'Déposez ici : la position et l’instant de la vidéo seront retenus');
  n.ecran = h('div', { class: 'lecteur' }, n.video, n.calque, n.indicateurDepot);
  n.ecran.addEventListener('dragover', (e) => {
    if (!e.dataTransfer.types.includes('application/x-edivid')) return;
    e.preventDefault();
    n.indicateurDepot.classList.remove('cache');
  });
  n.ecran.addEventListener('dragleave', (e) => { if (!n.ecran.contains(e.relatedTarget)) n.indicateurDepot.classList.add('cache'); });
  n.ecran.addEventListener('drop', deposerSurVideo);
  n.ongletsLecteur = h('div', { class: 'onglets-petits' });
  n.outilsLecteur = h('div', { class: 'outils-lecteur' });
  n.zoneCommentaire = h('div');
  n.timelines = h('div', { class: 'timelines' });
  n.stats = h('div');
  n.corpsLecteur = h('div', { class: 'carte-corps' }, n.ecran, n.outilsLecteur, n.zoneCommentaire, n.timelines, n.stats);
}

function sortieAffichee() {
  const p = E.vue.projet;
  if (E.lecteur === 'rendu') return p.rendu || null;
  if (E.lecteur === 'apercu') return p.apercu || null;
  return null;
}

function rushLecteur() {
  const p = E.vue.projet;
  if (!E.rushLecteur || !p.rushs.some((r) => r.id === E.rushLecteur)) E.rushLecteur = p.rushs[0]?.id || null;
  return E.rushLecteur;
}

function dessinerLecteur() {
  const p = E.vue.projet;
  const n = E.noeuds;
  if (E.lecteur !== 'origine' && !sortieAffichee()) {
    // Pas encore de montage : on montre la vidéo d'origine si elle est prête.
    if (E.lecteur === 'apercu' && !p.apercu && E.vue.proxys?.length) E.lecteur = 'origine';
  }
  remplir(n.ongletsLecteur,
    h('button', { class: E.lecteur === 'origine' ? 'actif' : '', disabled: !E.vue.proxys?.length, title: E.vue.proxys?.length ? 'La vidéo telle que filmée' : 'Version d’origine en préparation', onclick: () => changerLecteur('origine') }, 'Origine'),
    h('button', { class: E.lecteur === 'apercu' ? 'actif' : '', disabled: !p.apercu, onclick: () => changerLecteur('apercu') }, 'Aperçu'),
    h('button', { class: E.lecteur === 'rendu' ? 'actif' : '', disabled: !p.rendu, title: p.rendu ? '' : 'Pas encore de rendu final', onclick: () => changerLecteur('rendu') }, 'Rendu final'));
  let url = null;
  let vertical = false;
  if (E.lecteur === 'origine') {
    const rid = rushLecteur();
    const r = p.rushs.find((x) => x.id === rid);
    if (rid && E.vue.proxys?.includes(rid)) url = `/api/projets/${E.projetId}/rushs/${rid}/video`;
    vertical = r?.source ? r.source.hauteur > r.source.largeur : false;
  } else {
    const s = sortieAffichee();
    if (s) {
      url = `/api/projets/${E.projetId}/sorties/${s.fichier}`;
      vertical = (s.rapport.hauteur || 0) > (s.rapport.largeur || 1);
    }
  }
  const vide = n.ecran.querySelector('.vide');
  if (!url) {
    n.video.classList.add('cache');
    n.video.removeAttribute('src');
    delete n.video.dataset.url;
    const message = !p.rushs.length ? 'Déposez un rush pour commencer.'
      : !E.vue.pret_a_monter ? 'Analyse des rushs en cours…'
        : !p.montage ? 'Les rushs sont prêts. Cliquez sur « Proposer un montage » en bas de l’écran.'
          : 'Le premier aperçu se prépare…';
    if (vide) vide.lastChild.textContent = message;
    else n.ecran.append(h('div', { class: 'vide' }, icone('film'), h('p', {}, message)));
    n.ecran.classList.remove('vertical');
  } else {
    vide?.remove();
    n.video.classList.remove('cache');
    if (n.video.dataset.url !== url) { n.video.dataset.url = url; n.video.src = url; }
    n.ecran.classList.toggle('vertical', vertical);
  }
  dessinerOutilsLecteur();
  dessinerTimelines();
  dessinerStats();
  dessinerCalque();
}

function changerLecteur(mode, rush) {
  E.lecteur = mode;
  if (rush) E.rushLecteur = rush;
  dessinerLecteur();
}

function dessinerOutilsLecteur() {
  const n = E.noeuds;
  if (!n.outilsLecteur || !E.vue) return;
  const p = E.vue.projet;
  const enPause = n.video.paused && !!n.video.dataset.url;
  const commenter = h('button', {
    class: `btn petit ${enPause && p.montage ? 'doux' : ''}`, disabled: !enPause || !p.montage || !!E.commentaire,
    title: !p.montage ? 'Proposez d’abord un montage' : enPause ? 'Dites au LLM ce qu’il faut corriger à cet instant' : 'Mettez la vidéo en pause à l’endroit voulu',
    onclick: ouvrirCommentaire,
  }, icone('discussion'), 'Commenter ce moment');
  const choixRush = E.lecteur === 'origine' && p.rushs.length > 1
    ? h('select', { class: 'champ', style: { width: 'auto', marginTop: 0, padding: '5px 8px' }, onchange: (e) => changerLecteur('origine', e.target.value) },
        p.rushs.map((r) => h('option', { value: r.id, selected: r.id === rushLecteur() }, r.nom)))
    : null;
  const bascule = (cle, libelle, valeur) => h('label', { class: 'bascule', title: `Afficher ou masquer la ${libelle.toLowerCase()}` },
    h('input', { type: 'checkbox', checked: valeur, onchange: (e) => { E[cle] = e.target.checked; stocker(`edivid_${cle}`, e.target.checked ? '1' : '0'); dessinerTimelines(); } }),
    h('span', {}, libelle));
  remplir(n.outilsLecteur, commenter, choixRush, h('span', { class: 'espace' }),
    bascule('voirTlOrigine', 'Timeline d’origine', E.voirTlOrigine),
    bascule('voirTlMontage', 'Timeline du montage', E.voirTlMontage));
}

const NOMS_REGIME_LONG = { respiration: 'Respiration', rafale: 'Rafale', impact: 'Impact' };

/** Plans de la vidéo affichée (aperçu ou rendu) : ce qui a été réellement rendu. */
function plansAffiches() {
  const s = E.lecteur === 'rendu' ? E.vue.projet.rendu : E.vue.projet.apercu;
  return s?.rapport?.plans_detail || [];
}

function dessinerTimelines() {
  const n = E.noeuds;
  if (!n.timelines || !E.vue) return;
  const p = E.vue.projet;
  const prog = E.programme;
  const blocs = [];
  const s = p.apercu && E.lecteur !== 'rendu' ? p.apercu : (p.rendu || p.apercu);
  if (E.voirTlMontage && s) {
    const plans = s.rapport.plans_detail || [];
    const total = s.rapport.duree_montage || 1;
    const bande = h('div', { class: 'bande', role: 'img', 'aria-label': 'Timeline du montage, plan par plan' },
      plans.map((pl) => h('div', {
        class: `bloc ${pl.regime} ${pl.accroche ? 'accroche' : ''} ${pl.transition ? 'trans' : ''}`,
        style: { flexGrow: pl.d, flexBasis: 0 },
        title: `${minutes(pl.t)} · ${NOMS_REGIME_LONG[pl.regime]} · cadre ${pl.cadre} · ${fr(pl.d)} s${pl.accroche ? ' · accroche' : ''}${pl.transition ? ` · transition ${pl.transition}` : ''}`,
        onclick: () => { if (E.lecteur === 'origine') changerLecteur(p.apercu ? 'apercu' : 'rendu'); setTimeout(() => { n.video.currentTime = pl.t + 0.01; }, 50); },
      })));
    const repere = h('div', { class: 'reperes' });
    for (const i of prog?.incrustations || []) {
      const inc = p.incrustations.find((x) => x.id === i.id);
      repere.append(h('div', { class: 'repere', style: { left: `${(100 * i.debut) / total}%`, width: `${Math.max(0.4, (100 * (i.fin - i.debut)) / total)}%` },
        title: `${inc?.texte ? `Texte « ${inc.texte} »` : inc?.effet || p.medias.find((m) => m.id === inc?.media)?.nom || 'incrustation'} · ${minutes(i.debut)}` }));
    }
    const plage = E.proposition?.plage_sortie;
    n.teteMontage = h('div', { class: 'tete', style: { left: '0%' } });
    blocs.push(h('div', { class: 'piste-titre' }, h('b', {}, 'Montage'), h('span', { class: 'muet' }, ` · ${minutes(total)} · ${plans.length} plans`),
      E.lecteur === 'rendu' ? null : h('span', { class: 'muet' }, ' · dernier aperçu')),
    h('div', { class: 'piste-montage' }, bande, repere,
      plage ? h('div', { class: 'plage', style: { left: `${(100 * plage[0]) / total}%`, width: `${(100 * (plage[1] - plage[0])) / total}%` } }) : null,
      n.teteMontage));
  } else {
    n.teteMontage = null;
  }
  if (E.voirTlOrigine && prog?.rushs?.length) {
    n.tetesOrigine = {};
    for (const r of prog.rushs) {
      const rush = p.rushs.find((x) => x.id === r.id);
      const segments = prog.plans.filter((pl) => pl.rush === r.id && !pl.accroche);
      const garde = segments.reduce((a, pl) => a + pl.d, 0);
      const plage = E.proposition?.plage_source?.rush === r.id ? E.proposition.plage_source : null;
      const tete = h('div', { class: 'tete', style: { left: '0%' } });
      n.tetesOrigine[r.id] = tete;
      const piste = h('div', {
        class: 'piste', title: 'Cliquez pour lire la vidéo d’origine à cet endroit',
        onclick: (e) => {
          const b = piste.getBoundingClientRect();
          const t = ((e.clientX - b.left) / b.width) * r.duree;
          changerLecteur('origine', r.id);
          const aller = () => { n.video.currentTime = t; };
          if (n.video.readyState >= 1) setTimeout(aller, 30); else n.video.addEventListener('loadedmetadata', aller, { once: true });
        },
      },
      segments.map((pl) => h('div', { class: `segment ${pl.regime}`, style: { left: `${(100 * pl.src) / r.duree}%`, width: `${Math.max(0.15, (100 * pl.d) / r.duree)}%` } })),
      plage ? h('div', { class: 'plage', style: { left: `${(100 * plage.debut) / r.duree}%`, width: `${(100 * (plage.fin - plage.debut)) / r.duree}%` } }) : null,
      tete);
      blocs.push(h('div', { class: 'piste-titre' }, h('b', {}, prog.rushs.length > 1 ? `Origine — ${rush?.nom || ''}` : 'Origine'),
        h('span', { class: 'muet' }, ` · ${minutes(r.duree)} filmées · ${minutes(garde)} gardées par le script`)), piste);
    }
  } else {
    n.tetesOrigine = {};
  }
  if (blocs.length) {
    blocs.push(h('div', { class: 'legende' },
      h('span', {}, h('i', { style: { background: 'var(--impact)' } }), 'Impact'),
      h('span', {}, h('i', { style: { background: 'var(--rafale)' } }), 'Rafale'),
      h('span', {}, h('i', { style: { background: 'var(--respiration)' } }), 'Respiration'),
      h('span', {}, h('i', { style: { background: 'var(--surface-3)', outline: '1px solid var(--bord)' } }), 'Coupé'),
      h('span', {}, h('i', { style: { background: 'var(--accent)' } }), 'Incrustation'),
      E.proposition ? h('span', {}, h('i', { style: { background: 'color-mix(in srgb, var(--accent) 35%, transparent)', outline: '1px dashed var(--accent)' } }), 'Plage de la correction') : null));
  }
  remplir(n.timelines, blocs);
  suivreLecture();
}

function dessinerStats() {
  const n = E.noeuds;
  const p = E.vue.projet;
  const s = p.apercu || p.rendu;
  if (!s) { remplir(n.stats); return; }
  const r = s.rapport;
  const notes = p.montage?.notes || [];
  const avert = E.programme?.avertissements || r.avertissements || [];
  remplir(n.stats,
    h('div', { class: 'chiffres' },
      h('div', { class: 'chiffre' }, h('b', {}, minutes(r.duree_montage)), h('span', {}, `sur ${minutes(r.duree_source)} de rushs`)),
      h('div', { class: 'chiffre' }, h('b', {}, r.plans), h('span', {}, 'plans')),
      h('div', { class: 'chiffre' }, h('b', {}, `${fr(r.duree_moyenne_plan)} s`), h('span', {}, 'par plan en moyenne')),
      h('div', { class: 'chiffre' }, h('b', {}, r.zooms_animes + r.transitions), h('span', {}, 'zooms animés et transitions'))),
    avert.length ? h('div', { class: 'aide', style: { marginTop: '12px', background: 'var(--alerte-doux)' } }, icone('alerte'), h('span', {}, avert.join(' · '))) : null,
    notes.length ? h('details', { class: 'notes' }, h('summary', { style: { cursor: 'pointer' } }, h('b', {}, 'Notes du monteur')),
      h('ul', { style: { margin: '6px 0 0', paddingLeft: '18px' } }, notes.map((t) => h('li', {}, t)))) : null);
}

/** Instant affiché → phrase et décalage dans cette phrase (pour poser une incrustation). */
function instantVersPhrase() {
  const n = E.noeuds;
  const t = n.video.currentTime;
  const phrases = E.vue.phrases;
  if (E.lecteur === 'origine') {
    const rid = rushLecteur();
    const candidates = phrases.filter((ph) => ph.rush === rid);
    const ph = candidates.find((x) => t >= x.t0 && t <= x.t1) || candidates.reduce((m, x) => (!m || Math.abs(x.t0 - t) < Math.abs(m.t0 - t) ? x : m), null);
    return ph ? { phrase: ph, decalage: Math.max(0, t - ph.t0) } : null;
  }
  const pl = plansAffiches().find((x) => t >= x.t && t < x.t + x.d);
  if (!pl) return null;
  if (pl.accroche) return { accroche: true };
  const ph = phrases.find((x) => x.cle === pl.phrase);
  if (!ph) return null;
  const src = (pl.src ?? ph.t0) + (t - pl.t);
  return { phrase: ph, decalage: Math.max(0, src - ph.t0) };
}

/** Rectangle réellement occupé par l'image dans le lecteur (object-fit: contain). */
function rectImage() {
  const v = E.noeuds.video;
  const b = E.noeuds.ecran.getBoundingClientRect();
  const vw = v.videoWidth || 9;
  const vh = v.videoHeight || 16;
  const echelle = Math.min(b.width / vw, b.height / vh);
  const w = vw * echelle;
  const hh = vh * echelle;
  return { left: b.left + (b.width - w) / 2, top: b.top + (b.height - hh) / 2, width: w, height: hh, dx: (b.width - w) / 2, dy: (b.height - hh) / 2 };
}

async function deposerSurVideo(e) {
  const n = E.noeuds;
  e.preventDefault();
  n.indicateurDepot.classList.add('cache');
  const brut = e.dataTransfer.getData('application/x-edivid');
  if (!brut) return;
  if (!n.video.dataset.url) { toast('Ouvrez d’abord l’aperçu ou la vidéo d’origine.'); return; }
  const objet = JSON.parse(brut);
  const r = rectImage();
  const x = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
  const y = Math.min(1, Math.max(0, (e.clientY - r.top) / r.height));
  const lieu = instantVersPhrase();
  if (!lieu) { toast('Cet instant n’appartient à aucune phrase : avancez un peu la lecture.'); return; }
  if (lieu.accroche) { toast('Vous êtes dans l’accroche : posez plutôt l’élément dans le corps de la vidéo.'); return; }
  const corps = { phrase: lieu.phrase.cle, decalage: Math.round(lieu.decalage * 100) / 100, x, y, position: 'centre', ...objet };
  try {
    await api('POST', `/api/projets/${E.projetId}/incrustations`, corps);
    E.aMettreAJour = true;
    toast(`${objet.texte ? 'Texte posé' : 'Média posé'} à ${minutes(n.video.currentTime)}. Mettez à jour l’aperçu pour le voir rendu.`, 'ok');
  } catch (err) { toast(err.message, 'erreur'); }
}

/** Instant (dans la vue affichée) où une incrustation est visible, ou null. */
function incrustationVisible(inc, t) {
  const p = E.vue.projet;
  const ph = E.vue.phrases.find((x) => x.cle === inc.phrase);
  if (!ph) return false;
  if (E.lecteur === 'origine') {
    if (ph.rush !== rushLecteur()) return false;
    const debut = ph.t0 + (inc.decalage || 0);
    return t >= debut && t <= debut + inc.duree;
  }
  const placee = (E.programme?.incrustations || []).find((x) => x.id === inc.id);
  if (placee && E.lecteur === 'apercu' && !E.aMettreAJour) return t >= placee.debut && t <= placee.fin;
  // Repli : par les plans affichés.
  const src = ph.t0 + (inc.decalage || 0);
  const pl = plansAffiches().find((x) => !x.accroche && x.phrase === inc.phrase && src >= x.src && src < x.src + x.d)
    || plansAffiches().find((x) => !x.accroche && x.phrase === inc.phrase);
  if (!pl) return false;
  const debut = pl.t + Math.max(0, src - pl.src);
  return t >= debut && t <= debut + inc.duree;
  // (p reste utilisé par les appelants)
}

const CENTRES_NOMMES = { haut_gauche: [0.27, 0.16], haut_droite: [0.73, 0.16], bas_gauche: [0.27, 0.72], bas_droite: [0.73, 0.72], centre: [0.5, 0.5], plein: [0.5, 0.5], haut: [0.5, 0.18], bas: [0.5, 0.7] };

/** Calque d'édition, en pause : les textes et médias visibles à cet instant, déplaçables. */
function dessinerCalque() {
  const n = E.noeuds;
  if (!n.calque || !E.vue) return;
  const p = E.vue.projet;
  if (!n.video.paused || !n.video.dataset.url) { remplir(n.calque); return; }
  const r = rectImage();
  Object.assign(n.calque.style, { left: `${r.dx}px`, top: `${r.dy}px`, width: `${r.width}px`, height: `${r.height}px` });
  const t = n.video.currentTime;
  const visibles = p.incrustations.filter((i) => !i.effet && incrustationVisible(i, t));
  remplir(n.calque, visibles.map((inc) => {
    const [cx, cy] = inc.x != null ? [inc.x, inc.y] : (CENTRES_NOMMES[inc.position] || (inc.texte ? [0.5, 0.22] : [0.5, 0.5]));
    let contenu;
    const style = { left: `${cx * 100}%`, top: `${cy * 100}%` };
    if (inc.texte) {
      contenu = h('span', { class: `texte-calque couleur-${inc.couleur || 'blanc'} fond-${inc.fond || 'aucun'}`, style: { fontSize: `${r.height * (0.02 + 0.1 * inc.taille)}px` } }, inc.texte);
    } else {
      const m = p.medias.find((x) => x.id === inc.media);
      if (!m) return null;
      const url = `/api/projets/${E.projetId}/medias/${m.id}`;
      style.width = inc.position === 'plein' && inc.x == null ? '100%' : `${inc.taille * 100}%`;
      contenu = m.genre === 'video' ? h('video', { src: url, muted: true, autoplay: true, loop: true, playsinline: true }) : h('img', { src: url, alt: m.nom, draggable: 'false' });
    }
    const el = h('div', { class: 'element-calque', style, title: 'Glissez pour déplacer' }, contenu,
      h('button', { class: 'supprimer-calque', title: 'Retirer', onpointerdown: (e) => e.stopPropagation(), onclick: async (e) => {
        e.stopPropagation();
        await api('DELETE', `/api/projets/${E.projetId}/incrustations/${inc.id}`);
        E.aMettreAJour = true;
      } }, icone('fermer')));
    el.addEventListener('pointerdown', (e) => deplacerElement(e, el, inc));
    return el;
  }));
}

function deplacerElement(e, el, inc) {
  e.preventDefault();
  const calque = E.noeuds.calque.getBoundingClientRect();
  el.setPointerCapture(e.pointerId);
  let x = null;
  let y = null;
  const bouger = (ev) => {
    x = Math.min(1, Math.max(0, (ev.clientX - calque.left) / calque.width));
    y = Math.min(1, Math.max(0, (ev.clientY - calque.top) / calque.height));
    el.style.left = `${x * 100}%`;
    el.style.top = `${y * 100}%`;
  };
  const lacher = async () => {
    el.removeEventListener('pointermove', bouger);
    el.removeEventListener('pointerup', lacher);
    if (x === null) return;
    try {
      await api('POST', `/api/projets/${E.projetId}/incrustations/${inc.id}`, { x, y });
      E.aMettreAJour = true;
      dessinerBarre();
    } catch (err) { toast(err.message, 'erreur'); }
  };
  el.addEventListener('pointermove', bouger);
  el.addEventListener('pointerup', lacher);
}

function suivreLecture() {
  const n = E.noeuds;
  if (!E.vue || !n.video) return;
  const t = n.video.currentTime;
  const p = E.vue.projet;
  // Têtes de lecture : celle de la vue jouée, et sa correspondance sur l'autre timeline.
  const sMontage = p.apercu && E.lecteur !== 'rendu' ? p.apercu : (p.rendu || p.apercu);
  const totalMontage = sMontage?.rapport?.duree_montage || 1;
  let tMontage = null;
  let srcRush = null;
  let srcT = null;
  if (E.lecteur === 'origine') {
    srcRush = rushLecteur();
    srcT = t;
    const pl = (sMontage?.rapport?.plans_detail || []).find((x) => !x.accroche && x.rush === srcRush && t >= x.src && t < x.src + x.d);
    if (pl) tMontage = pl.t + (t - pl.src);
  } else {
    tMontage = t;
    const pl = plansAffiches().find((x) => t >= x.t && t < x.t + x.d);
    if (pl && pl.src != null) { srcRush = pl.rush; srcT = pl.src + (t - pl.t); }
  }
  if (n.teteMontage) {
    n.teteMontage.style.display = tMontage === null ? 'none' : '';
    if (tMontage !== null) n.teteMontage.style.left = `${(100 * tMontage) / totalMontage}%`;
  }
  for (const [rid, tete] of Object.entries(n.tetesOrigine || {})) {
    const r = E.programme?.rushs?.find((x) => x.id === rid);
    tete.style.display = rid === srcRush && r ? '' : 'none';
    if (rid === srcRush && r) tete.style.left = `${(100 * srcT) / r.duree}%`;
  }
  // Phrase en cours dans le texte.
  let cle = null;
  if (E.lecteur === 'origine') {
    cle = E.vue.phrases.find((ph) => ph.rush === srcRush && t >= ph.t0 && t <= ph.t1)?.cle || null;
  } else {
    const pl = plansAffiches().find((x) => t >= x.t && t < x.t + x.d);
    cle = pl && !pl.accroche ? pl.phrase : null;
  }
  if (cle !== E.phraseJouee) {
    E.phraseJouee = cle;
    E.noeuds.listePhrases?.querySelectorAll('.phrase.joue').forEach((x) => x.classList.remove('joue'));
    const el = cle ? E.noeuds.listePhrases?.querySelector(`[data-cle="${CSS.escape(cle)}"]`) : null;
    if (el) {
      el.classList.add('joue');
      const liste = E.noeuds.listePhrases;
      if (el.offsetTop < liste.scrollTop || el.offsetTop > liste.scrollTop + liste.clientHeight - 60) liste.scrollTo({ top: el.offsetTop - 80, behavior: 'smooth' });
    }
  }
}

function allerALaPhrase(cle) {
  const n = E.noeuds;
  const ph = E.vue.phrases.find((x) => x.cle === cle);
  if (E.lecteur === 'origine' && ph) {
    changerLecteur('origine', ph.rush);
    setTimeout(() => { n.video.currentTime = ph.t0; n.video.play(); }, 50);
    return;
  }
  const pl = plansAffiches().find((x) => x.phrase === cle && !x.accroche);
  if (pl) { n.video.currentTime = pl.t + 0.01; n.video.play(); return; }
  if (ph && E.vue.proxys?.includes(ph.rush)) {
    toast('Phrase coupée du montage : lecture dans la vidéo d’origine.');
    changerLecteur('origine', ph.rush);
    setTimeout(() => { n.video.currentTime = ph.t0; n.video.play(); }, 80);
  } else toast('Cette phrase n’est pas dans l’aperçu actuel.');
}

// ------------------------------------------------------------------ commentaire sur un instant

const EXEMPLES_COMMENTAIRE = ['Coupe cette phrase', 'Zoom serré sur ses yeux ici', 'Ajoute le texte « … » à cet endroit', 'C’est trop long, accélère ce passage', 'Laisse respirer ce moment, c’est émouvant'];

function ouvrirCommentaire() {
  const n = E.noeuds;
  const t = n.video.currentTime;
  E.commentaire = { t, vue: E.lecteur, rush: E.lecteur === 'origine' ? rushLecteur() : '' };
  E.proposition = null;
  dessinerCommentaire();
  dessinerOutilsLecteur();
}

function fermerCommentaire() {
  E.commentaire = null;
  if (E.proposition) api('DELETE', `/api/projets/${E.projetId}/commentaire/${E.proposition.id}`).catch(() => {});
  E.proposition = null;
  remplir(E.noeuds.zoneCommentaire);
  dessinerOutilsLecteur();
  dessinerTimelines();
}

function dessinerCommentaire(etape = 'saisie') {
  const n = E.noeuds;
  const c = E.commentaire;
  if (!c) { remplir(n.zoneCommentaire); return; }
  const prop = E.proposition;
  const zone = h('textarea', { class: 'champ', rows: '2', placeholder: prop ? 'Précisez ce qui ne va pas dans la proposition…' : 'Que voulez-vous changer à ce moment précis ?' });
  const envoyerCommentaire = async () => {
    const texte = zone.value.trim();
    if (!texte) { zone.focus(); return; }
    const precedente = E.proposition?.id || null;
    dessinerCommentaire('attente');
    try {
      E.proposition = await api('POST', `/api/projets/${E.projetId}/commentaire`, { t: c.t, vue: c.vue, rush: c.rush, texte, precedente });
      dessinerCommentaire('proposition');
      dessinerTimelines();
    } catch (err) {
      toast(err.message, 'erreur');
      dessinerCommentaire(prop ? 'proposition' : 'saisie');
    }
  };
  zone.addEventListener('keydown', (e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); envoyerCommentaire(); } });
  const entete = h('div', { class: 'commentaire-tete' },
    h('b', {}, `Commentaire à ${minutes(c.t)}`),
    h('span', { class: 'muet petit' }, c.vue === 'origine' ? ' · dans la vidéo d’origine' : ' · dans le montage'),
    h('span', { class: 'espace' }),
    h('button', { class: 'btn icone fantome', title: 'Fermer', onclick: fermerCommentaire }, icone('fermer')));
  let corps;
  if (etape === 'attente') {
    corps = h('div', { class: 'ecrit', style: { padding: '10px 0' } }, h('i'), h('i'), h('i'), h('span', { style: { marginLeft: '6px' } }, 'EDIVID reformule et prépare une correction…'));
  } else if (etape === 'proposition' && prop) {
    const plage = prop.plage_sortie ? `de ${minutes(prop.plage_sortie[0])} à ${minutes(prop.plage_sortie[1])} dans le montage` : `phrases ${prop.de} à ${prop.a}`;
    corps = h('div', {},
      h('p', { class: 'reformulation' }, prop.reformulation),
      prop.explication ? h('p', { class: 'petit muet', style: { margin: '4px 0 8px' } }, prop.explication) : null,
      !prop.possible ? h('div', { class: 'aide', style: { background: 'var(--alerte-doux)', marginBottom: '8px' } }, icone('alerte'), h('span', {}, 'EDIVID ne peut pas faire exactement cela ; voici ce qu’il propose à la place.')) : null,
      h('div', { class: 'petit', style: { marginBottom: '6px' } }, h('b', {}, 'Correction proposée '), h('span', { class: 'muet' }, `(${plage}, surlignée sur les timelines)`)),
      prop.actions.length
        ? h('ul', { class: 'liste-actions' }, prop.actions.map((a) => h('li', { class: a.startsWith('non appliqué') ? 'non' : '' }, a)))
        : h('p', { class: 'petit muet' }, 'Aucun changement : précisez votre demande.'),
      etape === 'proposition' && E.correction ? h('div', { style: { marginTop: '8px' } }, zone) : null,
      h('div', { class: 'commentaire-boutons' },
        E.correction
          ? h('button', { class: 'btn primaire', onclick: () => { E.correction = false; envoyerCommentaire(); } }, icone('envoyer'), 'Reproposer')
          : h('button', { class: 'btn primaire', disabled: !prop.actions.some((a) => !a.startsWith('non appliqué')), onclick: async () => {
              try {
                await api('POST', `/api/projets/${E.projetId}/commentaire/${prop.id}/valider`);
                toast('Correction appliquée : un nouvel aperçu se prépare.', 'ok');
                E.proposition = null;
                E.commentaire = null;
                remplir(n.zoneCommentaire);
                dessinerOutilsLecteur();
                dessinerTimelines();
              } catch (err) { toast(err.message, 'erreur'); }
            } }, icone('ok'), 'Valider'),
        h('button', { class: 'btn', onclick: () => { E.correction = !E.correction; dessinerCommentaire('proposition'); if (E.correction) setTimeout(() => zone.focus(), 30); } }, E.correction ? 'Ne pas corriger' : 'Corriger'),
        h('button', { class: 'btn fantome', onclick: fermerCommentaire }, 'Annuler')));
  } else {
    corps = h('div', {},
      zone,
      h('div', { class: 'suggestions', style: { padding: '8px 0 0' } }, EXEMPLES_COMMENTAIRE.map((ex) => h('button', { onclick: () => {
        zone.value = ex;
        zone.focus();
        const d = ex.indexOf('…');
        if (d >= 0) zone.setSelectionRange(d, d + 1);
      } }, ex))),
      h('div', { class: 'commentaire-boutons' },
        h('button', { class: 'btn primaire', onclick: envoyerCommentaire }, icone('envoyer'), 'Proposer une correction'),
        h('button', { class: 'btn fantome', onclick: fermerCommentaire }, 'Annuler')));
    setTimeout(() => zone.focus(), 30);
  }
  remplir(n.zoneCommentaire, h('div', { class: 'commentaire' }, entete, corps));
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
    const corrigee = !!p.corrections?.[ph.cle];
    if (E.edition === ph.cle) return ligneEdition(ph, corrigee);
    return h('div', { class: `phrase ${classe}`, 'data-cle': ph.cle },
      h('div', { class: 'temps' }, minutes(ph.t0)),
      h('div', { style: { minWidth: 0 } },
        h('div', { class: 'texte', style: { cursor: 'pointer' }, title: 'Clic : écouter ce passage — double-clic : corriger le texte',
          onclick: () => allerALaPhrase(ph.cle), ondblclick: () => { E.edition = ph.cle; dessinerPhrases(); } },
          ph.texte, corrigee ? h('span', { class: 'marque-corrige', title: 'Texte corrigé à la main' }, 'corrigé') : null),
        d && d.garder ? h('div', { class: 'etiquettes' },
          h('button', { class: `puce ${d.regime}`, title: 'Régime de la phrase — cliquez pour changer', onclick: () => changerPhrase(ph.cle, { regime: REGIMES[(REGIMES.indexOf(d.regime) + 1) % 3] }) }, NOMS_REGIME[d.regime]),
          h('button', { class: 'puce', title: 'Cadre — cliquez pour changer', onclick: () => changerPhrase(ph.cle, { cadre: CADRES[(CADRES.indexOf(d.cadre) + 1) % 4] }) }, NOMS_CADRE[d.cadre]),
          p.incrustations.filter((i) => i.phrase === ph.cle).length ? h('span', { class: 'puce', style: { cursor: 'default' } }, `✦ ${p.incrustations.filter((i) => i.phrase === ph.cle).length} incrustation(s)`) : null) : null),
      monte ? h('div', { class: 'boutons' },
        h('button', { class: 'btn icone fantome', title: 'Corriger le texte (orthographe, mot mal transcrit)', onclick: () => { E.edition = ph.cle; dessinerPhrases(); } }, icone('crayon')),
        h('button', { class: 'btn icone fantome', title: 'Ajouter une incrustation (image, GIF, vidéo, effet)', onclick: () => dialogueIncrustation(ph) }, icone('baguette')),
        h('button', { class: 'btn icone fantome', title: d && !d.garder ? 'Rétablir cette phrase' : 'Couper cette phrase', onclick: () => changerPhrase(ph.cle, { garder: !(d ? d.garder : true) }) },
          icone(d && !d.garder ? 'relancer' : 'ciseaux'))) : h('div', { class: 'boutons' },
        h('button', { class: 'btn icone fantome', title: 'Corriger le texte', onclick: () => { E.edition = ph.cle; dessinerPhrases(); } }, icone('crayon'))));
  }));
  appliquerReplie(E.noeuds.phrases, 'phrases');
  remplir(E.noeuds.phrases,
    enteteRepliable('phrases', '3. Le texte de la vidéo',
      monte ? h('div', { class: 'onglets-petits' },
        [['toutes', 'Toutes'], ['gardees', 'Gardées'], ['coupees', `Coupées (${nbCoupees})`]].map(([v, l]) =>
          h('button', { class: E.filtre === v ? 'actif' : '', onclick: () => { E.filtre = v; dessinerPhrases(); } }, l))) : null),
    h('div', { class: 'carte-corps' },
      phrases.length === 0 ? h('p', { class: 'muet petit', style: { margin: 0 } }, 'La transcription apparaîtra ici après l’analyse.')
        : !monte ? h('div', { class: 'aide', style: { marginBottom: '10px' } }, icone('info'), h('span', {}, 'Voici ce qui est dit dans vos rushs. Proposez un montage : le LLM décidera, phrase par phrase, ce qu’il garde, le rythme et le cadre.'))
          : h('p', { class: 'tres-petit muet', style: { margin: '0 0 8px' } }, 'Cliquez une phrase pour l’écouter, double-cliquez pour corriger son texte ; ses étiquettes changent le régime ou le cadre ; les ciseaux la coupent ; la baguette pose une incrustation.'),
      phrases.length ? zoneRemplacer() : null,
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
  n.garder = h('button', { class: 'btn icone fantome', title: 'Enregistrer cette demande dans ma bibliothèque', onclick: enregistrerDemande }, icone('etoile'));
  n.discussion = h('div', { class: 'discussion' }, n.messages, n.suggestions,
    h('div', { class: 'saisie' },
      h('button', { class: 'btn icone fantome', title: 'Bibliothèque de demandes', onclick: () => afficherBibliotheque() }, icone('livre')),
      n.saisie, n.garder, n.envoyer));
  remplir(n.panneau, n.onglets, n.corpsPanneau);
}

function dessinerPanneau() {
  const n = E.noeuds;
  const p = E.vue.projet;
  const onglets = [
    ['assistant', 'discussion', 'Assistant', 0],
    ['reglages', 'reglages', 'Réglages', 0],
    ['medias', 'image', 'Médias', p.medias.length],
    ['effets', 'baguette', 'Textes', p.incrustations.length],
    ['postprod', 'etincelles', 'PostProd', 0],
  ];
  appliquerReplie(n.panneau, 'panneau');
  remplir(n.onglets, ...onglets.map(([id, ic, l, compte]) =>
    h('button', { role: 'tab', class: E.onglet === id ? 'actif' : '', 'aria-selected': E.onglet === id ? 'true' : 'false',
      onclick: () => { E.onglet = id; if (estReplie('panneau')) { stocker('edivid_replie_panneau', '0'); } dessinerPanneau(); } }, icone(ic), l, compte ? h('span', { class: 'compte' }, compte) : null)),
    h('button', { class: 'chevron-panneau', title: 'Replier ou déplier ce panneau', 'aria-expanded': estReplie('panneau') ? 'false' : 'true',
      onclick: () => { stocker('edivid_replie_panneau', estReplie('panneau') ? '0' : '1'); dessinerPanneau(); } }, icone('chevron')));
  if (E.onglet === 'assistant') {
    dessinerDiscussion();
    if (n.corpsPanneau.firstChild !== n.discussion) remplir(n.corpsPanneau, n.discussion);
    return;
  }
  const corps = h('div', { class: 'onglet-corps' });
  if (E.onglet === 'reglages') dessinerReglages(corps);
  if (E.onglet === 'medias') dessinerMedias(corps);
  if (E.onglet === 'effets') dessinerEffets(corps);
  if (E.onglet === 'postprod') dessinerPostProd(corps);
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
    const unite = r.bornes.unite || '%';
    const sortie = h('b', { class: 'petit', style: { minWidth: '62px', textAlign: 'right' } }, `${valeur > 0 && unite === 'ms' ? '+' : ''}${valeur} ${unite}`);
    return h('div', { style: { display: 'flex', alignItems: 'center', gap: '10px', marginTop: '8px' } },
      h('input', { type: 'range', min: r.bornes.min, max: r.bornes.max, step: unite === 'ms' ? '10' : '5', value: valeur, 'aria-label': r.titre,
        oninput: (e) => { sortie.textContent = `${e.target.value > 0 && unite === 'ms' ? '+' : ''}${e.target.value} ${unite}`; }, onchange: (e) => changer(Number(e.target.value)) }), sortie);
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

function sectionConsignes(corps) {
  const p = E.vue.projet;
  const zone = h('textarea', { class: 'champ', rows: '4', maxlength: '4000', placeholder: 'Par exemple : coupes franches, jamais de glitch ; garder les anecdotes personnelles.', style: { marginTop: '8px', resize: 'vertical' } });
  zone.value = p.consignes || '';
  const enregistrer = async () => {
    if (zone.value === (E.vue.projet.consignes || '')) return;
    try {
      await api('POST', `/api/projets/${E.projetId}/consignes`, { texte: zone.value });
      E.vue.projet.consignes = zone.value;
      toast('Consignes enregistrées : elles s’appliquent au prochain montage et dans la conversation.');
    } catch (e) { toast(e.message, 'erreur'); }
  };
  zone.addEventListener('blur', enregistrer);
  const exemples = h('div', { class: 'suggestions', style: { padding: '8px 0 0' } });
  chargerBibliotheque().then((b) => remplir(exemples, (b.integree.consignes || []).map((c) =>
    h('button', { title: c.texte, onclick: () => { zone.value = (zone.value.trim() ? `${zone.value.trim()}\n` : '') + c.texte; enregistrer(); } }, `+ ${c.titre}`))));
  corps.append(h('div', { class: 'section-reglages' }, h('h3', {}, 'Vos consignes au monteur'),
    h('div', { class: 'reglage-aide' }, 'Vos préférences, en toutes lettres. Le LLM les lit à chaque montage et dans la conversation, en plus de ses propres consignes.'),
    zone, exemples,
    h('button', { class: 'btn petit fantome', style: { marginTop: '6px' }, onclick: afficherCoulisses }, icone('livre'), 'Voir les consignes complètes du LLM')));
}

function dessinerReglages(corps) {
  const p = E.vue.projet;
  sectionConsignes(corps);
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
    p.medias.some((m) => m.genre !== 'son') ? h('p', { class: 'tres-petit muet', style: { margin: '10px 0 0' } }, 'Astuce : mettez la vidéo en pause, puis glissez une image ou un GIF directement sur la vidéo, à l’endroit voulu.') : null,
    p.medias.length ? h('div', { class: 'grille-medias' }, p.medias.map((m) =>
      h('div', { class: `media ${m.genre !== 'son' ? 'deplacable' : ''}`, draggable: m.genre !== 'son' ? 'true' : null,
        ondragstart: (e) => {
          e.dataTransfer.setData('application/x-edivid', JSON.stringify({ media: m.id, taille: 0.35, duree: Math.min(m.duree || 3, 4) || 3, entree: 'splash' }));
          e.dataTransfer.effectAllowed = 'copy';
        } },
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
  ['haut_gauche', 'Haut gauche'], ['haut', 'Haut'], ['haut_droite', 'Haut droite'],
  ['bas_gauche', 'Bas gauche'], ['centre', 'Centre'], ['bas_droite', 'Bas droite'],
  ['plein', 'Plein écran'], ['bas', 'Bas'],
];
const NOMS_POSITION = Object.fromEntries(POSITIONS);
const EFFETS = { secousse: ['Secousse', 'L’image tremble : effet comique, surprise.'], noir_blanc: ['Noir et blanc', 'Le temps d’un instant dramatique.'] };

const EFFETS_TEXTE = [['splash', 'Splash'], ['fondu', 'Fondu'], ['glisse_gauche', 'Glissé gauche'], ['glisse_droite', 'Glissé droite'],
  ['glisse_haut', 'Glissé haut'], ['machine', 'Machine à écrire'], ['aucune', 'Sans effet']];

function moduleTexte(corps) {
  const T = E.texteModule = E.texteModule || { texte: '', couleur: 'jaune', fond: 'aucun', taille: 0.45, entree: 'splash', duree: 2.5 };
  const apercu = h('div', { class: 'bloc-texte-zone' });
  const dessinerApercu = () => {
    const bloc = h('div', {
      class: `bloc-texte texte-calque couleur-${T.couleur} fond-${T.fond} anim-${T.entree}`, draggable: 'true',
      title: 'Glissez ce bloc sur la vidéo (en pause), à l’endroit voulu',
      style: { fontSize: `${14 + 26 * T.taille}px` },
      ondragstart: (e) => {
        if (!T.texte.trim()) { e.preventDefault(); toast('Écrivez d’abord le texte.'); return; }
        e.dataTransfer.setData('application/x-edivid', JSON.stringify({ texte: T.texte, couleur: T.couleur, fond: T.fond, taille: T.taille, entree: T.entree, duree: T.duree }));
        e.dataTransfer.effectAllowed = 'copy';
      },
    }, T.texte.trim() || 'Votre texte');
    remplir(apercu, bloc, h('span', { class: 'poignee tres-petit muet' }, '⠿ glisser sur la vidéo'));
  };
  const zone = h('textarea', { class: 'champ', rows: '2', maxlength: '300', placeholder: 'Par exemple : ABONNEZ-VOUS !' });
  zone.value = T.texte;
  zone.addEventListener('input', () => { T.texte = zone.value; dessinerApercu(); });
  const segments = (cle, options) => {
    const el = h('div', { class: 'segments' });
    const dessiner = () => remplir(el, options.map(([v, l]) => h('button', { class: T[cle] === v ? 'actif' : '', onclick: () => { T[cle] = v; dessiner(); dessinerApercu(); } }, l)));
    dessiner();
    return el;
  };
  const effet = h('select', { class: 'champ' }, EFFETS_TEXTE.map(([v, l]) => h('option', { value: v, selected: T.entree === v }, l)));
  effet.addEventListener('change', () => { T.entree = effet.value; dessinerApercu(); });
  const libDuree = h('label', {}, `Durée : ${fr(T.duree)} s`);
  const libTaille = h('label', {}, 'Taille');
  dessinerApercu();
  corps.append(h('div', { class: 'section-reglages' },
    h('h3', {}, 'Composer un texte'),
    h('div', { class: 'reglage-aide' }, 'Écrivez, choisissez l’allure et l’effet, puis glissez le bloc sur la vidéo mise en pause : l’endroit du dépôt fixe la position, l’instant de la vidéo fixe le moment.'),
    h('div', { class: 'ligne-champ' }, zone),
    h('div', { class: 'ligne-champ' }, h('label', {}, 'Couleur'), segments('couleur', [['blanc', 'Blanc'], ['jaune', 'Jaune'], ['noir', 'Noir']])),
    h('div', { class: 'ligne-champ' }, h('label', {}, 'Fond'), segments('fond', [['aucun', 'Contour'], ['boite', 'Boîte'], ['bandeau', 'Bandeau']])),
    h('div', { class: 'ligne-champ' }, libTaille, h('input', { type: 'range', min: '0.1', max: '1', step: '0.05', value: T.taille, oninput: (e) => { T.taille = Number(e.target.value); dessinerApercu(); } })),
    h('div', { class: 'ligne-champ' }, h('label', {}, 'Effet d’entrée'), effet),
    h('div', { class: 'ligne-champ' }, libDuree, h('input', { type: 'range', min: '0.5', max: '8', step: '0.5', value: T.duree, oninput: (e) => { T.duree = Number(e.target.value); libDuree.textContent = `Durée : ${fr(T.duree)} s`; } })),
    apercu,
    h('div', { style: { display: 'flex', gap: '8px', flexWrap: 'wrap' } },
      h('button', { class: 'btn petit', onclick: dessinerApercu, title: 'Rejouer l’effet dans l’aperçu' }, icone('lecture'), 'Voir l’effet'),
      h('button', { class: 'btn petit doux', onclick: async () => {
        if (!T.texte.trim()) { toast('Écrivez d’abord le texte.'); return; }
        const n = E.noeuds;
        if (!n.video.dataset.url || !n.video.paused) { toast('Mettez la vidéo en pause à l’instant voulu.'); return; }
        const lieu = instantVersPhrase();
        if (!lieu || lieu.accroche) { toast('Choisissez un instant dans le corps de la vidéo.'); return; }
        try {
          await api('POST', `/api/projets/${E.projetId}/incrustations`, { texte: T.texte, couleur: T.couleur, fond: T.fond, taille: T.taille, entree: T.entree,
            duree: T.duree, phrase: lieu.phrase.cle, decalage: Math.round(lieu.decalage * 100) / 100, x: 0.5, y: 0.22, position: 'centre' });
          E.aMettreAJour = true;
          toast(`Texte posé à ${minutes(n.video.currentTime)}.`, 'ok');
        } catch (e) { toast(e.message, 'erreur'); }
      } }, icone('plus'), 'Poser à l’instant affiché'))));
}

function dessinerEffets(corps) {
  const p = E.vue.projet;
  moduleTexte(corps);
  corps.append(h('h3', { style: { margin: '4px 0 10px', textTransform: 'uppercase', fontSize: '11.5px', letterSpacing: '.08em', color: 'var(--texte-3)' } }, 'Posés dans la vidéo'));
  const texte = Object.fromEntries(E.vue.phrases.map((ph) => [ph.cle, ph]));
  corps.append(h('div', { class: 'aide', style: { marginBottom: '12px' } }, icone('info'),
    h('span', {}, 'Une incrustation pose une image, un GIF, une vidéo ou un effet sur une phrase précise. Pour en ajouter une : la baguette à côté d’une phrase, ou demandez à l’assistant (« mets le GIF du chat quand elle parle de montgolfière »).')));
  if (!p.incrustations.length) {
    corps.append(h('p', { class: 'muet petit' }, 'Rien pour l’instant : composez un texte ci-dessus, glissez un média depuis l’onglet Médias, ou utilisez la baguette à côté d’une phrase.'));
    return;
  }
  for (const inc of p.incrustations) {
    const media = p.medias.find((m) => m.id === inc.media);
    const quoi = inc.texte ? `Texte « ${inc.texte} »` : inc.effet ? EFFETS[inc.effet]?.[0] : media?.nom || '(média retiré)';
    const ph = texte[inc.phrase];
    corps.append(h('div', { class: 'incrustation' },
      h('div', { style: { minWidth: 0 } },
        h('b', { class: 'petit' }, `✦ ${quoi}`),
        h('div', { class: 'tres-petit muet', style: { marginTop: '3px' } }, ph ? `« ${ph.texte.slice(0, 90)}${ph.texte.length > 90 ? '…' : ''} »` : 'phrase introuvable'),
        h('div', { class: 'tres-petit muet' }, `${inc.decalage ? `après ${fr(inc.decalage)} s · ` : ''}${fr(inc.duree)} s${inc.effet ? '' : inc.x != null ? ' · position libre' : ` · ${NOMS_POSITION[inc.position] || inc.position}`}${inc.entree ? ` · ${(EFFETS_TEXTE.find(([v]) => v === inc.entree) || [, inc.entree])[1].toLowerCase()}` : ''}${inc.fond_vert ? ' · fond vert retiré' : ''}`)),
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
    p.rendu ? h('a', { class: 'btn doux', href: `/api/projets/${E.projetId}/sorties/${p.rendu.fichier}`, download: `${p.nom}.mp4` }, icone('telecharger'), h('span', { class: 'long' }, 'Télécharger'), h('span', { class: 'court' }, 'Fichier')) : null,
    p.rendu ? h('button', { class: 'btn doux', title: 'Versions prêtes pour YouTube, pour l’iPhone ou l’archive ; publication sur YouTube', onclick: afficherExport },
      icone('televerser'), h('span', { class: 'long' }, 'Exporter'), h('span', { class: 'court' }, 'Export')) : null,
    p.rendu && E.driveActif ? h('button', {
      class: 'btn doux', title: p.depot_drive ? `Déjà déposée : ${p.depot_drive}` : 'Déposer la vidéo finale dans le dossier de sortie du Drive',
      onclick: async () => { try { await api('POST', `/api/projets/${E.projetId}/drive`); toast('Envoi sur le Google Drive…'); } catch (e) { toast(e.message, 'erreur'); } },
    }, icone('nuage'), h('span', { class: 'long' }, p.depot_drive ? 'Redéposer sur Drive' : 'Envoyer sur Drive'), h('span', { class: 'court' }, 'Drive')) : null);
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
    h('div', { class: 'aide', style: { marginTop: '8px' } }, icone('discussion'),
      h('span', {}, 'Une retouche précise ? Mettez l’aperçu en pause et cliquez « Commenter ce moment » : le LLM reformule, propose une correction sur la bonne plage (surlignée sur les timelines), vous validez, corrigez ou annulez.')),
    h('div', { class: 'aide', style: { marginTop: '8px' } }, icone('baguette'),
      h('span', {}, 'Onglet Textes : écrivez un texte, choisissez son effet, puis glissez-le sur la vidéo en pause, à l’endroit voulu. Les images et GIF de l’onglet Médias se glissent de la même façon. En pause, tout ce qui est posé se déplace à la souris.')),
    h('div', { class: 'aide', style: { marginTop: '8px' } }, icone('film'),
      h('span', {}, 'Sous le lecteur, deux timelines : la vidéo d’origine (ce que le script garde, en couleur ; ce qu’il coupe, en gris) et le montage. Chacune s’allume ou s’éteint ; « Origine » relit la vidéo telle que filmée.')),
    h('div', { class: 'aide', style: { marginTop: '8px' } }, icone('etincelles'),
      h('span', {}, 'Onglet PostProd : un look en un clic (Beauté naturelle, Glow peau, Éditorial mode…), puis teint, lumière, grain à doser ; « Comparer » montre l’avant/après sur l’image affichée.')),
    h('div', { class: 'aide', style: { marginTop: '8px' } }, icone('televerser'),
      h('span', {}, 'Après le rendu final, « Exporter » donne une version prête pour YouTube (ou légère pour l’iPhone) et la publie sur votre chaîne si votre compte Google l’autorise.')),
    h('div', { class: 'aide', style: { marginTop: '8px' } }, icone('livre'),
      h('span', {}, 'Le livre à côté de la saisie ouvre la bibliothèque : des demandes toutes prêtes, les vôtres, et les consignes exactes du LLM. L’étoile enregistre la demande que vous venez d’écrire.')),
    h('div', { class: 'aide', style: { marginTop: '8px' } }, icone('nuage'),
      h('span', {}, 'Depuis l’iPhone : déposez la vidéo dans le dossier Google Drive « À monter ». EDIVID la monte seul et dépose le résultat dans « Montages » (Paramètres, en haut à droite).')),
    h('div', { style: { display: 'flex', justifyContent: 'flex-end', marginTop: '16px' } },
      h('button', { class: 'btn primaire', onclick: fermer }, 'C’est parti'))), { large: true });
}

// ------------------------------------------------------------------ bibliothèque de demandes

let bibliotheque = null;
async function chargerBibliotheque(forcer = false) {
  if (!bibliotheque || forcer) bibliotheque = await api('GET', '/api/bibliotheque');
  return bibliotheque;
}

/** Place une demande dans la saisie ; le premier [crochet] est sélectionné, prêt à remplacer. */
function insererDemande(texte) {
  const n = E.noeuds;
  E.onglet = 'assistant';
  dessinerPanneau();
  n.saisie.value = texte;
  n.saisie.dispatchEvent(new Event('input'));
  n.saisie.focus();
  const debut = texte.indexOf('[');
  const fin = texte.indexOf(']', debut);
  if (debut >= 0 && fin > debut) n.saisie.setSelectionRange(debut, fin + 1);
  else n.saisie.setSelectionRange(texte.length, texte.length);
}

function carteDemande(d, fermer, retirer) {
  const aCompleter = /\[[^\]]+\]/.test(d.texte);
  return h('div', { class: 'incrustation', style: { gridTemplateColumns: 'minmax(0,1fr) auto' } },
    h('div', { style: { minWidth: 0 } },
      h('b', { class: 'petit' }, d.titre),
      h('div', { class: 'petit', style: { marginTop: '3px' } }, d.texte),
      d.aide ? h('div', { class: 'tres-petit muet', style: { marginTop: '3px' } }, d.aide) : null),
    h('div', { style: { display: 'flex', flexDirection: 'column', gap: '4px' } },
      h('button', { class: 'btn petit doux', onclick: () => { fermer(); insererDemande(d.texte); } }, aCompleter ? 'Compléter' : 'Utiliser'),
      !aCompleter && E.vue?.projet.montage ? h('button', { class: 'btn petit', onclick: () => { fermer(); envoyer(d.texte); } }, 'Envoyer') : null,
      retirer ? h('button', { class: 'btn petit fantome danger', onclick: retirer }, 'Retirer') : null));
}

async function afficherBibliotheque(onglet = 'demandes') {
  const b = await chargerBibliotheque(true);
  fenetre('Bibliothèque de demandes', (fermer) => {
    const zone = h('div');
    const ongletsB = h('div', { class: 'onglets-petits', style: { marginBottom: '14px', flexWrap: 'wrap' } });
    const dessiner = async (o) => {
      onglet = o;
      remplir(ongletsB, [['demandes', 'Demandes'], ['miennes', `Mes demandes (${b.perso.length})`], ['consignes', 'Consignes'], ['coulisses', 'Coulisses']].map(([v, l]) =>
        h('button', { class: v === onglet ? 'actif' : '', onclick: () => dessiner(v) }, l)));
      if (o === 'demandes') {
        remplir(zone,
          h('p', { class: 'petit muet', style: { marginTop: 0 } }, 'Des formulations qui marchent bien avec l’assistant. Les [crochets] sont à remplacer par vos mots : « Compléter » place la demande dans la saisie, le crochet sélectionné.'),
          b.integree.categories.map((c) => h('div', { style: { marginBottom: '14px' } },
            h('h3', { style: { margin: '0 0 8px' } }, c.nom),
            c.demandes.map((d) => carteDemande(d, fermer)))));
      } else if (o === 'miennes') {
        const titre = h('input', { class: 'champ', placeholder: 'Titre (facultatif)', maxlength: '80' });
        const texte = h('textarea', { class: 'champ', rows: '2', placeholder: 'Votre demande, telle que vous l’écririez à l’assistant', style: { marginTop: '6px' } });
        remplir(zone,
          h('p', { class: 'petit muet', style: { marginTop: 0 } }, 'Vos demandes enregistrées, partagées avec toutes les personnes qui utilisent EDIVID sur cet ordinateur. L’étoile à côté de la saisie enregistre la demande en cours.'),
          b.perso.length ? b.perso.map((d) => carteDemande(d, fermer, async () => {
            await api('DELETE', `/api/bibliotheque/${d.id}`);
            b.perso = b.perso.filter((x) => x.id !== d.id);
            dessiner('miennes');
          })) : h('p', { class: 'petit muet' }, 'Aucune demande enregistrée pour l’instant.'),
          h('div', { class: 'carte carte-corps', style: { marginTop: '10px', boxShadow: 'none' } },
            h('h3', { style: { margin: '0 0 8px' } }, 'Ajouter une demande'), titre, texte,
            h('button', { class: 'btn primaire petit', style: { marginTop: '8px' }, onclick: async () => {
              try {
                const d = await api('POST', '/api/bibliotheque', { titre: titre.value, texte: texte.value });
                b.perso.push(d);
                dessiner('miennes');
              } catch (e) { toast(e.message, 'erreur'); }
            } }, icone('plus'), 'Ajouter')));
      } else if (o === 'consignes') {
        remplir(zone,
          h('p', { class: 'petit muet', style: { marginTop: 0 } }, 'Des consignes durables, à placer dans l’onglet Réglages (« Vos consignes au monteur ») : le LLM les suit à chaque montage du projet.'),
          (b.integree.consignes || []).map((c) => h('div', { class: 'incrustation' },
            h('div', {}, h('b', { class: 'petit' }, c.titre), h('div', { class: 'petit', style: { marginTop: '3px' } }, c.texte)),
            E.vue ? h('button', { class: 'btn petit doux', onclick: async () => {
              const actuelles = (E.vue.projet.consignes || '').trim();
              const texte = (actuelles ? `${actuelles}\n` : '') + c.texte;
              await api('POST', `/api/projets/${E.projetId}/consignes`, { texte });
              E.vue.projet.consignes = texte;
              toast('Consigne ajoutée au projet.', 'ok');
            } }, 'Ajouter au projet') : null)));
      } else {
        remplir(zone, h('p', { class: 'muet petit' }, 'Chargement…'));
        const c = E.projetId ? await api('GET', `/api/projets/${E.projetId}/coulisses`) : null;
        remplir(zone,
          h('p', { class: 'petit muet', style: { marginTop: 0 } }, 'Les consignes exactes que reçoit le LLM, à lire pour comprendre ses choix ou vous en inspirer. Celles du monteur changent avec vos réglages et vos consignes.'),
          c ? [['Le monteur (premier montage)', c.monteur], ['Le chef monteur (relecture)', c.critique], ['L’assistant (conversation)', c.assistant]].map(([t, texte]) =>
            h('details', { style: { marginBottom: '10px' } }, h('summary', { style: { cursor: 'pointer', fontWeight: 600 } }, t),
              h('pre', { style: { whiteSpace: 'pre-wrap', fontSize: '12.5px', background: 'var(--surface-2)', padding: '12px', borderRadius: '10px', marginTop: '8px' } }, texte))) : null);
      }
    };
    dessiner(onglet);
    return h('div', {}, ongletsB, zone);
  }, { large: true });
}

function afficherCoulisses() { afficherBibliotheque('coulisses'); }

function enregistrerDemande() {
  const texte = E.noeuds.saisie.value.trim();
  if (!texte) { toast('Écrivez d’abord une demande dans la saisie, puis enregistrez-la.'); return; }
  fenetre('Enregistrer la demande', (fermer) => {
    const titre = h('input', { class: 'champ', placeholder: 'Un titre court', maxlength: '80' });
    setTimeout(() => titre.focus(), 30);
    return h('form', { onsubmit: async (e) => {
      e.preventDefault();
      try { await api('POST', '/api/bibliotheque', { titre: titre.value, texte }); fermer(); toast('Demande ajoutée à votre bibliothèque.', 'ok'); }
      catch (err) { toast(err.message, 'erreur'); }
    } },
    h('p', { class: 'petit muet', style: { marginTop: 0 } }, `« ${texte} »`), titre,
    h('div', { style: { display: 'flex', justifyContent: 'flex-end', gap: '8px', marginTop: '14px' } },
      h('button', { class: 'btn', type: 'button', onclick: fermer }, 'Annuler'),
      h('button', { class: 'btn primaire', type: 'submit' }, 'Enregistrer')));
  });
}

// ------------------------------------------------------------------ paramètres

function champ(libelle, el, aide) {
  return h('div', { class: 'ligne-champ' }, h('label', {}, libelle), el, aide ? h('div', { class: 'reglage-aide' }, aide) : null);
}
function interrupteurSimple(valeur, rappel) {
  return h('label', { class: 'interrupteur' }, h('input', { type: 'checkbox', checked: !!valeur, onchange: (e) => rappel(e.target.checked) }), h('span'));
}
function ligneInterrupteur(titre, aide, valeur, rappel) {
  return h('div', { class: 'reglage' }, h('div', { class: 'reglage-tete' }, h('span', { class: 'reglage-titre' }, titre), interrupteurSimple(valeur, rappel)),
    aide ? h('div', { class: 'reglage-aide' }, aide) : null);
}
function blocCommande(texte) {
  return h('div', { style: { display: 'flex', gap: '6px', alignItems: 'center', marginTop: '6px' } },
    h('code', { style: { flex: 1, background: 'var(--surface-2)', padding: '8px 10px', borderRadius: '8px', fontSize: '13px', overflowX: 'auto', whiteSpace: 'nowrap' } }, texte),
    h('button', { class: 'btn icone fantome', title: 'Copier', onclick: async () => { try { await navigator.clipboard.writeText(texte); toast('Commande copiée.'); } catch { toast('Copie impossible : sélectionnez le texte.'); } } }, icone('copier')));
}

async function afficherParametres() {
  const P = await api('GET', '/api/parametres');
  const projets = await api('GET', '/api/projets');
  const c = P.courriel;
  const d = P.drive;
  const enregistrer = async (message = 'Paramètres enregistrés.') => {
    try {
      const v = await api('POST', '/api/parametres', { url_publique: P.url_publique, courriel: c, drive: d });
      c.mot_de_passe = '';
      c.mot_de_passe_defini = v.courriel.mot_de_passe_defini;
      toast(message, 'ok');
      return true;
    } catch (e) { toast(e.message, 'erreur'); return false; }
  };
  const entree = (obj, cle, attrs = {}) => {
    const el = h('input', { class: 'champ', value: obj[cle] ?? '', ...attrs });
    el.addEventListener('input', () => { obj[cle] = attrs.type === 'number' ? Number(el.value) : el.value; });
    return el;
  };
  const destinataires = h('input', { class: 'champ', value: (c.destinataires || []).join(', '), placeholder: 'vous@exemple.fr, collegue@exemple.fr' });
  destinataires.addEventListener('input', () => { c.destinataires = destinataires.value.split(/[,;\s]+/).filter(Boolean); });
  const securite = h('div', { class: 'segments' });
  const dessinerSecurite = () => remplir(securite, [['ssl', 'SSL (465)'], ['starttls', 'STARTTLS (587)'], ['aucune', 'Aucune']].map(([v, l]) =>
    h('button', { class: c.securite === v ? 'actif' : '', onclick: () => { c.securite = v; c.port = v === 'ssl' ? 465 : v === 'starttls' ? 587 : c.port; port.value = c.port; dessinerSecurite(); } }, l)));
  const port = entree(c, 'port', { type: 'number', min: '1', max: '65535' });
  dessinerSecurite();
  const etatDrive = P.etat_drive?.etat || {};
  const drivesRelies = (P.comptes || []).filter((x) => x.distant);
  const veilles = drivesRelies.filter((x) => x.veille).length + (d.actif ? 1 : 0);
  const intervalle = h('select', { class: 'champ' }, [[60, '1 minute'], [120, '2 minutes'], [300, '5 minutes'], [600, '10 minutes']].map(([v, l]) =>
    h('option', { value: v, selected: d.intervalle_s === v }, l)));
  intervalle.addEventListener('change', () => { d.intervalle_s = Number(intervalle.value); });
  const modele = h('select', { class: 'champ' }, h('option', { value: '' }, 'Réglages par défaut'),
    projets.map((p) => h('option', { value: p.id, selected: d.modele === p.id }, p.nom)));
  modele.addEventListener('change', () => { d.modele = modele.value; });

  remplir($app, h('div', { class: 'accueil', style: { maxWidth: '820px' } },
    h('div', { style: { display: 'flex', alignItems: 'center', gap: '12px' } },
      h('a', { class: 'btn icone fantome', href: '#/', title: 'Retour aux projets' }, icone('retour')), logo(), h('span', { class: 'espace' }), boutonTheme()),
    h('h1', { style: { margin: '24px 0 4px' } }, 'Paramètres'),
    h('p', { class: 'muet', style: { marginTop: 0 } }, 'Communs à tous les projets et à toutes les personnes qui utilisent EDIVID sur cet ordinateur.'),
    sectionGoogle(P),

    h('div', { class: 'carte carte-corps', style: { marginTop: '16px' } },
      h('h2', { style: { display: 'flex', gap: '8px', alignItems: 'center' } }, icone('courriel'), 'Prévenir par courriel'),
      h('p', { class: 'petit muet' }, 'Un message quand un montage est prêt, quand la vidéo finale est prête, ou si quelque chose échoue.'),
      ligneInterrupteur('Envoyer des courriels', null, c.actif, (v) => { c.actif = v; }),
      champ('Destinataires', destinataires, 'Une ou plusieurs adresses, séparées par des virgules.'),
      ligneInterrupteur('Montage prêt', 'Après le premier aperçu d’un montage par le LLM.', c.montage, (v) => { c.montage = v; }),
      ligneInterrupteur('Vidéo finale prête', 'Après le rendu final (et son dépôt sur le Drive, s’il est prévu).', c.rendu, (v) => { c.rendu = v; }),
      ligneInterrupteur('Problème', 'Si une analyse, un montage ou un rendu échoue.', c.erreur, (v) => { c.erreur = v; }),
      h('details', { style: { marginTop: '12px' }, open: !c.serveur },
        h('summary', { style: { cursor: 'pointer', fontWeight: 600 } }, 'Serveur d’envoi'),
        h('div', { class: 'aide', style: { marginTop: '10px' } }, icone('info'),
          h('span', {}, 'Les réglages SMTP de votre boîte mail. Par exemple chez Infomaniak : serveur mail.infomaniak.com, SSL (465), identifiant = l’adresse complète. Chez Gmail : smtp.gmail.com, SSL (465), avec un « mot de passe d’application ».')),
        champ('Serveur', entree(c, 'serveur', { placeholder: 'mail.infomaniak.com' })),
        champ('Sécurité', securite),
        champ('Port', port),
        champ('Identifiant', entree(c, 'identifiant', { autocomplete: 'off' })),
        champ('Mot de passe', entree(c, 'mot_de_passe', { type: 'password', autocomplete: 'new-password', placeholder: c.mot_de_passe_defini ? '•••••••• (enregistré — laissez vide pour le garder)' : '' })),
        champ('Expéditeur', entree(c, 'expediteur'), 'Tel qu’il apparaît dans la boîte de réception, par exemple « EDIVID <adresse@exemple.fr> ».')),
      h('div', { style: { display: 'flex', gap: '8px', marginTop: '16px', flexWrap: 'wrap' } },
        h('button', { class: 'btn primaire', onclick: () => enregistrer() }, 'Enregistrer'),
        h('button', { class: 'btn', onclick: async () => {
          if (!(await enregistrer('Enregistré. Envoi de l’essai…'))) return;
          try { const r = await api('POST', '/api/parametres/courriel/essai'); toast(r.message, 'ok'); } catch (e) { toast(e.message, 'erreur'); }
        } }, icone('courriel'), 'Envoyer un essai'))),

    h('div', { class: 'carte carte-corps', style: { marginTop: '16px' } },
      h('h2', { style: { display: 'flex', gap: '8px', alignItems: 'center' } }, icone('nuage'), 'Google Drive : monter depuis l’iPhone'),
      h('p', { class: 'petit muet' }, 'Déposez une vidéo dans un dossier du Drive depuis l’iPhone : EDIVID la récupère, la monte, la rend, dépose la vidéo finale dans un autre dossier et vous prévient.'),
      h('div', { class: 'guide-liste' },
        [['Relier son compte Google', h('span', { class: 'petit muet' }, 'Dans la carte « Connexion avec Google » ci-dessus, une fois par compte : EDIVID crée « ' + d.entree + ' » et « ' + d.sortie + ' » sur son Drive et le surveille (interrupteur « Veille » à côté du compte).')],
        ['Envoyer depuis l’iPhone', h('span', { class: 'petit muet' }, 'App Google Drive → dossier « ' + d.entree + ' » → « + » → Importer → choisissez la vidéo. Un sous-dossier avec plusieurs vidéos donne un seul projet aux rushs multiples. Vérifiez sur le premier envoi que la définition est bien celle de l’iPhone (EDIVID l’affiche dans la carte du rush).')],
        ['Récupérer la vidéo finale', h('span', { class: 'petit muet' }, 'Elle arrive dans « ' + d.sortie + ' », et un courriel vous prévient si vous l’avez activé.')],
        ].map(([t, x], i) => h('div', { class: 'guide-item' }, h('div', { class: 'num' }, i + 1), h('div', {}, h('h3', {}, t), x)))),
      h('p', { class: 'petit muet', style: { margin: '16px 0 0' } }, 'Réglages communs à tous les Drive reliés :'),
      champ('Dossier d’entrée', entree(d, 'entree')),
      champ('Dossier de sortie', entree(d, 'sortie')),
      champ('Fréquence', intervalle),
      ligneInterrupteur('Lancer le rendu final tout seul', 'Sinon, EDIVID s’arrête à l’aperçu et vous prévient : vous ajustez, puis lancez le rendu.', d.rendu_final, (v) => { d.rendu_final = v; }),
      ligneInterrupteur('Déposer la vidéo finale sur le Drive', null, d.deposer, (v) => { d.deposer = v; }),
      champ('Réglages des projets venus du Drive', modele, 'Choisissez un projet dont vous aimez le montage : ses réglages et ses consignes seront repris.'),
      h('div', { class: 'aide', style: { marginTop: '12px', background: etatDrive.erreur ? 'var(--erreur-doux)' : 'var(--accent-doux)' } }, icone(etatDrive.erreur ? 'alerte' : 'info'),
        !P.etat_drive?.rclone ? h('div', {}, h('span', {}, 'rclone n’est pas installé sur azoth. Dans un terminal sur azoth :'), blocCommande('curl https://rclone.org/install.sh | sudo bash'))
          : h('span', {}, !drivesRelies.length && !d.actif ? 'Aucun Drive relié : reliez un compte Google dans la carte ci-dessus.'
            : etatDrive.en_cours ? etatDrive.en_cours
              : etatDrive.erreur ? `${etatDrive.message} : ${etatDrive.erreur}`
                : etatDrive.dernier_passage ? `Dernier passage : ${new Date(etatDrive.dernier_passage).toLocaleTimeString('fr-FR')} — ${etatDrive.message}`
                  : veilles ? 'Premier passage en attente.' : 'Veille désactivée (interrupteur « Veille » à côté du compte).')),
      h('div', { style: { display: 'flex', gap: '8px', marginTop: '16px', flexWrap: 'wrap' } },
        h('button', { class: 'btn primaire', onclick: () => enregistrer() }, 'Enregistrer'),
        h('button', { class: 'btn', onclick: async () => {
          if (!(await enregistrer('Enregistré. Vérification…'))) return;
          try { const r = await api('POST', '/api/parametres/drive/verifier'); toast(r.message, 'ok'); } catch (e) { toast(e.message, 'erreur'); }
        } }, 'Vérifier la liaison'),
        h('button', { class: 'btn', onclick: async () => {
          try { await api('POST', '/api/parametres/drive/maintenant'); toast('Passage lancé.'); setTimeout(afficherParametres, 4000); } catch (e) { toast(e.message, 'erreur'); }
        } }, 'Regarder maintenant'))),

    h('div', { class: 'carte carte-corps', style: { marginTop: '16px' } },
      h('h2', {}, 'Lien des courriels'),
      champ('Adresse de l’interface', entree(P, 'url_publique'), 'Utilisée dans les liens des courriels : l’adresse par laquelle on ouvre EDIVID sur le tailnet.'),
      h('button', { class: 'btn primaire', style: { marginTop: '12px' }, onclick: () => enregistrer() }, 'Enregistrer')),

    h('div', { class: 'carte carte-corps', style: { marginTop: '16px' } },
      h('h2', {}, 'Encodage matériel'),
      P.nvenc
        ? h('p', { class: 'petit' }, h('span', { class: 'pastille ok' }, 'Disponible'), ' L’encodeur de la carte NVIDIA accélère aperçus et rendus quand la carte a de la place ; sinon EDIVID encode avec le processeur.')
        : h('div', {}, h('p', { class: 'petit muet' }, 'Absent : EDIVID encode avec le processeur (plus lent, même qualité). Pour l’activer, installez le paquet correspondant au pilote, puis redémarrez EDIVID :'),
          blocCommande('sudo apt install libnvidia-encode-595-server libnvidia-decode-595-server'),
          blocCommande('systemctl --user restart edivid')))));
}

// ------------------------------------------------------------------ connexion avec Google (Paramètres)

function sectionGoogle(P) {
  const g = P.google;
  const https = (P.url_publique || '').startsWith('https://');
  const retour = `${(P.url_publique || '').replace(/\/$/, '')}/api/google/retour`;
  const entreeG = (cle, attrs = {}) => {
    const el = h('input', { class: 'champ', value: g[cle] ?? '', ...attrs });
    el.addEventListener('input', () => { g[cle] = el.value; });
    return el;
  };
  const comptes = P.comptes || [];
  return h('div', { class: 'carte carte-corps', style: { marginTop: '16px' } },
    h('h2', { style: { display: 'flex', gap: '8px', alignItems: 'center' } }, icone('google'), 'Connexion avec Google'),
    h('p', { class: 'petit muet' }, 'Les appareils du tailnet entrent sans code ni compte. Relier un compte Google sert à son Drive (les vidéos déposées depuis son iPhone sont montées et lui reviennent) et à la publication sur YouTube.'),
    g.configure
      ? h('div', {},
          comptes.length ? comptes.map((c) => h('div', { class: 'incrustation' },
            h('div', { style: { minWidth: 0 } },
              h('b', { class: 'petit' }, c.nom || c.email), h('div', { class: 'tres-petit muet' }, c.email),
              h('div', { class: 'tres-petit', style: { marginTop: '4px' } }, c.distant ? h('span', { class: 'pastille ok' }, 'Drive relié') : h('span', { class: 'pastille alerte' }, 'Drive non relié'))),
            h('div', { style: { display: 'flex', alignItems: 'center', gap: '10px' } },
              c.distant ? h('label', { class: 'bascule', title: 'Surveiller le dossier d’entrée de ce Drive' },
                h('input', { type: 'checkbox', checked: c.veille, onchange: async (e) => {
                  try { await api('POST', '/api/comptes', { email: c.email, veille: e.target.checked }); toast(e.target.checked ? 'Veille de ce Drive activée.' : 'Veille de ce Drive arrêtée.', 'ok'); }
                  catch (err) { toast(err.message, 'erreur'); e.target.checked = !e.target.checked; }
                } }), h('span', {}, 'Veille')) : null,
              h('button', { class: 'btn petit fantome danger', onclick: async () => {
                if (!(await confirmer('Retirer ce compte ?', `${c.email} ne pourra plus se connecter et son Drive sera délié.`, 'Retirer'))) return;
                await api('DELETE', `/api/comptes/${encodeURIComponent(c.email)}`);
                afficherParametres();
              } }, 'Retirer'))))
            : h('p', { class: 'petit muet' }, 'Aucun compte autorisé pour l’instant.'),
          h('a', { class: 'btn bouton-google', href: '/api/google/connexion?mode=liaison', style: { marginTop: '10px' } }, icone('google'), 'Autoriser et relier un compte Google'),
          h('p', { class: 'tres-petit muet' }, 'Une seule fois par compte Google, depuis n’importe quel appareil : la personne clique ce bouton et accepte chez Google.'))
      : h('div', { class: 'aide', style: { background: 'var(--alerte-doux)' } }, icone('alerte'), h('span', {}, 'Pas encore configurée : suivez la mise en place ci-dessous (une seule fois).')),
    h('details', { style: { marginTop: '14px' }, open: !g.configure },
      h('summary', { style: { cursor: 'pointer', fontWeight: 600 } }, 'Mise en place (une seule fois)'),
      h('div', { class: 'guide-liste' },
        [['Adresse en HTTPS', https
          ? h('span', { class: 'petit muet' }, `Déjà en place : ${P.url_publique}.`)
          : h('div', {},
            h('span', { class: 'petit muet' }, 'Google n’accepte qu’une adresse en HTTPS. Dans la console Tailscale (DNS), activez « HTTPS Certificates », puis sur azoth :'),
            blocCommande('sudo tailscale serve --bg 8030'),
            h('span', { class: 'petit muet' }, 'Mettez ensuite l’adresse https://… dans « Lien des courriels » plus bas.'),
            h('div', { class: 'tres-petit', style: { color: 'var(--alerte)', marginTop: '4px' } }, `Adresse actuelle : ${P.url_publique} — elle doit commencer par https://`))],
        ['Créer le projet et activer les API', h('span', { class: 'petit muet' },
          'Sur console.cloud.google.com, avec le compte Google qui gérera l’application : nouveau projet « EDIVID ». Puis API et services → Bibliothèque : activez « Google Drive API » et « YouTube Data API v3 ».')],
        ['Écran de consentement', h('span', { class: 'petit muet' },
          'Google Auth Platform (anciennement « Écran de consentement OAuth ») : type Externe, nom EDIVID, votre adresse de contact. Accès aux données : ajoutez les champs d’application …/auth/drive, …/auth/youtube.upload, openid, email et profile. Audience : ajoutez en utilisateurs test chaque compte Google qui se reliera.')],
        ['Créer l’identifiant', h('div', {}, h('span', { class: 'petit muet' }, 'Clients → Créer un client → type « Application Web ». URI de redirection autorisé :'), blocCommande(retour))],
        ['Coller l’identifiant ici', h('div', {},
          champ('ID client', entreeG('client_id', { placeholder: '…apps.googleusercontent.com' })),
          champ('Code secret du client', entreeG('client_secret', { type: 'password', autocomplete: 'new-password', placeholder: g.secret_defini ? '•••••••• (enregistré — laissez vide pour le garder)' : '' })),
          h('button', { class: 'btn primaire', style: { marginTop: '10px' }, onclick: async () => {
            try { await api('POST', '/api/parametres', { google: { client_id: g.client_id, client_secret: g.client_secret } }); toast('Identifiant Google enregistré.', 'ok'); afficherParametres(); }
            catch (e) { toast(e.message, 'erreur'); }
          } }, 'Enregistrer'))],
        ['Relier, puis passer en production', h('span', { class: 'petit muet' },
          'Cliquez « Autoriser et relier un compte Google » (plus haut) pour chaque compte, et acceptez Drive et YouTube. Une fois l’essai réussi, dans Audience, publiez l’application (« En production ») : sinon Google coupe la liaison au bout de 7 jours. Google affichera « application non validée » : il suffit de continuer.')],
        ['Lever le verrou YouTube', h('span', { class: 'petit muet' },
          'Tant que l’application n’a pas passé l’audit de l’API YouTube, Google garde en privé les vidéos envoyées par EDIVID. La demande se fait par le formulaire « YouTube API Services – Audit and Quota Extension ». Rien n’assure qu’une vidéo déjà verrouillée soit libérée ensuite : en attendant, téléchargez l’export YouTube et envoyez-le à la main depuis YouTube Studio.')],
        ].map(([t, x], i) => h('div', { class: 'guide-item' }, h('div', { class: 'num' }, i + 1), h('div', {}, h('h3', {}, t), x))))));
}

// ------------------------------------------------------------------ transcription modifiable

function ligneEdition(ph, corrigee) {
  const zone = h('textarea', { class: 'champ', rows: '2', 'aria-label': 'Texte de la phrase' });
  zone.value = ph.texte;
  const enregistrer = async (texte) => {
    try {
      await api('POST', `/api/projets/${E.projetId}/correction`, { cle: ph.cle, texte });
      E.edition = null;
      E.aMettreAJour = true;
      toast('Texte corrigé : les sous-titres en tiendront compte au prochain aperçu.', 'ok');
    } catch (e) { toast(e.message, 'erreur'); }
  };
  zone.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); enregistrer(zone.value); }
    if (e.key === 'Escape') { E.edition = null; dessinerPhrases(); }
  });
  setTimeout(() => { zone.focus(); zone.setSelectionRange(zone.value.length, zone.value.length); }, 20);
  return h('div', { class: 'phrase edition', 'data-cle': ph.cle },
    h('div', { class: 'temps' }, minutes(ph.t0)),
    h('div', { style: { minWidth: 0 } }, zone,
      h('div', { style: { display: 'flex', gap: '6px', marginTop: '6px', flexWrap: 'wrap' } },
        h('button', { class: 'btn petit primaire', onclick: () => enregistrer(zone.value) }, icone('ok'), 'Enregistrer'),
        h('button', { class: 'btn petit', onclick: () => { E.edition = null; dessinerPhrases(); } }, 'Annuler'),
        corrigee ? h('button', { class: 'btn petit fantome', title: 'Revenir au texte transcrit', onclick: () => enregistrer('') }, 'Texte d’origine') : null,
        h('span', { class: 'tres-petit muet', style: { alignSelf: 'center' } }, 'Entrée pour enregistrer, Échap pour annuler'))),
    h('span'));
}

function zoneRemplacer() {
  if (!E.voirRemplacer) {
    return h('button', { class: 'btn petit fantome', style: { marginBottom: '6px' }, onclick: () => { E.voirRemplacer = true; dessinerPhrases(); } },
      icone('crayon'), 'Rechercher et remplacer dans tout le texte');
  }
  const cherche = h('input', { class: 'champ', placeholder: 'Mot mal transcrit (ex. : mon moyenne)' });
  const remplace = h('input', { class: 'champ', placeholder: 'Remplacer par (ex. : mon moyen)' });
  const lancer = async () => {
    try {
      const r = await api('POST', `/api/projets/${E.projetId}/remplacer`, { cherche: cherche.value, remplace: remplace.value });
      toast(r.phrases ? `${r.phrases} phrase(s) corrigée(s).` : 'Rien trouvé.', r.phrases ? 'ok' : '');
      if (r.phrases) E.aMettreAJour = true;
    } catch (e) { toast(e.message, 'erreur'); }
  };
  return h('div', { class: 'remplacer' }, cherche, remplace,
    h('button', { class: 'btn petit primaire', onclick: lancer }, 'Remplacer partout'),
    h('button', { class: 'btn icone fantome', title: 'Fermer', onclick: () => { E.voirRemplacer = false; dessinerPhrases(); } }, icone('fermer')));
}

// ------------------------------------------------------------------ PostProd

let catalogueLooks = null;
let minuteriePostProd = null;

function dessinerPostProd(corps) {
  const p = E.vue.projet;
  const pp = E.postprod = { ...p.postprod };
  if (!catalogueLooks) {
    remplir(corps, h('p', { class: 'muet petit' }, 'Chargement…'));
    api('GET', '/api/postprod/looks').then((c) => { catalogueLooks = c; dessinerPanneau(); });
    return;
  }
  const enregistrer = () => {
    clearTimeout(minuteriePostProd);
    minuteriePostProd = setTimeout(async () => {
      try {
        await api('POST', `/api/projets/${E.projetId}/postprod`, E.postprod);
        E.aMettreAJour = true;
        dessinerBarre();
        if (E.comparaison) comparer();
      } catch (e) { toast(e.message, 'erreur'); }
    }, 450);
  };
  const curseur = (cle, titre, aide, min, max) => {
    const sortie = h('b', { class: 'petit', style: { minWidth: '38px', textAlign: 'right' } }, `${pp[cle] > 0 && min < 0 ? '+' : ''}${pp[cle]}`);
    return h('div', { class: 'reglage' },
      h('div', { class: 'reglage-tete' }, h('span', { class: 'reglage-titre' }, titre), sortie),
      aide ? h('div', { class: 'reglage-aide' }, aide) : null,
      h('input', { type: 'range', min, max, step: '1', value: pp[cle], 'aria-label': titre,
        oninput: (e) => { pp[cle] = Number(e.target.value); E.postprod.look = ''; sortie.textContent = `${pp[cle] > 0 && min < 0 ? '+' : ''}${pp[cle]}`; enregistrer(); } }));
  };
  const couleur = h('select', { class: 'champ' }, catalogueLooks.couleurs.map((c) => h('option', { value: c.cle, selected: pp.couleur === c.cle }, c.nom)));
  couleur.addEventListener('change', () => { pp.couleur = couleur.value; pp.look = ''; enregistrer(); });
  E.noeuds.comparaison = h('div', { class: 'comparaison' });
  const horizontal = (p.apercu?.rapport?.largeur || 0) > (p.apercu?.rapport?.hauteur || 1);
  corps.append(
    h('div', { class: 'aide', style: { marginBottom: '14px' } }, icone('info'),
      h('span', {}, 'Le traitement de l’image sur toute la vidéo : couleur, teint, lumière, grain. Choisissez un look, ajustez, puis comparez avant/après sur l’image affichée. Les textes et sous-titres restent nets.')),
    h('div', { class: 'section-reglages' }, h('h3', {}, 'Looks'),
      h('div', { class: 'grille-looks' }, catalogueLooks.looks.map((l) => h('button', {
        class: `look ${pp.look === l.cle ? 'choisi' : ''}`, title: l.description,
        onclick: async () => {
          Object.assign(E.postprod, l.reglages, { look: l.cle });
          try {
            await api('POST', `/api/projets/${E.projetId}/postprod`, E.postprod);
            E.vue.projet.postprod = { ...E.postprod };
            E.aMettreAJour = true;
            toast(`Look « ${l.nom} » appliqué. Mettez à jour l’aperçu pour le voir dans la vidéo.`, 'ok');
            dessinerPanneau();
            if (E.comparaison) comparer();
          } catch (e) { toast(e.message, 'erreur'); }
        },
      }, h('b', {}, l.nom), h('span', {}, l.description))))),
    h('div', { class: 'section-reglages' }, h('h3', {}, 'Avant / après'),
      h('div', { class: 'reglage-aide' }, 'Sur l’image d’origine, à l’instant affiché dans le lecteur (mettez en pause là où le visage est bien visible).'),
      h('button', { class: 'btn petit doux', style: { marginTop: '8px' }, onclick: () => { E.comparaison = true; comparer(); } }, icone('image'), 'Comparer à l’instant affiché'),
      E.noeuds.comparaison),
    h('div', { class: 'section-reglages' }, h('h3', {}, 'Couleur'),
      h('div', { class: 'reglage' }, h('div', { class: 'reglage-titre' }, 'Colorimétrie'), h('div', { class: 'reglage-aide' }, 'L’ambiance générale des couleurs.'), couleur),
      curseur('exposition', 'Exposition', 'Plus clair ou plus sombre.', -100, 100),
      curseur('contraste', 'Contraste', null, -100, 100),
      curseur('saturation', 'Saturation', 'Intensité des couleurs.', -100, 100),
      curseur('temperature', 'Température', 'Vers le bleu (froid) ou l’ambre (chaud).', -100, 100)),
    h('div', { class: 'section-reglages' }, h('h3', {}, 'Peau et lumière'),
      curseur('lissage', 'Lissage du teint', 'Adoucit la peau et gomme les petites imperfections, sans flouter les yeux ni les contours.', 0, 100),
      curseur('diffusion', 'Diffusion (glow)', 'Un halo doux dans les lumières, comme un filtre « Pro-Mist » : peau lumineuse, image rêveuse.', 0, 100),
      curseur('nettete', 'Netteté', 'Relève les détails (cheveux, regard). À doser avec le lissage.', 0, 100)),
    h('div', { class: 'section-reglages' }, h('h3', {}, 'Texture et cadre'),
      curseur('grain', 'Grain', 'Une texture de pellicule fine et vivante.', 0, 100),
      curseur('vignette', 'Vignette', 'Assombrit doucement les bords pour guider le regard au centre.', 0, 100),
      h('div', { class: 'reglage' }, h('div', { class: 'reglage-tete' }, h('span', { class: 'reglage-titre' }, 'Bandes cinéma'),
        interrupteurSimple(pp.bandes, (v) => { pp.bandes = v; pp.look = ''; enregistrer(); })),
        h('div', { class: 'reglage-aide' }, horizontal ? 'Bandes noires au format 2,39:1, comme au cinéma.' : 'Pour les vidéos horizontales seulement : sans effet en vertical.'))),
    h('button', { class: 'btn petit fantome', onclick: async () => {
      E.postprod = { look: '', couleur: 'naturel', exposition: 0, contraste: 0, saturation: 0, temperature: 0, lissage: 0, diffusion: 0, nettete: 0, grain: 0, vignette: 0, bandes: false };
      await api('POST', `/api/projets/${E.projetId}/postprod`, E.postprod);
      E.vue.projet.postprod = { ...E.postprod };
      E.aMettreAJour = true;
      dessinerPanneau();
    } }, icone('relancer'), 'Tout remettre à zéro'));
  if (E.comparaison) comparer();
}

function comparer() {
  const n = E.noeuds;
  if (!n.comparaison) return;
  if (!n.video.dataset.url) { remplir(n.comparaison, h('p', { class: 'petit muet' }, 'Ouvrez d’abord l’aperçu ou la vidéo d’origine dans le lecteur.')); return; }
  const base = `/api/projets/${E.projetId}/postprod/image?t=${n.video.currentTime.toFixed(2)}&vue=${E.lecteur}&rush=${E.lecteur === 'origine' ? rushLecteur() : ''}&_=${Date.now()}`;
  const image = (avant) => h('figure', {}, h('img', { src: `${base}&avant=${avant ? "true" : "false"}`, alt: avant ? 'Avant' : 'Après', loading: 'lazy',
    onerror: (e) => { e.target.replaceWith(h('p', { class: 'petit muet' }, 'Image indisponible à cet instant.')); } }),
    h('figcaption', {}, avant ? 'Avant' : 'Après'));
  remplir(n.comparaison, image(true), image(false));
}

// ------------------------------------------------------------------ export et YouTube

const PRESETS_EXPORT = [
  ['youtube', 'YouTube', 'Recommandé. Réglages conseillés par YouTube : débit élevé, image fixe à 30 i/s, son AAC 384 kb/s. La meilleure qualité une fois en ligne.'],
  ['leger', 'Léger', 'Pour l’iPhone, WhatsApp ou un courriel : 720p, fichier environ quatre fois plus petit.'],
  ['max', 'Qualité maximale', 'Pour l’archive ou une retouche ailleurs : fichier lourd, aucune perte visible.'],
];

async function afficherExport() {
  let P = null;
  try { P = await api('GET', '/api/parametres'); } catch { /* sans comptes */ }
  fenetre('Exporter la vidéo', (fermer) => {
    const zone = h('div');
    const dessiner = () => {
      const p = E.vue.projet;
      const exports = [...(p.exports || [])].reverse();
      remplir(zone,
        h('p', { class: 'petit muet', style: { marginTop: 0 } }, `À partir du rendu final (${p.rendu.rapport.largeur}×${p.rendu.rapport.hauteur}, ${minutes(p.rendu.rapport.duree_montage)}). L’export prend une à quelques minutes.`),
        h('div', { class: 'grille-medias', style: { gridTemplateColumns: 'repeat(3, 1fr)' } }, PRESETS_EXPORT.map(([cle, nom, aide]) =>
          h('div', { class: `look ${cle === 'youtube' ? 'choisi' : ''}`, style: { cursor: 'default' } },
            h('b', {}, nom), h('span', {}, aide),
            h('button', { class: `btn petit ${cle === 'youtube' ? 'primaire' : ''}`, style: { marginTop: '8px' }, onclick: async () => {
              try { await api('POST', `/api/projets/${E.projetId}/export`, { preset: cle }); toast(`Export ${nom} lancé.`); fermer(); }
              catch (e) { toast(e.message, 'erreur'); }
            } }, 'Exporter')))),
        h('h3', { style: { margin: '18px 0 8px' } }, 'Vos exports'),
        exports.length ? exports.map((x) => h('div', { class: 'incrustation' },
          h('div', { style: { minWidth: 0 } },
            h('b', { class: 'petit' }, (PRESETS_EXPORT.find(([k]) => k === x.preset) || [, x.preset])[1]),
            h('div', { class: 'tres-petit muet' }, `${x.largeur}×${x.hauteur} · ${taille(x.taille)} · ${new Date(x.cree).toLocaleString('fr-FR')}`),
            x.youtube ? h('a', { class: 'tres-petit', href: x.youtube, target: '_blank', rel: 'noopener' }, `Sur YouTube : ${x.youtube}`) : null),
          h('div', { style: { display: 'flex', gap: '4px', flexWrap: 'wrap', justifyContent: 'flex-end' } },
            h('a', { class: 'btn petit doux', href: `/api/projets/${E.projetId}/sorties/${x.fichier}`, download: `${p.nom} (${x.preset}).mp4` }, icone('telecharger'), 'Télécharger'),
            h('button', { class: 'btn petit', onclick: () => { fermer(); dialogueYoutube(x, P); } }, 'Publier sur YouTube'),
            h('button', { class: 'btn icone fantome danger', title: 'Supprimer cet export', onclick: async () => {
              await api('DELETE', `/api/projets/${E.projetId}/exports/${x.id}`);
              await rafraichir();
              dessiner();
            } }, icone('corbeille')))))
          : h('p', { class: 'petit muet' }, 'Aucun export pour l’instant.'));
    };
    dessiner();
    return zone;
  }, { large: true });
}

function dialogueYoutube(x, P) {
  const comptes = (P?.comptes || []).filter((c) => c.youtube);
  const p = E.vue.projet;
  const f = { titre: p.nom, description: '', mots_cles: '', visibilite: 'private', categorie: '26', compte: comptes[0]?.email || '' };
  fenetre('Publier sur YouTube', (fermer) => {
    const zone = h('div');
    const champTexte = (cle, attrs = {}, multi = false) => {
      const el = h(multi ? 'textarea' : 'input', { class: 'champ', ...attrs });
      el.value = f[cle];
      el.addEventListener('input', () => { f[cle] = el.value; });
      return el;
    };
    const dessiner = () => {
      if (!comptes.length) {
        remplir(zone, h('div', { class: 'aide', style: { background: 'var(--alerte-doux)' } }, icone('alerte'),
          h('span', {}, 'Aucun compte Google n’a autorisé YouTube. Dans Paramètres, connectez votre compte avec Google (il demande aussi l’accès à YouTube), puis revenez ici.')),
          h('a', { class: 'btn primaire', href: '#/parametres', style: { marginTop: '12px' }, onclick: fermer }, 'Ouvrir les Paramètres'));
        return;
      }
      const visibilite = h('div', { class: 'segments' });
      const dessinerVis = () => remplir(visibilite, [['private', 'Privée'], ['unlisted', 'Non répertoriée'], ['public', 'Publique']].map(([v, l]) =>
        h('button', { class: f.visibilite === v ? 'actif' : '', onclick: () => { f.visibilite = v; dessinerVis(); } }, l)));
      dessinerVis();
      const categorie = h('select', { class: 'champ' }, [['26', 'Vie pratique et style'], ['22', 'Personnes et blogs'], ['27', 'Éducation'], ['17', 'Sport'], ['24', 'Divertissement']].map(([v, l]) =>
        h('option', { value: v, selected: f.categorie === v }, l)));
      categorie.addEventListener('change', () => { f.categorie = categorie.value; });
      const compte = h('select', { class: 'champ' }, comptes.map((c) => h('option', { value: c.email, selected: f.compte === c.email }, `${c.nom} — ${c.email}`)));
      compte.addEventListener('change', () => { f.compte = compte.value; });
      const proposer = h('button', { class: 'btn petit doux', onclick: async () => {
        proposer.disabled = true;
        proposer.lastChild.textContent = 'Le LLM rédige…';
        try {
          const v = await api('POST', `/api/projets/${E.projetId}/youtube/proposer`);
          f.titre = v.titre || f.titre;
          f.description = v.description || f.description;
          f.mots_cles = (v.mots_cles || []).join(', ');
          dessiner();
        } catch (e) { toast(e.message, 'erreur'); proposer.disabled = false; }
      } }, icone('etincelles'), h('span', {}, 'Proposer titre, description et mots-clés'));
      remplir(zone,
        proposer,
        champ('Chaîne (compte Google)', compte),
        champ('Titre', champTexte('titre', { maxlength: '100' })),
        champ('Description', champTexte('description', { rows: '6' }, true), 'Les chapitres horodatés, s’il y en a, rendent la vidéo plus facile à parcourir.'),
        champ('Mots-clés', champTexte('mots_cles', { placeholder: 'séparés par des virgules' })),
        champ('Visibilité', visibilite, 'Commencez en « Privée » pour vérifier, puis passez-la en publique depuis YouTube Studio (une fois l’audit passé, voir ci-dessous).'),
        champ('Catégorie', categorie),
        h('div', { class: 'aide', style: { marginTop: '12px' } }, icone('info'),
          h('span', {}, 'Tant que votre application Google n’a pas passé l’audit de l’API YouTube, Google garde en privé les vidéos envoyées par EDIVID. La demande d’audit se fait dans la console Google Cloud.')),
        h('div', { style: { display: 'flex', justifyContent: 'flex-end', gap: '8px', marginTop: '16px' } },
          h('button', { class: 'btn', onclick: fermer }, 'Annuler'),
          h('button', { class: 'btn primaire', onclick: async () => {
            try {
              await api('POST', `/api/projets/${E.projetId}/youtube`, { export: x.id, compte: f.compte, titre: f.titre, description: f.description,
                mots_cles: f.mots_cles.split(',').map((m) => m.trim()).filter(Boolean), visibilite: f.visibilite, categorie: f.categorie });
              toast('Envoi sur YouTube lancé : la progression s’affiche en bas.', 'ok');
              fermer();
            } catch (e) { toast(e.message, 'erreur'); }
          } }, icone('televerser'), 'Publier')));
    };
    dessiner();
    return zone;
  }, { large: true });
}
