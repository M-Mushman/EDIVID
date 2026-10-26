// EDIVID — interface. JavaScript sans dépendance : tout passe par l'API du serveur local.

import { creerApercuDirect } from './postprod-direct.js';
import { creerSonDirect } from './son-direct.js';
import { creerHabillageDirect } from './habillage-direct.js';
import { creerTextesDirect } from './textes-direct.js';

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
  pause: '<path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" fill="currentColor" stroke="none"/>',
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
  zoom: '<circle cx="11" cy="11" r="6"/><path d="M20 20l-4.5-4.5M11 8v6M8 11h6"/>',
  texte: '<path d="M5 6V4h14v2M12 4v16M9 20h6"/>',
  couches: '<path d="M12 3l9 5-9 5-9-5z"/><path d="M3 13l9 5 9-5"/>',
  onde: '<path d="M3 12h1M7 8v8M11 5v14M15 8v8M19 10v4"/>',
  fondu: '<rect x="3" y="5" width="11" height="11" rx="2"/><rect x="10" y="9" width="11" height="11" rx="2"/>',
  signet: '<path d="M6 3h12v18l-6-4-6 4z"/>',
  telephone: '<rect x="7" y="2" width="10" height="20" rx="2.5"/><path d="M11 18h2"/>',
  youtube: '<rect x="2.5" y="5.5" width="19" height="13" rx="4"/><path d="M10 9.5v5l4.5-2.5z" fill="currentColor" stroke="none"/>',
  peau: '<circle cx="12" cy="12" r="8"/><path d="M8.5 14.5c1.8 1.4 5.2 1.4 7 0M9 10h.01M15 10h.01"/>',
  palette: '<path d="M12 3a9 9 0 1 0 0 18c1.1 0 1.5-.8 1.5-1.5 0-1.2-1-1.5-1-2.5s.7-1.5 1.8-1.5H17a4 4 0 0 0 4-4C21 6.5 17 3 12 3z"/><path d="M7.5 11h.01M10 7h.01M15 7h.01"/>',
  dossier: '<path d="M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2.5h8.5A1.5 1.5 0 0 1 21 9v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5z"/>',
  grain: '<path d="M5 5h.01M10 7h.01M15 5h.01M19 9h.01M7 11h.01M12 12h.01M17 14h.01M5 17h.01M10 18h.01M15 19h.01M19 18h.01"/>',
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

/** Thème du contrat de design KARAYAN : data-theme toujours posé, choix gardé sous « karayan_theme ». */
function appliquerTheme() {
  const t = lire('karayan_theme');
  document.documentElement.dataset.theme = t === 'light' || t === 'dark' ? t
    : (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
}
function boutonTheme() {
  const sombre = document.documentElement.dataset.theme === 'dark';
  return h('button', {
    class: 'btn icone fantome', title: sombre ? 'Passer en thème clair' : 'Passer en thème sombre',
    onclick: () => { stocker('karayan_theme', sombre ? 'light' : 'dark'); appliquerTheme(); route(); },
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

// ------------------------------------------------------------------ blocs d'outils (accordéon)

/** Bloc d'outils repliable du panneau latéral : un fond teinté par famille d'outils, une ligne de résumé
 *  lisible même replié ; replié par défaut, l'état est gardé par navigateur. */
function blocOutil(cle, { icone: ic, titre, resume, famille }, ...contenu) {
  const ouvert = lire(`edivid_outil_${cle}`) === '1';
  const id = `outil-${cle.replace(/[^a-z0-9_-]/gi, '_')}`;
  const corps = h('div', { class: 'outil-corps', id, hidden: !ouvert }, ...contenu);
  const bloc = h('section', { class: `outil famille-${famille}${ouvert ? ' ouvert' : ''}` });
  const tete = h('button', {
    class: 'outil-tete', type: 'button', 'aria-expanded': ouvert ? 'true' : 'false', 'aria-controls': id,
    onclick: () => {
      const o = !bloc.classList.contains('ouvert');
      bloc.classList.toggle('ouvert', o);
      corps.hidden = !o;
      tete.setAttribute('aria-expanded', o ? 'true' : 'false');
      stocker(`edivid_outil_${cle}`, o ? '1' : '0');
    },
  },
  h('span', { class: 'outil-icone' }, icone(ic)),
  h('span', { class: 'outil-titres' }, h('b', {}, titre), resume ? h('span', { class: 'outil-resume' }, resume) : null),
  h('span', { class: 'outil-chevron' }, icone('chevron')));
  bloc.append(tete, corps);
  return bloc;
}

/** Les réglages d'une section, en une ligne : choix, curseurs, interrupteurs actifs. */
function resumeSection(section, reglages) {
  const rs = E.catalogue.filter((r) => r.section === section);
  const parts = [];
  for (const r of rs) {
    const v = reglages[r.cle];
    if (r.genre === 'choix') parts.push(r.options.find((o) => o.valeur === v)?.libelle);
    else if (r.genre === 'curseur') parts.push(`${r.titre.split(' ')[0].toLowerCase()} ${v} ${r.bornes.unite}`);
    else if (r.genre === 'media_son') parts.push(v ? (E.vue.projet.medias.find((m) => m.id === v)?.nom || 'musique') : 'sans musique');
  }
  const inter = rs.filter((r) => r.genre === 'interrupteur');
  if (inter.length > 1) parts.push(`${inter.filter((r) => reglages[r.cle]).length}/${inter.length} activés`);
  else if (inter.length === 1) parts.push(reglages[inter[0].cle] ? 'activé' : 'désactivé');
  return parts.filter(Boolean).join(' · ');
}

// Famille (teinte du bloc) et icône de chaque section de réglages.
const SECTIONS_REGLAGES = {
  Autocut: ['montage', 'ciseaux'],
  Structure: ['montage', 'couches'],
  Zooms: ['cadre', 'zoom'],
  Transitions: ['cadre', 'fondu'],
  'Sous-titres': ['texte', 'texte'],
  Image: ['image', 'image'],
  Son: ['son', 'onde'],
};


// ------------------------------------------------------------------ état

const E = {
  catalogue: null,
  projetId: null,
  vue: null,
  etat: null,
  onglet: 'reglages',
  lecteur: 'apercu',
  rushLecteur: null,
  programme: null,
  proposition: null,
  commentaire: null,
  correction: false,
  voirTlOrigine: lire('edivid_voirTlOrigine') !== '0',
  voirTlMontage: lire('edivid_voirTlMontage') !== '0',
  filtre: 'toutes',
  ppDirect: lire('edivid_ppDirect') !== '0',
  sonDirect: lire('edivid_sonDirect') !== '0',
  editionSt: null,
  selectionSt: new Set(),
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
    partiel: `Connecté, mais Google n’a pas accordé l’accès à ${(q.get('manque') || '').split(',').map((m) => ({ drive: 'Google Drive', youtube: 'YouTube' }[m] || m)).join(' ni à ')}. `
      + 'Reconnectez-vous et, sur l’écran de Google, cochez les cases correspondantes (elles peuvent être décochées par défaut).',
  };
  setTimeout(() => toast(messages[g] || 'Connexion avec Google : réponse inattendue.', g === 'annule' ? '' : 'erreur'), 300);
  if (g === 'partiel') setTimeout(() => toast(messages.partiel, 'erreur'), 6000);
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
    // Un jeu de réglages enregistré peut servir de point de départ.
    const jeu = h('select', { class: 'champ', 'aria-label': 'Réglages de départ' }, h('option', { value: '' }, 'Réglages par défaut'));
    const ligneJeu = h('div', { hidden: true, style: { marginTop: '14px' } },
      h('label', { class: 'petit', style: { fontWeight: 600, display: 'block', marginBottom: '6px' } }, 'Partir d’un jeu de réglages'), jeu);
    api('GET', '/api/jeux').then(({ jeux }) => {
      if (!jeux.length) return;
      jeu.append(...jeux.map((j) => h('option', { value: j.id }, j.nom)));
      ligneJeu.hidden = false;
    }).catch(() => {});
    setTimeout(() => champ.focus(), 30);
    return h('form', {
      onsubmit: async (e) => {
        e.preventDefault();
        const { id } = await api('POST', '/api/projets', { nom: champ.value });
        if (jeu.value) {
          try { await api('POST', `/api/projets/${id}/jeux/${jeu.value}`); } catch (err) { toast(err.message, 'erreur'); }
        }
        fermer();
        location.hash = `#/projet/${id}`;
      },
    },
    h('label', { class: 'petit', style: { fontWeight: 600, display: 'block', marginBottom: '6px' } }, 'Nom du projet'),
    champ, ligneJeu,
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
    // Le serveur sait si l'aperçu reflète l'état du projet : l'avertissement survit à un rechargement de la page.
    if (vue.apercu_a_jour === false) E.aMettreAJour = true;
    if (vue.apercu_a_jour === true) E.aMettreAJour = false;
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
  n.panneau = h('aside', { class: 'carte panneau-droit', 'aria-label': 'Réglages, médias, textes et post-production' });
  n.barre = h('div', { class: 'barre-actions-int' });
  n.dockDiscussion = h('section', { class: 'dock-discussion', 'aria-label': 'Discussion avec EDIVID' });
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
    n.main = h('main', { class: 'mise-en-page' },
      h('div', { class: 'colonne' }, n.rushs,
        // Panneaux 2 et 3 côte à côte : on lit la vidéo en suivant son texte.
        h('div', { class: 'atelier' }, n.lecteur, n.phrases)),
      n.panneau),
    n.dock = h('div', { class: 'dock' }, n.dockDiscussion, h('div', { class: 'barre-actions' }, n.barre)));
  // La page réserve en bas la hauteur du bloc fixe (discussion et actions), qui change quand on le replie.
  new ResizeObserver(() => document.documentElement.style.setProperty('--dock', `${n.dock.offsetHeight}px`)).observe(n.dock);
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
  dessinerDiscussion();
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
  telechargement: ['Téléchargement', 'pastille alerte'],
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
      h('button', { class: 'btn petit doux', style: { marginTop: '8px' }, onclick: () => choisirDepuisDrive('rushs') }, icone('google'), 'Choisir sur Google Drive'),
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
  // Le plein écran du lecteur met tout son cadre en plein écran (vidéo et calques du direct : PostProd, habillage,
  // sous-titres, textes) ; celui de la vidéo seule les perdrait, d'où son bouton masqué.
  n.video = h('video', { controls: true, playsinline: true, preload: 'metadata', controlslist: 'nofullscreen' });
  n.video.addEventListener('timeupdate', suivreLecture);
  // Pendant la lecture, le suivi passe à ~12 fois par seconde : le mot surligné suit la voix.
  n.video.addEventListener('play', () => { clearInterval(E.suiviFin); E.suiviFin = setInterval(suivreLecture, 80); });
  n.video.addEventListener('pause', () => clearInterval(E.suiviFin));
  n.video.addEventListener('pause', () => { dessinerOutilsLecteur(); dessinerCalque(); });
  const enLecture = () => E.noeuds.listePhrases?.classList.toggle('en-lecture', !n.video.paused);
  n.video.addEventListener('play', enLecture);
  n.video.addEventListener('pause', enLecture);
  n.video.addEventListener('play', () => { dessinerOutilsLecteur(); dessinerCalque(); });
  n.video.addEventListener('seeked', dessinerCalque);
  n.video.addEventListener('loadedmetadata', dessinerCalque);
  n.calque = h('div', { class: 'calque' });
  n.indicateurDepot = h('div', { class: 'indicateur-depot cache' }, 'Déposez ici : la position et l’instant de la vidéo seront retenus');
  n.ecran = h('div', { class: 'lecteur' }, n.video, n.calque, n.indicateurDepot);
  n.ecran.addEventListener('dblclick', (e) => { if (e.target === n.video) basculerPleinEcran(); });
  // Un double-clic sur la vidéo (Firefox) peut encore la mettre seule en plein écran : on y met le cadre à la place.
  document.addEventListener('fullscreenchange', () => {
    if (document.fullscreenElement === n.video) document.exitFullscreen().then(() => n.ecran.requestFullscreen?.()).catch(() => {});
    dessinerOutilsLecteur();
  });
  // PostProd en direct (WebGL) : null si le navigateur ne le permet pas, l'aperçu reste alors sans PostProd.
  n.direct = creerApercuDirect(n.video, n.ecran);
  // Son en direct (Web Audio) : null si le navigateur n'a pas les processeurs audio.
  n.son = window.AudioWorkletNode ? creerSonDirect(n.video) : null;
  n.habillage = creerHabillageDirect(n.video, n.ecran);
  n.textes = creerTextesDirect(n.video, n.ecran);
  const pause = () => n.ecran.classList.toggle('en-pause', n.video.paused);
  n.video.addEventListener('play', pause);
  n.video.addEventListener('pause', pause);
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
  majApercuDirect();
  majSonDirect();
  majHabillageDirect();
  majTextesDirect();
}

/** Sous-titres, titres des parties, textes et images posés, en direct sur l'aperçu (le programme en cours). */
function majTextesDirect() {
  const n = E.noeuds;
  if (!n.textes || !E.vue) return;
  const p = E.vue.projet;
  const actif = E.lecteur === 'apercu' && !!p.apercu?.rapport?.textes_direct && !!n.video.dataset.url && !!E.programme;
  // Le calque d'édition (en pause) garde ses poignées ; l'image qu'on voit est celle du direct.
  n.calque.classList.toggle('direct', actif);
  n.textes.regler(actif ? {
    programme: E.programme, reglages: p.reglages, decalage: decalageGenerique(p.apercu),
    urlMedia: (id) => { const m = p.medias.find((x) => x.id === id); return m && m.genre !== 'son' ? { url: urlMedia(m), genre: m.genre } : null; },
  } : null, actif);
}

/** Habillage en direct sur l'aperçu : celui qu'on règle (`enCours`), sinon celui du projet. Le rendu final l'a dans
 *  son image ; un aperçu d'avant le direct aussi. */
function majHabillageDirect(enCours = null) {
  const n = E.noeuds;
  if (!n.habillage || !E.vue) return;
  const p = E.vue.projet;
  const actif = E.lecteur === 'apercu' && !!p.apercu?.rapport?.habillage_direct && !!n.video.dataset.url;
  const liste = (enCours || p.habillage || []).map((x) => {
    const m = p.medias.find((y) => y.id === x.media);
    return { ...x, url: m ? urlMedia(m) : '' };
  });
  n.habillage.regler(liste, actif);
}

const CLES_SON_DIRECT = ['reverberation', 'voix', 'timbre_voix', 'harmoniques', 'deesseur', 'espace'];
let sonDirectCle = '';
let sonNiveau = { cle: '', gain: 9 };

/** Son en direct sur l'aperçu : la suite de la chaîne de la voix, le lieu, le fond, le gain final mesuré. */
async function majSonDirect(forcer = false) {
  const n = E.noeuds;
  if (!n.son || !E.vue) return;
  const p = E.vue.projet;
  const pistesAp = p.apercu?.rapport?.son_direct;
  const actif = E.sonDirect && E.lecteur === 'apercu' && !!pistesAp && !!n.video.dataset.url;
  const cle = JSON.stringify([actif, p.apercu?.fichier, CLES_SON_DIRECT.map((k) => p.reglages[k])]);
  if (cle === sonDirectCle && !forcer) return;
  sonDirectCle = cle;
  if (!actif) { n.son.regler({ actif: false }); return; }
  try {
    const d = await api('GET', `/api/projets/${E.projetId}/son/direct`);
    const url = (f) => (f ? `/api/projets/${E.projetId}/sorties/${encodeURIComponent(f)}` : '');
    const cleNiveau = JSON.stringify([p.apercu.fichier, d.filtres, d.lieu?.url || '']);
    const config = { actif: true, filtres: d.filtres, lieuUrl: d.lieu?.url || '', part: d.lieu?.part || 0,
      gainDb: sonNiveau.gain, pistes: { fond: url(pistesAp.fond), generique: url(pistesAp.generique) } };
    await n.son.regler(config);
    // Le gain final se mesure sur tout l'aperçu (quelques secondes la première fois) : le son change d'abord, le
    // niveau suit.
    if (cleNiveau !== sonNiveau.cle) {
      const r = await api('GET', `/api/projets/${E.projetId}/son/niveau`);
      sonNiveau = { cle: cleNiveau, gain: r.gain };
      if (sonDirectCle === cle) await n.son.regler({ gainDb: r.gain });
    }
  } catch (e) { toast(`Son en direct : ${e.message}`, 'erreur'); }
}

const PP_NEUTRE = { couleur: 'naturel', exposition: 0, contraste: 0, saturation: 0, temperature: 0, lissage: 0, diffusion: 0, nettete: 0, grain: 0, vignette: 0, bandes: false };

/** La PostProd en cours (celle qu'on règle, sinon celle du projet) sur l'aperçu et la vidéo d'origine. Le rendu
 *  final l'a déjà dans son image : on ne la rajoute pas. */
function majApercuDirect() {
  const n = E.noeuds;
  if (!n.direct || !E.vue) return;
  const enReglage = E.onglet === 'postprod' && E.postprod && E.postprodDe === E.projetId;
  const pp = { ...PP_NEUTRE, ...(enReglage ? E.postprod : E.vue.projet.postprod) };
  const neutre = Object.keys(PP_NEUTRE).every((k) => pp[k] === PP_NEUTRE[k]);
  // Un aperçu rendu avant le direct a déjà la PostProd dans son image : on ne l'applique pas deux fois.
  const dejaDansImage = E.lecteur === 'apercu' && !E.vue.projet.apercu?.rapport?.postprod_direct;
  const actif = E.ppDirect && E.lecteur !== 'rendu' && !dejaDansImage && !!n.video.dataset.url && !neutre;
  const couleurNeutre = pp.couleur === 'naturel' && !pp.exposition && !pp.contraste && !pp.saturation && !pp.temperature;
  const table = couleurNeutre ? '' : `/api/postprod/lut?${new URLSearchParams({ couleur: pp.couleur, exposition: pp.exposition, contraste: pp.contraste, saturation: pp.saturation, temperature: pp.temperature })}`;
  n.direct.regler(pp, table);
  n.direct.activer(actif);
}

function basculerPleinEcran() {
  const n = E.noeuds;
  if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
  else n.ecran.requestFullscreen?.().catch((e) => toast(`Plein écran impossible : ${e.message}`, 'erreur'));
}

function changerLecteur(mode, rush) {
  E.lecteur = mode;
  if (rush) E.rushLecteur = rush;
  dessinerLecteur();
  dessinerPhrases();
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
  const pleinEcran = n.ecran.requestFullscreen ? h('button', { class: 'btn petit', title: 'Plein écran, avec sous-titres, habillage et PostProd (double-clic sur la vidéo)',
    onclick: basculerPleinEcran }, icone('film'), document.fullscreenElement === n.ecran ? 'Quitter le plein écran' : 'Plein écran') : null;
  remplir(n.outilsLecteur, commenter, pleinEcran, choixRush, h('span', { class: 'espace' }),
    bascule('voirTlOrigine', 'Timeline d’origine', E.voirTlOrigine),
    bascule('voirTlMontage', 'Timeline du montage', E.voirTlMontage),
    E.lecteur !== 'rendu' && n.direct ? h('label', { class: 'bascule', title: 'Voir la PostProd (couleur, teint, grain…) en direct sur la vidéo ; décochez pour comparer avec l’image brute' },
      h('input', { type: 'checkbox', checked: E.ppDirect, onchange: (e) => { E.ppDirect = e.target.checked; stocker('edivid_ppDirect', E.ppDirect ? '1' : '0'); majApercuDirect(); } }),
      h('span', {}, 'PostProd en direct')) : null,
    E.lecteur === 'apercu' && n.son && E.vue.projet.apercu?.rapport?.son_direct ? h('label', { class: 'bascule', title: 'Le son réglé (écho, timbre, lieu, niveau) en direct ; décochez pour entendre la voix de base, sans musique' },
      h('input', { type: 'checkbox', checked: E.sonDirect, onchange: (e) => { E.sonDirect = e.target.checked; stocker('edivid_sonDirect', E.sonDirect ? '1' : '0'); majSonDirect(true); } }),
      h('span', {}, 'Son en direct')) : null);
}

const NOMS_REGIME_LONG = { respiration: 'Respiration', rafale: 'Rafale', impact: 'Impact' };

/** Plans de la vidéo affichée (aperçu ou rendu) : ce qui a été réellement rendu. */
/** Durée de la séquence d'ouverture du générique dans une vidéo rendue (0 sans générique). */
function decalageGenerique(s) { return s?.rapport?.generique?.debut || 0; }

/** Plans de la vidéo jouée, au temps de la vidéo : l'ouverture du générique les décale. */
function plansAffiches() {
  const s = E.lecteur === 'rendu' ? E.vue.projet.rendu : E.vue.projet.apercu;
  const plans = s?.rapport?.plans_detail || [];
  const d = decalageGenerique(s);
  return d ? plans.map((x) => ({ ...x, t: x.t + d })) : plans;
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
        onclick: () => {
          if (E.lecteur === 'origine') changerLecteur(p.apercu ? 'apercu' : 'rendu');
          setTimeout(() => { n.video.currentTime = pl.t + decalageGenerique(E.lecteur === 'rendu' ? p.rendu : p.apercu) + 0.01; }, 50);
        },
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
    r.son ? h('p', { class: 'tres-petit muet', style: { margin: '10px 0 0' } }, icone('onde'), ` Son de cette vidéo — ${r.son}`,
      E.aMettreAJour ? h('b', { style: { color: 'var(--alerte)' } }, ' · des changements attendent un nouvel aperçu') : null) : null,
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
      const url = urlMedia(m);
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
    tMontage = t - decalageGenerique(E.lecteur === 'rendu' ? p.rendu : p.apercu);
    if (tMontage < 0 || tMontage > totalMontage) tMontage = null;
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
    cle = pl ? (pl.accroche ? CLE_ACCROCHE : pl.phrase) : null;
  }
  if (cle !== E.phraseJouee) {
    E.phraseJouee = cle;
    E.noeuds.listePhrases?.querySelectorAll('.phrase.joue').forEach((x) => x.classList.remove('joue'));
    const el = cle ? E.noeuds.listePhrases?.querySelector(`[data-cle="${CSS.escape(cle)}"]`) : null;
    if (el) {
      el.classList.add('joue');
      const liste = E.noeuds.listePhrases;
      if (el.offsetTop < liste.scrollTop + 20 || el.offsetTop + el.offsetHeight > liste.scrollTop + liste.clientHeight - 20) {
        liste.scrollTo({ top: el.offsetTop - liste.clientHeight / 3, behavior: 'smooth' });
      }
    }
  }
  if (cle && srcT !== null) marquerMot(cle, srcT);
}

/** Dans la phrase jouée, le mot prononcé à l'instant srcT (temps du rush) est mis en évidence. */
function marquerMot(cle, srcT) {
  const el = E.noeuds.listePhrases?.querySelector(`[data-cle="${CSS.escape(cle)}"]`);
  const zone = el?.querySelector('.texte');
  const ph = E.vue.phrases.find((x) => x.cle === cle);
  if (!zone || !ph?.mots?.length) return;
  if (!zone.dataset.mots) {
    zone.dataset.mots = '1';
    const marque = zone.querySelector('.marque-corrige');
    const morceaux = [];
    ph.mots.forEach(([, , m], i) => {
      if (i && !/^['’-]/.test(m)) morceaux.push(' ');
      morceaux.push(h('span', { class: 'mot', 'data-i': i }, m));
    });
    remplir(zone, morceaux, marque);
  }
  const i = ph.mots.findIndex(([a, b]) => srcT >= a - 0.05 && srcT <= b + 0.12);
  const actuel = zone.querySelector('.mot.joue');
  if (actuel && Number(actuel.dataset.i) === i) return;
  actuel?.classList.remove('joue');
  if (i >= 0) zone.querySelector(`[data-i="${i}"]`)?.classList.add('joue');
}

function allerALaPhrase(cle) {
  const n = E.noeuds;
  // La phrase en cours de lecture : un clic la met en pause, le suivant reprend là où elle en était.
  if (cle === E.phraseJouee && n.video.dataset.url) {
    if (n.video.paused) n.video.play(); else n.video.pause();
    return;
  }
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

const NOMS_REGIME = { rafale: 'Rafale', impact: 'Impact', respiration: 'Respiration' };
const NOMS_CADRE = { large: 'Plan large', moyen: 'Plan moyen', visage: 'Gros plan', yeux: 'Yeux' };
const AIDE_REGIME = 'Rythme de la phrase, sans toucher au cadre — Rafale : coupes serrées, un plan sur deux resserré d’un cran. '
  + 'Impact : coupe sèche, l’image pousse vers le visage pendant le plan. Respiration : les silences sont gardés, le plan est tenu.';
const NOMS_MOUVEMENT = { auto: 'Mouvement selon le rythme', fixe: 'Fixe', pousse: 'Zoom avant continu', punch: 'Punch à chaque coupe' };
const AIDE_MOUVEMENT = 'Mouvement de l’image pendant la phrase — Selon le rythme : zoom avant continu pour un Impact, punch pour une Rafale (si les réglages Zoom qui avance et Recadrer à chaque coupe sont allumés). '
  + 'Fixe : aucun zoom. Zoom avant continu : un seul mouvement vers le visage sur toute la phrase. Punch : un plan sur deux resserré d’un cran.';
const NOMS_TRANSITION = { auto: 'Auto', coupe: 'Aucune (coupe franche)', flash: 'Flash blanc', fondu: 'Fondu au noir', zoom: 'Zoom', glisse: 'Glissé flou', glitch: 'Glitch' };
const AIDE_TRANSITION = 'Transition au début de la phrase. Auto : celle du LLM ou du réglage général, au début de chaque partie. Aucune : coupe franche, sans effet.';
const AIDE_CADRE = 'Cadre de la phrase — Plan large : l’image entière. Plan moyen : buste. Gros plan : visage. Yeux : le plus serré que la vidéo permet, centré sur le regard.';
/** Ligne de l'accroche (phrases reprises en ouverture) dans le texte : elle se surligne pendant qu'elle joue. */
const CLE_ACCROCHE = '__accroche';

function menuPhrase(cle, champ, valeur, noms, aide, libelle) {
  return h('select', { class: `puce ${champ === 'regime' ? valeur : ''} ${valeur === 'auto' ? 'auto' : ''}`, title: aide, 'aria-label': libelle,
    onchange: (e) => changerPhrase(cle, { [champ]: e.target.value }) },
  Object.entries(noms).map(([v, l]) => h('option', { value: v, selected: v === valeur }, l)));
}

/** Transition réellement jouée au début d'une phrase dans le montage en cours (null : coupe franche). */
function transitionJouee(cle) {
  return (E.programme?.plans || []).find((x) => x.phrase === cle && !x.accroche)?.transition || null;
}

/** Phrases de l'accroche dans le montage en cours, dans leur ordre. */
function clesAccroche() {
  return [...new Set((E.programme?.plans || []).filter((x) => x.accroche).map((x) => x.phrase))];
}

async function changerAccroche(cles) {
  try {
    await api('POST', `/api/projets/${E.projetId}/accroche`, { cles });
    E.aMettreAJour = true;
    dessinerBarre();
  } catch (e) { toast(e.message, 'erreur'); }
}

async function fusionner(cle, sens) {
  try {
    await api('POST', `/api/projets/${E.projetId}/fusion`, { cle, sens });
    E.aMettreAJour = true;
    dessinerBarre();
  } catch (e) { toast(e.message, 'erreur'); }
}

/** Petit menu d'actions (⋯) ; il se ferme au clic suivant, où qu'il soit. */
function menuActions(titre, actions) {
  const liste = h('div', { class: 'menu-actions', role: 'menu', hidden: true },
    actions.filter(Boolean).map(([ic, libelle, faire]) => h('button', { role: 'menuitem', onclick: () => { liste.hidden = true; faire(); } }, icone(ic), libelle)));
  const bouton = h('button', { class: 'btn icone fantome', title: titre, 'aria-haspopup': 'menu',
    onclick: (e) => {
      e.stopPropagation();
      const ouvrir = liste.hidden;
      document.querySelectorAll('.menu-actions').forEach((m) => { m.hidden = true; });
      liste.hidden = !ouvrir;
      if (ouvrir) setTimeout(() => document.addEventListener('click', () => { liste.hidden = true; }, { once: true }));
    } }, '⋯');
  return h('div', { class: 'menu-ancre' }, bouton, liste);
}

/** Ligne de l'accroche en tête du texte : chaque phrase reprise se retire, l'accroche entière se supprime. */
function ligneAccroche() {
  const cles = clesAccroche();
  if (!cles.length) return null;
  const affiches = plansAffiches().filter((x) => x.accroche);
  const debut = affiches[0]?.t ?? 0;
  return h('div', { class: 'phrase impact accroche-ligne', 'data-cle': CLE_ACCROCHE },
    h('div', { class: 'temps' }, minutes(debut)),
    h('div', { style: { minWidth: 0 } },
      h('div', { class: 'etiquette-accroche' }, 'Accroche — reprise en ouverture de la vidéo'),
      h('div', { class: 'puces-accroche' }, cles.map((c) => {
        const ph = E.vue.phrases.find((x) => x.cle === c);
        return h('span', { class: 'puce-accroche' },
          h('span', { class: 'texte', title: 'Clic : écouter l’accroche — double-clic : corriger le texte de cette phrase',
            onclick: () => { if (!affiches.length) { toast('Mettez l’aperçu à jour pour écouter cette accroche.'); return; }
              if (E.lecteur === 'origine') changerLecteur('apercu'); setTimeout(() => { E.noeuds.video.currentTime = debut + 0.01; E.noeuds.video.play(); }, 50); },
            ondblclick: () => { E.edition = c; dessinerPhrases(); E.noeuds.listePhrases.querySelector(`[data-cle="${CSS.escape(c)}"]`)?.scrollIntoView({ block: 'center' }); } },
          `« ${ph?.texte || '…'} »`),
          h('button', { class: 'puce-retirer', title: 'Retirer cette phrase de l’accroche', 'aria-label': 'Retirer de l’accroche',
            onclick: () => changerAccroche(cles.filter((x) => x !== c)) }, icone('fermer')));
      }))),
    h('div', { class: 'boutons' },
      h('button', { class: 'btn icone fantome', title: 'Supprimer l’accroche : la vidéo commence directement par le début', onclick: () => changerAccroche([]) }, icone('corbeille'))));
}

async function changerPhrase(cle, changement) {
  try {
    const r = await api('POST', `/api/projets/${E.projetId}/phrase`, { cle, ...changement });
    if (r.fait) { E.aMettreAJour = true; dessinerBarre(); }
  } catch (e) { toast(e.message, 'erreur'); }
}

/** Temps d'une phrase, sur la même horloge que le lecteur : dans la vidéo montée (aperçu ou rendu) si elle y est,
 *  sinon dans le rush, en italique. Le survol donne les deux. */
function tempsPhrase(ph) {
  const pl = E.lecteur !== 'origine' ? plansAffiches().find((x) => x.phrase === ph.cle && !x.accroche) : null;
  const rush = `dans le rush : ${minutes(ph.t0)}`;
  if (pl) return h('span', { class: 'horloge', title: `Dans la vidéo : ${minutes(pl.t)} — ${rush}` }, minutes(pl.t));
  return h('span', { class: 'horloge rush', title: E.lecteur === 'origine' ? rush : `Pas dans la vidéo montée — ${rush}` }, minutes(ph.t0));
}

/** Mots d'une phrase tels qu'ils passent à l'écran : gras = en sous-titre, surligné = en emphase, grisé = pas
 *  affiché (retiré à la main, ou coupé au montage avec une hésitation ou un silence). */
function motsPhrase(ph, zone) {
  const st = E.programme?.sous_titres?.[ph.cle];
  const affiches = new Map(st || []);
  const masques = new Set(ph.decision?.masques || []);
  const choisis = E.editionSt === ph.cle ? E.selectionSt : null;
  const morceaux = [];
  ph.mots.forEach(([, , m], i) => {
    if (i && !/^['’-]/.test(m)) morceaux.push(' ');
    const classes = ['mot'];
    if (st) classes.push(affiches.has(i) ? 'affiche' : masques.has(i) ? 'masque' : 'hors');
    if (affiches.get(i)) classes.push('emphase');
    if (choisis?.has(i)) classes.push('choisi');
    morceaux.push(h('span', { class: classes.join(' '), 'data-i': i,
      title: !st ? '' : affiches.has(i) ? '' : masques.has(i) ? 'Retiré des sous-titres' : 'Coupé au montage (hésitation, silence) : « Affiché » peut le forcer à l’écran' }, m));
  });
  zone.dataset.mots = '1';
  return morceaux;
}

/** Éditeur des sous-titres d'une phrase : on choisit des mots (clic, ou glisser), puis Gras les affiche ou les
 *  retire, Surligner leur donne l'emphase ou la leur retire. */
function editeurSousTitres(ph, zone) {
  const d = ph.decision || {};
  const st = new Map(E.programme?.sous_titres?.[ph.cle] || []);
  const choix = () => [...E.selectionSt].sort((a, b) => a - b);
  const montrer = () => zone.querySelectorAll('.mot').forEach((el) => el.classList.toggle('choisi', E.selectionSt.has(Number(el.dataset.i))));
  // Clic : ce mot seul. Glisser ou Maj+clic : tous les mots entre l'ancre et celui-ci. Ctrl+clic : ajouter ou retirer.
  const plage = (a, b) => { E.selectionSt = new Set(); for (let k = Math.min(a, b); k <= Math.max(a, b); k++) E.selectionSt.add(k); };
  zone.addEventListener('mousedown', (e) => {
    const el = e.target.closest('.mot');
    if (!el) return;
    e.preventDefault();
    const i = Number(el.dataset.i);
    if (e.shiftKey && E.ancreSt != null) plage(E.ancreSt, i);
    else if (e.ctrlKey || e.metaKey) { if (E.selectionSt.has(i)) E.selectionSt.delete(i); else E.selectionSt.add(i); E.ancreSt = i; }
    else { E.selectionSt = new Set([i]); E.ancreSt = i; }
    montrer();
    const suivre = (ev) => { const m = ev.target.closest?.('.mot'); if (m && zone.contains(m) && ev.buttons) { plage(E.ancreSt, Number(m.dataset.i)); montrer(); } };
    zone.addEventListener('mouseover', suivre);
    document.addEventListener('mouseup', () => zone.removeEventListener('mouseover', suivre), { once: true });
  });
  // Chaque action attend la réponse du serveur puis recharge le texte : la suivante part de l'état à jour.
  let occupe = false;
  const appliquer = async (changement) => {
    if (occupe) return;
    occupe = true;
    try { await changerPhrase(ph.cle, changement); await rafraichir(); } finally { occupe = false; }
  };
  const exiger = () => {
    const c = choix();
    if (!c.length) toast('Choisissez d’abord des mots : clic sur un mot, ou glisser sur plusieurs.');
    return c;
  };
  const masques = new Set(d.masques || []);
  const forces = new Set(d.forces || []);
  // Tous les mots choisis sont à l'écran : on les retire. Sinon on les met tous à l'écran, en forçant ceux que le
  // montage coupe (ils s'affichent au bord du morceau gardé le plus proche).
  const gras = () => {
    const c = exiger();
    if (!c.length) return;
    if (c.every((i) => st.has(i))) {
      c.forEach((i) => { masques.add(i); forces.delete(i); });
    } else {
      c.forEach((i) => { masques.delete(i); if (!st.has(i)) forces.add(i); });
    }
    appliquer({ masques: [...masques], forces: [...forces] });
  };
  const surligner = () => {
    const c = exiger();
    if (!c.length) return;
    const emphase = new Set(d.emphase || [...st].filter(([, fort]) => fort).map(([i]) => i));
    const tous = c.every((i) => emphase.has(i));
    c.forEach((i) => (tous ? emphase.delete(i) : emphase.add(i)));
    appliquer({ emphase: [...emphase] });
  };
  const nbHors = ph.mots.filter((_, i) => !st.has(i) && !masques.has(i)).length;
  return h('div', { class: 'editeur-st' },
    h('button', { class: 'btn petit', title: 'Afficher ou retirer des sous-titres les mots choisis', onclick: gras }, h('b', {}, 'G'), 'Affiché'),
    h('button', { class: 'btn petit', title: 'Donner ou retirer l’emphase aux mots choisis (couleur et taille, style dynamique)', onclick: surligner }, h('span', { class: 'emphase' }, 'S'), 'Emphase'),
    h('button', { class: 'btn petit fantome', disabled: !masques.size, title: masques.size ? 'Remettre dans les sous-titres les mots retirés à la main' : 'Aucun mot retiré à la main',
      onclick: () => appliquer({ masques: [] }) }, masques.size ? `Tout afficher (${masques.size} retiré${masques.size > 1 ? 's' : ''})` : 'Tout afficher'),
    d.emphase ? h('button', { class: 'btn petit fantome', title: 'Revenir aux mots forts choisis par le LLM', onclick: () => appliquer({ emphase_auto: true }) }, 'Emphase du LLM') : null,
    h('span', { class: 'espace' }),
    h('button', { class: 'btn petit primaire', onclick: () => { E.editionSt = null; dessinerPhrases(); } }, 'Fermer'),
    h('div', { class: 'tres-petit muet', style: { flexBasis: '100%' } },
      'Clic sur un mot pour le choisir (glisser, ou Maj+clic, pour plusieurs), puis Affiché ou Emphase. Gras : à l’écran. Surligné : en emphase. Barré : retiré à la main.',
      nbHors ? ` Grisé : coupé au montage (hésitation, silence), ${nbHors} mot${nbHors > 1 ? 's' : ''} ; « Affiché » le force à l’écran, au bord du passage gardé le plus proche.` : '',
      texte_style_sous_titres()));
}

function texte_style_sous_titres() {
  const r = E.vue.projet.reglages;
  if (!r.sous_titres) return ' Les sous-titres sont éteints dans les réglages.';
  return r.style_sous_titres === 'dynamique' ? '' : ' L’emphase ne se voit qu’avec le style de sous-titres « dynamique ».';
}

/** Titre de la partie qui commence à cette phrase : modifiable en place, supprimable. */
function ligneChapitre(c) {
  const champ = h('input', { class: 'champ titre-partie', value: c.titre, maxlength: '80', 'aria-label': 'Titre de la partie' });
  const enregistrer = async (titre) => {
    try { await api('POST', `/api/projets/${E.projetId}/chapitre`, { cle: c.cle, titre }); E.aMettreAJour = true; dessinerBarre(); }
    catch (e) { toast(e.message, 'erreur'); }
  };
  champ.addEventListener('change', () => enregistrer(champ.value));
  champ.addEventListener('keydown', (e) => { if (e.key === 'Enter') champ.blur(); });
  const actif = E.vue.projet.reglages.titres_chapitres;
  return h('div', { class: 'partie' },
    h('span', { class: 'etiquette-accroche', title: actif ? 'Titre affiché à l’image au début de la partie' : 'Les titres de parties sont éteints dans les réglages' },
      actif ? 'Partie' : 'Partie (titre non affiché)'),
    champ,
    h('button', { class: 'btn icone fantome', title: 'Supprimer cette partie et son titre', onclick: () => enregistrer('') }, icone('corbeille')));
}

/** Images et vidéos posées sur la phrase, en vignettes sous ses outils : un clic rouvre leurs réglages. */
function vignettesIncrustations(ph) {
  const p = E.vue.projet;
  const liste = p.incrustations.filter((i) => i.phrase === ph.cle && i.media);
  if (!liste.length) return null;
  return h('div', { class: 'vignettes-incrustations' }, liste.map((inc) => {
    const m = p.medias.find((x) => x.id === inc.media);
    const arrivee = ARRIVEES.find(([v]) => v === (inc.entree || 'fondu'))?.[1] || inc.entree;
    return h('button', { class: 'vignette-incrustation', title: `${m?.nom || 'Média'} — ${arrivee}, ${fr(inc.duree)} s. Cliquez pour régler.`,
      onclick: () => dialogueIncrustation(ph, inc) }, m ? vignetteMedia(m) : icone('image'));
  }));
}

/** Actions plus rares d'une phrase : accroche, soudure avec une voisine du même rush. */
function menuPhraseActions(ph, phrases) {
  const i = phrases.indexOf(ph);
  const prec = phrases[i - 1];
  const suiv = phrases[i + 1];
  const accroche = clesAccroche();
  const dans = accroche.includes(ph.cle);
  const partie = (E.vue.projet.montage?.chapitres || []).some((c) => c.cle === ph.cle);
  return menuActions('Plus d’actions sur cette phrase', [
    ['texte', 'Régler les sous-titres de cette phrase', () => {
      // La lecture fait défiler le texte : on la met en pause pendant le réglage.
      E.noeuds.video.pause();
      E.editionSt = ph.cle; E.selectionSt = new Set(); E.ancreSt = null;
      dessinerPhrases();
    }],
    partie ? null : ['signet', 'Commencer une partie ici', () => api('POST', `/api/projets/${E.projetId}/chapitre`, { cle: ph.cle, titre: 'Nouvelle partie' })
      .then(() => { E.aMettreAJour = true; dessinerBarre(); }).catch((e) => toast(e.message, 'erreur'))],
    ['etoile', dans ? 'Retirer de l’accroche' : 'Ajouter à l’accroche', () => changerAccroche(dans ? accroche.filter((c) => c !== ph.cle) : [...accroche, ph.cle])],
    prec && prec.rush === ph.rush ? ['haut', 'Souder à la phrase précédente', () => fusionner(ph.cle, 'precedente')] : null,
    suiv && suiv.rush === ph.rush ? ['bas', 'Souder à la phrase suivante', () => fusionner(ph.cle, 'suivante')] : null,
    ph.fusionnees?.length ? ['ciseaux', 'Séparer la phrase soudée', () => fusionner(ph.cle, 'separer')] : null,
  ]);
}

function dessinerPhrases() {
  const { phrases, projet: p } = E.vue;
  const monte = !!p.montage;
  const visibles = phrases.filter((ph) => E.filtre === 'toutes' || !ph.decision
    || (E.filtre === 'gardees' ? ph.decision.garder : !ph.decision.garder));
  const nbCoupees = phrases.filter((ph) => ph.decision && !ph.decision.garder).length;
  const debut = E.noeuds.listePhrases?.scrollTop || 0;
  const accroche = monte && E.filtre !== 'coupees' ? ligneAccroche() : null;
  const chapitres = new Map((p.montage?.chapitres || []).map((c) => [c.cle, c]));
  E.noeuds.listePhrases = h('div', { class: 'phrases' }, accroche, visibles.map((ph) => {
    const ligne = lignePhrase(ph);
    return chapitres.has(ph.cle) ? [ligneChapitre(chapitres.get(ph.cle)), ligne] : ligne;
  }));
  function lignePhrase(ph) {
    const d = ph.decision;
    const classe = d ? (d.garder ? d.regime : 'coupee') : '';
    const corrigee = !!p.corrections?.[ph.cle];
    if (E.edition === ph.cle) return ligneEdition(ph, corrigee);
    // En réglage des sous-titres, un clic choisit des mots : il ne lance pas la lecture.
    const enReglage = E.editionSt === ph.cle && d?.garder;
    const zoneTexte = enReglage
      ? h('div', { class: 'texte', style: { cursor: 'text' }, title: 'Clic : choisir un mot — glisser ou Maj+clic : plusieurs — Ctrl+clic : ajouter ou retirer un mot' })
      : h('div', { class: 'texte', style: { cursor: 'pointer' }, title: 'Clic : écouter ce passage — double-clic : corriger le texte',
        onclick: () => allerALaPhrase(ph.cle), ondblclick: () => { E.edition = ph.cle; dessinerPhrases(); } });
    remplir(zoneTexte, monte && d?.garder && E.programme ? motsPhrase(ph, zoneTexte) : ph.texte);
    const editeur = enReglage ? editeurSousTitres(ph, zoneTexte) : null;
    return h('div', { class: `phrase ${classe}${editeur ? ' edition-st' : ''}`, 'data-cle': ph.cle },
      // Lecture et pause : l'icône suit la vidéo (classe « en-lecture » de la liste, posée par le lecteur).
      h('button', { class: 'temps', title: 'Écouter ce passage, ou le mettre en pause', onclick: () => allerALaPhrase(ph.cle) },
        h('span', { class: 'ic-lecture' }, icone('lecture')), h('span', { class: 'ic-pause' }, icone('pause')), tempsPhrase(ph)),
      h('div', { style: { minWidth: 0 } },
        zoneTexte, editeur,
        d && d.garder ? h('div', { class: 'etiquettes' },
          menuPhrase(ph.cle, 'regime', d.regime, NOMS_REGIME, AIDE_REGIME, 'Rythme'),
          menuPhrase(ph.cle, 'cadre', d.cadre, NOMS_CADRE, AIDE_CADRE, 'Cadre'),
          menuPhrase(ph.cle, 'mouvement', d.mouvement || 'auto', NOMS_MOUVEMENT, AIDE_MOUVEMENT, 'Mouvement'),
          menuPhrase(ph.cle, 'transition', d.transition_choisie || 'auto',
            { ...NOMS_TRANSITION, auto: `Entrée auto : ${transitionJouee(ph.cle) ? NOMS_TRANSITION[transitionJouee(ph.cle)] || transitionJouee(ph.cle) : 'coupe franche'}` },
            AIDE_TRANSITION, 'Transition d’entrée'),
          ph.fusionnees?.length ? h('span', { class: 'puce', title: 'Phrase soudée à la main' }, '⛓ soudée') : null,
          p.incrustations.filter((i) => i.phrase === ph.cle && !i.media).map((inc) => h('span', { class: 'puce puce-incrustation', title: `${nomIncrustation(inc)} — ${fr(inc.duree)} s` },
            `✦ ${nomIncrustation(inc).slice(0, 26)}`,
            h('button', { class: 'puce-retirer', title: 'Retirer cette incrustation', 'aria-label': `Retirer ${nomIncrustation(inc)}`,
              onclick: async (e) => { e.stopPropagation(); await retirerIncrustation(inc); } }, icone('fermer'))))) : null),
      monte ? h('div', { class: 'boutons' },
        h('button', { class: 'btn icone fantome', title: 'Corriger le texte (orthographe, mot mal transcrit)', onclick: () => { E.edition = ph.cle; dessinerPhrases(); } }, icone('crayon')),
        h('button', { class: 'btn icone fantome', title: 'Ajouter une incrustation (image, GIF, vidéo, effet)', onclick: () => dialogueIncrustation(ph) }, icone('baguette')),
        h('button', { class: 'btn icone fantome', title: d && !d.garder ? 'Rétablir cette phrase' : 'Couper cette phrase', onclick: () => changerPhrase(ph.cle, { garder: !(d ? d.garder : true) }) },
          icone(d && !d.garder ? 'relancer' : 'ciseaux')),
        menuPhraseActions(ph, phrases),
        vignettesIncrustations(ph)) : h('div', { class: 'boutons' },
        h('button', { class: 'btn icone fantome', title: 'Corriger le texte', onclick: () => { E.edition = ph.cle; dessinerPhrases(); } }, icone('crayon'))));
  }
  appliquerReplie(E.noeuds.phrases, 'phrases');
  remplir(E.noeuds.phrases,
    enteteRepliable('phrases', '3. Le texte de la vidéo',
      monte ? h('div', { class: 'onglets-petits' },
        [['toutes', 'Toutes'], ['gardees', 'Gardées'], ['coupees', `Coupées (${nbCoupees})`]].map(([v, l]) =>
          h('button', { class: E.filtre === v ? 'actif' : '', onclick: () => { E.filtre = v; dessinerPhrases(); } }, l))) : null),
    h('div', { class: 'carte-corps' },
      phrases.length === 0 ? h('p', { class: 'muet petit', style: { margin: 0 } }, 'La transcription apparaîtra ici après l’analyse.')
        : !monte ? h('div', { class: 'aide', style: { marginBottom: '10px' } }, icone('info'), h('span', {}, 'Voici ce qui est dit dans vos rushs. Proposez un montage : le LLM décidera, phrase par phrase, ce qu’il garde, le rythme et le cadre.'))
          : h('p', { class: 'tres-petit muet', style: { margin: '0 0 8px' } }, 'Cliquez une phrase pour l’écouter, double-cliquez pour corriger son texte. Ses menus règlent le rythme, le cadre, le mouvement et la transition d’entrée, chacun sans toucher aux autres. Les ciseaux la coupent, la baguette pose une incrustation, ⋯ la met en accroche ou la soude à sa voisine.'),
      phrases.length ? zoneRemplacer() : null,
      E.noeuds.listePhrases));
  E.noeuds.listePhrases.scrollTop = debut;
  E.noeuds.listePhrases.classList.toggle('en-lecture', !E.noeuds.video.paused);
  E.phraseJouee = null;
  suivreLecture();
}

// ------------------------------------------------------------------ panneau latéral et discussion

function construirePanneau() {
  const n = E.noeuds;
  n.onglets = h('div', { class: 'onglets', role: 'tablist' });
  n.corpsPanneau = h('div', { style: { display: 'flex', flexDirection: 'column', minHeight: 0, flex: 1 } });
  // Poignée toujours visible sur le bord du tiroir : un clic le rentre ou le sort.
  n.poignee = h('button', { class: 'poignee', onclick: () => { stocker('edivid_replie_panneau', estReplie('panneau') ? '0' : '1'); dessinerPanneau(); } });
  remplir(n.panneau, n.poignee, h('div', { class: 'contenu-tiroir' }, n.onglets, n.corpsPanneau));
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
  n.apercuDiscussion = h('span', { class: 'apercu-discussion muet' });
  n.chevronDiscussion = h('button', { class: 'btn icone fantome chevron', title: 'Replier ou déplier la discussion', onclick: (e) => { e.stopPropagation(); basculerDiscussion(); } }, icone('chevron'));
  n.teteDiscussion = h('div', { class: 'dock-tete', onclick: () => basculerDiscussion() },
    icone('discussion'), h('b', {}, 'Assistant'), n.apercuDiscussion, n.chevronDiscussion);
  remplir(n.dockDiscussion, h('div', { class: 'dock-int' }, n.teteDiscussion, n.discussion));
  appliquerDiscussion();
}

function appliquerDiscussion() {
  const n = E.noeuds;
  const replie = estReplie('discussion');
  n.dockDiscussion.classList.toggle('replie', replie);
  n.chevronDiscussion.setAttribute('aria-expanded', replie ? 'false' : 'true');
}

function basculerDiscussion(ouvrir) {
  const replie = ouvrir === undefined ? !estReplie('discussion') : !ouvrir;
  stocker('edivid_replie_discussion', replie ? '1' : '0');
  appliquerDiscussion();
  if (!replie) E.noeuds.messages.scrollTop = E.noeuds.messages.scrollHeight;
}

function dessinerPanneau() {
  const n = E.noeuds;
  const p = E.vue.projet;
  const onglets = [
    ['reglages', 'reglages', 'Réglages', 0],
    ['medias', 'image', 'Médias', p.medias.length],
    ['effets', 'baguette', 'Textes', p.incrustations.length],
    ['postprod', 'etincelles', 'PostProd', 0],
  ];
  const replie = estReplie('panneau');
  n.panneau.classList.toggle('rentre', replie);
  n.main.classList.toggle('tiroir-replie', replie);
  n.poignee.title = replie ? 'Sortir le panneau des réglages' : 'Rentrer le panneau sur le côté';
  n.poignee.setAttribute('aria-expanded', replie ? 'false' : 'true');
  remplir(n.poignee, icone('chevron'), h('span', {}, replie ? 'Réglages · Médias · Textes · PostProd' : 'Rentrer'));
  remplir(n.onglets,
    ...onglets.map(([id, ic, l, compte]) =>
      h('button', { role: 'tab', class: E.onglet === id ? 'actif' : '', 'aria-selected': E.onglet === id ? 'true' : 'false', title: l,
        onclick: () => { E.onglet = id; dessinerPanneau(); } },
      icone(ic), h('span', { class: 'libelle' }, l), compte ? h('span', { class: 'compte' }, compte) : null)));
  if (replie) return;
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
  const dernier = p.discussion[p.discussion.length - 1];
  n.apercuDiscussion.textContent = E.attenteReponse ? 'EDIVID réfléchit…' : dernier ? dernier.texte.replace(/\s+/g, ' ') : '';
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
      h('input', { type: 'range', min: r.bornes.min, max: r.bornes.max, step: unite === 'ms' ? '10' : (r.bornes.max - r.bornes.min <= 30 ? '1' : '5'), value: valeur, 'aria-label': r.titre,
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
  corps.append(blocOutil('consignes', { icone: 'livre', titre: 'Vos consignes au monteur', famille: 'recettes',
    resume: p.consignes?.trim() ? p.consignes.trim().replace(/\s+/g, ' ').slice(0, 80) : 'Aucune consigne pour l’instant' },
    h('div', { class: 'reglage-aide' }, 'Vos préférences, en toutes lettres. Le LLM les lit à chaque montage et dans la conversation, en plus de ses propres consignes.'),
    zone, exemples,
    h('button', { class: 'btn petit fantome', style: { marginTop: '6px' }, onclick: afficherCoulisses }, icone('livre'), 'Voir les consignes complètes du LLM')));
}

const POSITIONS_HABILLAGE = [['haut_gauche', 'En haut à gauche'], ['haut', 'En haut'], ['haut_droite', 'En haut à droite'],
  ['bas_gauche', 'En bas à gauche'], ['bas', 'En bas'], ['bas_droite', 'En bas à droite']];

/** Curseur d'un réglage d'élément : libellé, valeur affichée, et rappel à chaque mouvement. */
function curseurElement(libelle, valeur, min, max, pas, format, changer) {
  const sortie = h('b', { class: 'petit' }, format(valeur));
  return h('div', { class: 'reglage' }, h('div', { class: 'reglage-tete' }, h('span', { class: 'reglage-titre' }, libelle), sortie),
    h('input', { type: 'range', min, max, step: pas, value: valeur, 'aria-label': libelle,
      oninput: (e) => { sortie.textContent = format(Number(e.target.value)); changer(Number(e.target.value), false); },
      onchange: (e) => changer(Number(e.target.value), true) }));
}

/** Habillage : logos et textes posés sur toute la durée du montage. */
const ACCEPTE_MEDIA = { image: 'image/*,.png,.svg,.gif', video: 'video/*', son: 'audio/*' };

/** Envoie un fichier de l'ordinateur dans la médiathèque (ou remplace le fichier d'un média existant). */
function chargerMedia(accepte, rappel, remplacer = null) {
  choisirFichiers(accepte, false, async (fichiers) => {
    const f = fichiers[0];
    if (!f) return;
    try {
      const chemin = remplacer ? `/api/projets/${E.projetId}/medias/${remplacer}` : `/api/projets/${E.projetId}/medias`;
      const r = await fetch(`${chemin}?nom=${encodeURIComponent(f.name)}`, { method: 'PUT', body: f });
      const j = await r.json();
      if (!r.ok) throw new Error(j.erreur || `erreur ${r.status}`);
      E.aMettreAJour = true;
      await rappel?.(j.id);
      rafraichir();
    } catch (e) { toast(e.message, 'erreur'); }
  });
}

/** Choix d'un média : ceux de la médiathèque, ou un autre fichier (ordinateur, Google Drive). */
function choixMedia({ genres, valeur, vide, libelle, changer }) {
  const medias = E.vue.projet.medias.filter((m) => genres.includes(m.genre));
  const accepte = ACCEPTE_MEDIA[genres[0] === 'gif' ? 'image' : genres[0]];
  const choix = h('select', { class: 'champ', 'aria-label': libelle },
    vide !== undefined ? h('option', { value: '', selected: !valeur }, vide) : null,
    medias.map((m) => h('option', { value: m.id, selected: m.id === valeur }, m.nom)),
    h('option', { value: '__fichier' }, 'Autre fichier de l’ordinateur…'),
    E.driveActif ? h('option', { value: '__drive' }, 'Autre fichier sur Google Drive…') : null);
  choix.addEventListener('change', () => {
    const v = choix.value;
    choix.value = valeur || '';
    if (v === '__fichier') chargerMedia(accepte, (id) => changer(id));
    else if (v === '__drive') { choisirDepuisDrive('medias'); toast('Une fois importé, le fichier apparaîtra dans cette liste.'); }
    else changer(v);
  });
  return choix;
}

function blocHabillage(corps) {
  const p = E.vue.projet;
  const liste = (p.habillage || []).map((x) => ({ ...x }));
  let minuterie;
  const enregistrer = (tout_de_suite = true, message = '') => {
    // La vidéo montre l'habillage tout de suite, avant l'enregistrement.
    majHabillageDirect(liste);
    clearTimeout(minuterie);
    minuterie = setTimeout(async () => {
      try {
        const r = await api('POST', `/api/projets/${E.projetId}/habillage`, liste);
        r.habillage.forEach((x, i) => { if (liste[i]) liste[i].id = x.id; });
        // L'habillage se voit en direct sur l'aperçu : il ne demande plus de nouvel aperçu.
        if (!E.vue.projet.apercu?.rapport?.habillage_direct) E.aMettreAJour = true;
        if (message) toast(message, 'ok');
      } catch (e) { toast(e.message, 'erreur'); }
    }, tout_de_suite ? 0 : 500);
  };
  const zone = h('div');
  // Un nouveau logo part toujours d'un fichier choisi ; une image déjà dans la médiathèque se choisit ensuite
  // dans la liste du logo.
  const ajouterLogo = () => chargerMedia(ACCEPTE_MEDIA.image, async (id) => {
    liste.push({ genre: 'logo', media: id, position: 'haut_droite', taille: 0.16, opacite: 0.9 });
    await api('POST', `/api/projets/${E.projetId}/habillage`, liste);
    toast('Logo posé sur la vidéo : réglez sa place et sa taille.', 'ok');
  });
  const element = (x, i) => {
    const retirer = h('button', { class: 'btn icone petit fantome danger', title: 'Retirer cet élément',
      onclick: () => { liste.splice(i, 1); enregistrer(true, 'Élément retiré.'); dessiner(); } }, icone('corbeille'));
    const position = h('select', { class: 'champ', 'aria-label': 'Place' }, POSITIONS_HABILLAGE.map(([v, l]) => h('option', { value: v, selected: x.position === v }, l)));
    position.addEventListener('change', () => { x.position = position.value; enregistrer(); });
    let tete;
    if (x.genre === 'logo') {
      const choix = choixMedia({ genres: ['image', 'gif'], valeur: x.media, libelle: 'Image du logo',
        changer: (id) => { x.media = id; enregistrer(); dessiner(); } });
      tete = h('div', { class: 'habillage-tete' },
        h('div', { class: 'habillage-vignette' }, x.media ? h('img', { src: urlMedia(E.vue.projet.medias.find((m) => m.id === x.media) || { id: x.media }), alt: '' }) : icone('image')),
        h('div', { style: { flex: 1, minWidth: 0 } }, h('b', { class: 'petit' }, 'Logo'), choix), retirer);
    } else {
      const champ = h('input', { class: 'champ', value: x.texte || '', maxlength: '80', 'aria-label': 'Texte' });
      champ.addEventListener('input', () => { x.texte = champ.value; enregistrer(false); });
      tete = h('div', { class: 'habillage-tete' }, h('div', { class: 'habillage-vignette' }, icone('texte')),
        h('div', { style: { flex: 1, minWidth: 0 } }, h('b', { class: 'petit' }, 'Texte'), champ), retirer);
    }
    return h('div', { class: 'element-habillage' }, tete,
      h('div', { class: 'reglage' }, h('div', { class: 'reglage-titre' }, 'Place'), position),
      x.genre === 'logo'
        ? curseurElement('Taille', Math.round((x.taille || 0.16) * 100), 5, 45, 1, (v) => `${v} % de la largeur`, (v, fin) => { x.taille = v / 100; enregistrer(fin); })
        : curseurElement('Taille du texte', Math.round((x.taille || 0.035) * 1000) / 10, 2, 8, 0.5, (v) => `${fr(v)} %`, (v, fin) => { x.taille = v / 100; enregistrer(fin); }),
      curseurElement('Opacité', Math.round((x.opacite || 0.9) * 100), 10, 100, 5, (v) => `${v} %`, (v, fin) => { x.opacite = v / 100; enregistrer(fin); }),
      // Pour détacher le logo d'une image chargée : une ombre portée douce, un halo lumineux autour.
      // Pas de 1 : un petit logo se règle finement. Le halo s'affiche aussi par son rayon dans la vidéo finale (1920 px).
      x.genre === 'logo' ? curseurElement('Ombre portée', Math.round((x.ombre || 0) * 100), 0, 100, 1, (v) => (v ? `${v} %` : 'aucune'), (v, fin) => { x.ombre = v / 100; enregistrer(fin); }) : null,
      x.genre === 'logo' ? curseurElement('Halo lumineux', Math.round((x.halo || 0) * 100), 0, 100, 1,
        (v) => (v ? `${v} % · rayon ${fr(0.04 * 1920 * (x.taille || 0.16) * (0.6 + v / 100), 1)} px` : 'aucun'), (v, fin) => { x.halo = v / 100; enregistrer(fin); }) : null,
      x.genre === 'texte' ? h('div', { class: 'segments', role: 'radiogroup', 'aria-label': 'Couleur du texte', style: { marginTop: '8px' } },
        [['blanc', 'Blanc'], ['noir', 'Noir'], ['jaune', 'Jaune']].map(([v, l]) => h('button', { class: (x.couleur || 'blanc') === v ? 'actif' : '',
          onclick: () => { x.couleur = v; enregistrer(); dessiner(); } }, l))) : null);
  };
  const dessiner = () => remplir(zone,
    h('div', { class: 'reglage-aide' }, 'Un logo ou un texte posé sur toute la durée du montage (pas sur le générique) : votre marque, votre nom, votre compte. Un PNG à fond transparent rend le mieux. Les sous-titres passent au-dessus.'),
    liste.map(element),
    h('div', { style: { display: 'flex', gap: '8px', marginTop: '10px', flexWrap: 'wrap' } },
      h('button', { class: 'btn petit doux', onclick: ajouterLogo }, icone('image'), 'Ajouter un logo'),
      h('button', { class: 'btn petit doux', onclick: () => {
        liste.push({ genre: 'texte', texte: '@votrecompte', position: 'bas_droite', taille: 0.035, opacite: 0.85, couleur: 'blanc' });
        enregistrer(true, 'Texte ajouté : écrivez-le, puis choisissez sa place.');
        dessiner();
      } }, icone('texte'), 'Ajouter un texte')));
  dessiner();
  const resume = liste.length ? liste.map((x) => (x.genre === 'logo' ? 'logo' : `« ${x.texte} »`)).join(' · ') : 'Rien sur toute la durée';
  corps.append(blocOutil('habillage', { icone: 'etoile', titre: 'Habillage permanent', famille: 'image', resume }, zone));
}

/** Générique : une vidéo, ou un carton (titre sur une image), avant et après le montage. */
function blocGenerique(corps) {
  const p = E.vue.projet;
  const g = { debut: p.generique?.debut ? { ...p.generique.debut } : null, fin: p.generique?.fin ? { ...p.generique.fin } : null };
  const videos = p.medias.filter((m) => m.genre === 'video');
  let minuterie;
  const enregistrer = (tout_de_suite = true) => {
    clearTimeout(minuterie);
    minuterie = setTimeout(async () => {
      try { await api('POST', `/api/projets/${E.projetId}/generique`, g); E.aMettreAJour = true; }
      catch (e) { toast(e.message, 'erreur'); }
    }, tout_de_suite ? 0 : 500);
  };
  const zone = h('div');
  const cote = (cle, libelle) => {
    const s = g[cle];
    const genre = s ? s.genre : 'aucun';
    const choisir = (v) => {
      if (v === 'aucun') g[cle] = null;
      else if (v === 'video') {
        // Pas de vidéo dans la médiathèque : on la choisit tout de suite.
        if (!videos.length) {
          chargerMedia(ACCEPTE_MEDIA.video, async (id) => { g[cle] = { genre: 'video', media: id, duree: 0 }; await api('POST', `/api/projets/${E.projetId}/generique`, g); });
          return;
        }
        g[cle] = { genre: 'video', media: videos[0].id, duree: 0 };
      } else g[cle] = { genre: 'carton', titre: cle === 'debut' ? p.nom : 'Merci d’avoir regardé', sous_titre: '', media: '', duree: 3 };
      enregistrer();
      dessiner();
    };
    const parties = [h('div', { class: 'segments', role: 'radiogroup', 'aria-label': libelle },
      [['aucun', 'Aucun'], ['video', 'Vidéo'], ['carton', 'Titre + image']].map(([v, l]) => h('button', { class: genre === v ? 'actif' : '', onclick: () => choisir(v) }, l)))];
    if (s?.genre === 'video') {
      const choix = choixMedia({ genres: ['video'], valeur: s.media, libelle: 'Vidéo', changer: (id) => { s.media = id; enregistrer(); dessiner(); } });
      parties.push(h('div', { class: 'reglage' }, h('div', { class: 'reglage-titre' }, 'Vidéo'), choix),
        curseurElement('Durée gardée', s.duree || 0, 0, 30, 1, (v) => (v ? `${v} s au plus` : 'entière'), (v, fin) => { s.duree = v; enregistrer(fin); }));
    } else if (s?.genre === 'carton') {
      const titre = h('input', { class: 'champ', value: s.titre || '', maxlength: '80', placeholder: 'Titre', 'aria-label': 'Titre' });
      titre.addEventListener('input', () => { s.titre = titre.value; enregistrer(false); });
      const sous = h('input', { class: 'champ', value: s.sous_titre || '', maxlength: '120', placeholder: 'Sous-titre (facultatif)', 'aria-label': 'Sous-titre' });
      sous.addEventListener('input', () => { s.sous_titre = sous.value; enregistrer(false); });
      const image = choixMedia({ genres: ['image', 'gif'], valeur: s.media, vide: 'Fond sombre uni', libelle: 'Image de fond',
        changer: (id) => { s.media = id; enregistrer(); dessiner(); } });
      parties.push(h('div', { class: 'reglage' }, h('div', { class: 'reglage-titre' }, 'Titre'), titre, h('div', { style: { height: '6px' } }), sous),
        h('div', { class: 'reglage' }, h('div', { class: 'reglage-titre' }, 'Image de fond'), image),
        curseurElement('Durée', s.duree || 3, 1.5, 10, 0.5, (v) => `${fr(v)} s`, (v, fin) => { s.duree = v; enregistrer(fin); }));
    }
    return h('div', { class: 'element-habillage' }, h('b', { class: 'petit' }, libelle), ...parties);
  };
  const dessiner = () => remplir(zone,
    h('div', { class: 'reglage-aide' }, 'Une séquence avant le montage et une après : votre vidéo d’ouverture ou de fin (onglet Médias), ou un titre sur une image. Elles s’enchaînent en fondu ; les sous-titres et l’habillage ne s’y posent pas.'),
    cote('debut', 'Avant le montage'), cote('fin', 'Après le montage'));
  dessiner();
  const decrire = (s) => (!s ? 'aucun' : s.genre === 'video' ? 'vidéo' : `« ${s.titre || 'carton'} »`);
  corps.append(blocOutil('generique', { icone: 'film', titre: 'Générique : début et fin', famille: 'montage',
    resume: `Début : ${decrire(g.debut)} · Fin : ${decrire(g.fin)}` }, zone));
}

/** Choisir des fichiers sur le Google Drive relié et les télécharger dans le projet (rushs ou médias). */
function choisirDepuisDrive(cible) {
  const choisis = new Map();
  let compte = '';
  fenetre(cible === 'rushs' ? 'Choisir des vidéos sur Google Drive' : 'Choisir des médias sur Google Drive', (fermer) => {
    const zone = h('div', {}, h('p', { class: 'petit muet' }, 'Lecture du Drive…'));
    const bouton = h('button', { class: 'btn primaire', disabled: true, onclick: async () => {
      bouton.disabled = true;
      try {
        const r = await api('POST', `/api/projets/${E.projetId}/drive/importer`, { compte, cible, fichiers: [...choisis.values()] });
        toast(`${r.lances} fichier(s) en téléchargement depuis Google Drive : ${cible === 'rushs' ? 'ils apparaissent dans « Vos rushs », puis sont analysés' : 'ils arrivent dans la médiathèque'}.`, 'ok');
        fermer();
        rafraichir();
      } catch (e) { toast(e.message, 'erreur'); bouton.disabled = false; }
    } }, 'Importer');
    const maj = () => {
      bouton.disabled = !choisis.size;
      bouton.textContent = choisis.size ? `Importer ${choisis.size} fichier${choisis.size > 1 ? 's' : ''}` : 'Importer';
    };
    const aller = async (chemin) => {
      remplir(zone, h('p', { class: 'petit muet' }, 'Lecture du Drive…'));
      let r;
      try {
        r = await api('GET', `/api/drive/parcourir?compte=${encodeURIComponent(compte)}&chemin=${encodeURIComponent(chemin)}&genre=${cible}`);
      } catch (e) {
        remplir(zone, h('div', { class: 'aide', style: { background: 'var(--erreur-doux)' } }, icone('alerte'), h('span', {}, e.message)),
          h('a', { class: 'btn petit doux', href: '#/parametres', style: { marginTop: '10px' }, onclick: fermer }, icone('engrenage'), 'Ouvrir les Paramètres'));
        return;
      }
      compte = r.compte;
      const morceaux = chemin ? chemin.split('/') : [];
      const choixCompte = r.comptes.length > 1 ? h('select', { class: 'champ', style: { marginBottom: '10px' }, 'aria-label': 'Compte Google',
        onchange: (e) => { compte = e.target.value; choisis.clear(); maj(); aller(''); } },
      r.comptes.map((c) => h('option', { value: c.email, selected: c.email === compte }, `${c.nom || c.email} — ${c.email}`))) : null;
      remplir(zone, choixCompte,
        h('div', { class: 'fil-drive' }, h('button', { class: 'lien', onclick: () => aller('') }, 'Mon Drive'),
          morceaux.map((m, i) => [h('span', { class: 'muet' }, ' / '), h('button', { class: 'lien', onclick: () => aller(morceaux.slice(0, i + 1).join('/')) }, m)])),
        h('div', { class: 'liste-drive' },
          r.dossiers.map((d) => h('button', { class: 'drive-ligne', onclick: () => aller(chemin ? `${chemin}/${d}` : d) }, icone('dossier'), h('span', { class: 'drive-nom' }, d))),
          r.fichiers.map((f) => h('label', { class: 'drive-ligne' },
            h('input', { type: 'checkbox', checked: choisis.has(f.chemin), onchange: (e) => { if (e.target.checked) choisis.set(f.chemin, f); else choisis.delete(f.chemin); maj(); } }),
            icone(/\.(mov|mp4|m4v|mkv|avi|mts|hevc|webm|3gp)$/i.test(f.nom) ? 'film' : /\.(mp3|wav|m4a|aac|ogg|flac)$/i.test(f.nom) ? 'musique' : 'image'),
            h('span', { class: 'drive-nom' }, f.nom),
            h('span', { class: 'tres-petit muet' }, `${taille(f.taille)}${f.modifie ? ` · ${new Date(f.modifie).toLocaleDateString('fr-FR')}` : ''}`))),
          !r.dossiers.length && !r.fichiers.length ? h('p', { class: 'petit muet', style: { padding: '10px' } },
            cible === 'rushs' ? 'Aucune vidéo dans ce dossier.' : 'Aucun média dans ce dossier.') : null));
    };
    aller('');
    return h('div', {},
      h('p', { class: 'petit muet', style: { marginTop: 0 } }, cible === 'rushs'
        ? 'Cochez une ou plusieurs vidéos : elles sont téléchargées dans le projet en qualité d’origine, puis analysées.'
        : 'Cochez des images, vidéos ou musiques : elles entrent dans la médiathèque du projet.'),
      zone,
      h('div', { style: { display: 'flex', justifyContent: 'flex-end', gap: '8px', marginTop: '16px' } }, h('button', { class: 'btn', onclick: fermer }, 'Annuler'), bouton));
  }, { large: true });
}

/** Jeux de réglages : enregistrer ceux du projet sous un nom, en recharger un d'un clic. */
function blocJeux(corps) {
  const zone = h('div', {}, h('p', { class: 'petit muet' }, 'Chargement…'));
  const dessiner = async () => {
    let jeux;
    try {
      jeux = (await api('GET', '/api/jeux')).jeux;
      if (!catalogueLooks) catalogueLooks = await api('GET', '/api/postprod/looks');
    }
    catch (e) { remplir(zone, h('p', { class: 'petit', style: { color: 'var(--erreur)' } }, `Jeux de réglages indisponibles : ${e.message}`)); return; }
    const nom = h('input', { class: 'champ', placeholder: 'Nom, par exemple « Coach lumineux »', maxlength: '60', 'aria-label': 'Nom du jeu de réglages' });
    const enregistrer = async () => {
      if (!nom.value.trim()) { nom.focus(); return; }
      try {
        const j = await api('POST', '/api/jeux', { nom: nom.value, projet: E.projetId });
        toast(`Réglages et PostProd enregistrés sous « ${j.nom} ».`, 'ok');
        dessiner();
      } catch (e) { toast(e.message, 'erreur'); }
    };
    nom.addEventListener('keydown', (e) => { if (e.key === 'Enter') enregistrer(); });
    remplir(zone,
      h('div', { class: 'reglage-aide' }, 'Enregistrez les réglages et la PostProd de ce projet sous un nom, puis rechargez-les d’un clic sur n’importe quel projet. Les vidéos venues du Drive peuvent aussi les reprendre (Paramètres). Un nom déjà pris est mis à jour.'),
      h('div', { class: 'ligne-jeu' }, nom, h('button', { class: 'btn petit primaire', onclick: enregistrer }, icone('signet'), 'Enregistrer')),
      jeux.length
        ? h('div', { class: 'liste-jeux' }, jeux.map((j) => h('div', { class: 'jeu' },
          h('div', { class: 'jeu-infos' }, h('b', {}, j.nom),
            h('span', { class: 'tres-petit muet' }, [j.postprod?.look ? `look ${catalogueLooks?.looks.find((l) => l.cle === j.postprod.look)?.nom || j.postprod.look}` : null, `zooms ${j.reglages.zooms || 'normal'}`,
              j.reglages.timbre_voix ? `voix ${j.reglages.timbre_voix}` : null].filter(Boolean).join(' · '))),
          h('button', { class: 'btn petit doux', title: 'Appliquer ce jeu à ce projet', onclick: async () => {
            try {
              const r = await api('POST', `/api/projets/${E.projetId}/jeux/${j.id}`);
              E.aMettreAJour = true;
              toast(`« ${r.jeu} » rechargé${r.faits.length ? ` : ${r.faits.length} réglage(s) changé(s)` : ' : rien ne changeait'}. Mettez à jour l’aperçu.`, 'ok');
            } catch (e) { toast(e.message, 'erreur'); }
          } }, 'Charger'),
          h('button', { class: 'btn icone petit fantome danger', title: 'Supprimer ce jeu', onclick: async () => {
            if (!(await confirmer('Supprimer ce jeu de réglages ?', `« ${j.nom} » ne pourra plus être rechargé. Les projets qui l’ont utilisé gardent leurs réglages.`, 'Supprimer'))) return;
            try { await api('DELETE', `/api/jeux/${j.id}`); dessiner(); } catch (e) { toast(e.message, 'erreur'); }
          } }, icone('corbeille')))))
        : h('p', { class: 'tres-petit muet', style: { margin: '10px 0 0' } }, 'Aucun jeu enregistré pour l’instant.'));
  };
  dessiner();
  corps.append(blocOutil('jeux', { icone: 'signet', titre: 'Jeux de réglages', famille: 'recettes',
    resume: 'Enregistrer ces réglages, ou en recharger un' }, zone));
}

function dessinerReglages(corps) {
  const p = E.vue.projet;
  corps.append(h('div', { class: 'aide', style: { marginBottom: '12px' } }, icone('info'),
    h('span', {}, 'Ouvrez un bloc pour régler. Chaque changement est enregistré aussitôt ; cliquez ensuite sur « Mettre à jour l’aperçu ». Vous pouvez aussi simplement le demander à l’assistant.')));
  blocJeux(corps);
  sectionConsignes(corps);
  blocHabillage(corps);
  blocGenerique(corps);
  // Le son se règle dans l'onglet PostProd, avec l'image.
  for (const s of [...new Set(E.catalogue.map((r) => r.section))].filter((x) => x !== 'Son')) blocSection(corps, s);
}

/** Un bloc de réglages du catalogue (une section : Autocut, Zooms, Son…). */
function blocSection(corps, s) {
  const p = E.vue.projet;
  const [famille, ic] = SECTIONS_REGLAGES[s] || ['recettes', 'reglages'];
  corps.append(blocOutil(`reglages_${s}`, { icone: ic, titre: s, famille, resume: resumeSection(s, p.reglages) },
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

// ------------------------------------------------------------------ ambiances sonores

let catalogueAmbiances = null;
const ecoute = { audio: null, cle: null, attente: null };

/** Écoute d'essai : vingt secondes du rush traitées par le serveur ; un second clic met en pause. */
function ecouter(cle, bouton) {
  if (ecoute.cle === cle && ecoute.audio && !ecoute.audio.paused) { ecoute.audio.pause(); dessinerPanneau(); return; }
  ecoute.audio?.pause();
  E.noeuds.video?.pause();
  ecoute.cle = cle;
  ecoute.attente = cle;
  if (bouton) { bouton.disabled = true; bouton.lastChild.textContent = 'Préparation…'; }
  const a = new Audio(`/api/projets/${E.projetId}/son/essai${cle === 'actuel' ? '' : `?ambiance=${encodeURIComponent(cle)}`}`);
  ecoute.audio = a;
  a.addEventListener('playing', () => { ecoute.attente = null; dessinerPanneau(); });
  a.addEventListener('pause', () => dessinerPanneau());
  a.addEventListener('ended', () => dessinerPanneau());
  a.addEventListener('error', () => { ecoute.attente = null; toast('Écoute impossible : le rush est-il analysé ?', 'erreur'); dessinerPanneau(); });
  a.play().catch(() => {});
}

function boutonEcoute(cle, libelle) {
  const joue = ecoute.cle === cle && ecoute.audio && !ecoute.audio.paused;
  const prepare = ecoute.attente === cle;
  const b = h('button', { class: 'btn petit doux', disabled: prepare, title: 'Vingt secondes de votre rush avec ce son ; recliquez pour mettre en pause',
    onclick: (e) => { e.stopPropagation(); ecouter(cle, b); } },
  icone(joue ? 'pause' : 'lecture'), h('span', {}, prepare ? 'Préparation…' : joue ? 'Pause' : libelle));
  return b;
}

/** Niveaux d'écho de la pièce : chacun règle ensemble la porte de traîne (en direct) et le retrait de l'écho et
 *  du bruit par IA (inscrit dans l'aperçu). */
const NIVEAUX_ECHO = [
  ['aucun', 'Aucun', 'non', 'non'], ['leger', 'Léger', 'attenuer', 'non'], ['moyen', 'Moyen', 'attenuer', 'douce'],
  ['fort', 'Fort', 'attenuer', 'forte'], ['tres_fort', 'Très fort', 'forte', 'forte'],
];

async function reglerSon(changements) {
  try {
    const r = await api('POST', `/api/projets/${E.projetId}/reglages`, changements);
    if (r.faits?.length) E.aMettreAJour = true;
  } catch (e) { toast(e.message, 'erreur'); }
}

function marqueSon(genre) {
  if (genre === 'direct') return h('span', { class: 'marque', title: 'S’entend aussitôt sur l’aperçu (case « Son en direct »)' }, 'en direct');
  if (genre === 'partiel') return h('span', { class: 'marque marque-attente', title: 'Léger s’entend aussitôt ; à partir de Moyen, le retrait de l’écho est inscrit dans l’aperçu : il faut le mettre à jour' }, 'en partie au prochain aperçu');
  return h('span', { class: 'marque marque-attente', title: 'Inscrit dans l’aperçu : il faut le mettre à jour' }, 'au prochain aperçu');
}

function ligneSon(titre, aide, controle, genre, enLigne = false) {
  return h('div', { class: 'reglage' },
    h('div', { class: 'reglage-tete' }, h('span', { class: 'reglage-titre' }, titre, marqueSon(genre)), enLigne ? controle : null),
    aide ? h('div', { class: 'reglage-aide' }, aide) : null,
    enLigne ? null : controle);
}

/** Le son en deux blocs : la voix (une ambiance pour partir, puis chaque réglage une seule fois), et la musique
 *  avec le calage. Une ambiance n'est qu'un ensemble de valeurs de ces mêmes réglages. */
function blocsSon(corps) {
  const r = E.vue.projet.reglages;
  const R = (cle) => E.catalogue.find((x) => x.cle === cle);
  const ligne = (cle, genre, titre = R(cle).titre, aide = R(cle).aide) => ligneSon(titre, aide, controleReglage(R(cle), r[cle]), genre, R(cle).genre === 'interrupteur');
  const libelle = (cle, v) => R(cle).options.find((o) => o.valeur === v)?.libelle || v;

  // « Voix plus présente » et le timbre ne font qu'un choix ; « Sans traitement » éteint le traitement.
  const timbre = r.voix ? r.timbre_voix : 'aucun';
  const choixTimbre = h('select', { class: 'champ', 'aria-label': 'Timbre de la voix',
    onchange: (e) => reglerSon(e.target.value === 'aucun' ? { voix: false } : { voix: true, timbre_voix: e.target.value }) },
  [...R('timbre_voix').options.map((o) => [o.valeur, o.libelle]), ['aucun', 'Sans traitement']].map(([v, l]) => h('option', { value: v, selected: v === timbre }, l)));
  // Un seul choix d'écho : porte de traîne et retrait de l'écho mêlé à la voix, ensemble. La reconstruction du timbre
  // part de la voix débruitée à fond : seuls Fort et Très fort lui conviennent (le serveur l'impose aussi).
  const niveau = NIVEAUX_ECHO.find(([, , porte, wpe]) => porte === r.reverberation && wpe === r.dereverberation);
  const echo = h('div', { class: 'segments', role: 'radiogroup', 'aria-label': 'Retirer l’écho de la pièce' }, NIVEAUX_ECHO.map(([cle, l, porte, wpe]) =>
    h('button', { class: niveau?.[0] === cle ? 'actif' : '', role: 'radio', 'aria-checked': niveau?.[0] === cle ? 'true' : 'false',
      disabled: r.restauration && wpe !== 'forte', title: r.restauration && wpe !== 'forte' ? 'Indisponible avec « Reconstruire le timbre », qui demande l’écho retiré à fond' : '',
      onclick: () => reglerSon({ reverberation: porte, dereverberation: wpe }) }, l)));
  // Le débruiteur IA retire déjà le bruit : la réduction du bruit classique est éteinte, et le dit.
  const ia = r.dereverberation !== 'non';
  const ligneDebruitage = ligne('debruitage', 'apercu');
  if (ia) {
    ligneDebruitage.classList.add('desactive');
    ligneDebruitage.querySelectorAll('button, select, input').forEach((x) => { x.disabled = true; });
    ligneDebruitage.querySelector('.reglage-aide').textContent = 'Éteinte : l’IA qui retire l’écho (niveau Moyen et au-delà) retire déjà le bruit, et les deux ensemble rendaient la voix métallique.';
  }
  const active = ambianceActive();
  // Bruit, écho retiré par l'IA et calage sont inscrits dans l'aperçu : s'il ne les suit plus, on le dit en clair.
  const inscrit = E.vue.projet.apercu?.rapport?.son_inscrit;
  const ecarts = inscrit ? [
    inscrit.dereverberation !== r.dereverberation ? `l’écho retiré par l’IA (« ${libelle('dereverberation', inscrit.dereverberation)} » au lieu de « ${libelle('dereverberation', r.dereverberation)} »)` : null,
    inscrit.debruitage !== r.debruitage ? `la réduction du bruit (« ${libelle('debruitage', inscrit.debruitage)} » au lieu de « ${libelle('debruitage', r.debruitage)} »)` : null,
    inscrit.calage_son !== r.calage_son ? 'le calage son/image' : null,
    !!inscrit.restauration !== !!r.restauration ? (r.restauration ? 'le timbre reconstruit par l’IA' : 'la voix sans timbre reconstruit') : null,
  ].filter(Boolean) : [];
  if (ecarts.length) {
    corps.append(h('div', { class: 'aide', style: { background: 'var(--alerte-doux)', marginBottom: '10px' } }, icone('alerte'),
      h('span', {}, `L’aperçu que vous écoutez n’a pas encore ${ecarts.join(', ')} : le reste s’entend en direct, mais ceci demande « Mettre à jour l’aperçu ».`)));
  }
  corps.append(blocOutil('son_voix', { icone: 'onde', titre: 'Son de la voix', famille: 'son',
    resume: active ? `ambiance ${active.nom}` : [timbre === 'aucun' ? 'sans traitement' : libelle('timbre_voix', timbre),
      `écho retiré ${niveau ? niveau[1].toLowerCase() : 'sur mesure'}`, r.espace !== 'aucun' ? libelle('espace', r.espace).toLowerCase() : null].filter(Boolean).join(' · ') },
  h('div', { class: 'reglage-aide', style: { marginBottom: '8px' } },
    'Partez d’une ambiance : elle règle d’un coup tous les réglages ci-dessous. Écoutez d’abord vingt secondes de votre rush avec chacune, puis ajustez. Toucher un réglage fait passer le son « sur mesure ».'),
  h('div', { style: { marginBottom: '10px' } }, boutonEcoute('actuel', 'Écouter le son actuel')),
  cartesAmbiances(),
  h('h4', { class: 'titre-famille', style: { marginTop: '18px' } }, active ? `Ajuster « ${active.nom} »` : 'Ajuster'),
  ligneSon('Timbre', 'La couleur de la voix : égaliseur et compression qui lisse les écarts de volume.', choixTimbre, 'direct'),
  ligne('harmoniques', 'direct'),
  ligne('deesseur', 'direct'),
  ligneSon('Retirer l’écho de la pièce',
    'Léger : la traîne qui suit chaque phrase baisse. Moyen et Fort : une IA retire aussi l’écho et le bruit pendant que la voix parle, sans changer les mots (quelques secondes par rush sur la carte graphique, au prochain aperçu). Très fort : en plus, la traîne est coupée net, au risque de raboter la fin des mots doux.',
    [echo, niveau ? null : h('div', { class: 'tres-petit muet', style: { marginTop: '6px' } }, 'Réglage sur mesure (venu d’un jeu de réglages ou de l’assistant) : choisissez un niveau pour revenir à l’un d’eux.')], 'partiel'),
  ligneDebruitage,
  ligne('restauration', 'apercu'),
  ligne('espace', 'direct', 'Ajouter la résonance d’un lieu', 'L’inverse du retrait de l’écho : prête à la voix, déjà nettoyée, la résonance d’un lieu choisi — petite pièce, salle de conférence, grande salle, cathédrale.')));

  corps.append(blocOutil('son_musique', { icone: 'musique', titre: 'Musique et calage', famille: 'son',
    resume: [r.musique ? `${E.vue.projet.medias.find((m) => m.id === r.musique)?.nom || 'musique'} · ${r.volume_musique} %` : 'sans musique',
      r.calage_son ? `calage ${r.calage_son} ms` : null].filter(Boolean).join(' · ') },
  ligne('musique', 'apercu'), ligne('volume_musique', 'apercu'), ligne('calage_son', 'apercu')));
}

/** L'ambiance active (tous ses réglages égaux à ceux du projet), ou null. */
function ambianceActive() {
  const r = E.vue.projet.reglages;
  return catalogueAmbiances?.find((a) => Object.entries(a.reglages).every(([k, v]) => r[k] === v)) || null;
}

/** Cartes des ambiances : un clic applique d'un coup tous les réglages de son d'une ambiance. */
function cartesAmbiances() {
  if (!catalogueAmbiances) {
    api('GET', '/api/son/ambiances').then((c) => { catalogueAmbiances = c.ambiances; dessinerPanneau(); }).catch(() => {});
    return h('p', { class: 'muet petit' }, 'Chargement des ambiances…');
  }
  const active = ambianceActive();
  const familles = [...new Set(catalogueAmbiances.map((a) => a.famille))];
  return familles.map((fam) => [
      h('h4', { class: 'titre-famille' }, fam),
      h('div', { class: 'grille-looks' }, catalogueAmbiances.filter((a) => a.famille === fam).map((a) => h('div', {
        class: `look ${active?.cle === a.cle ? 'choisi' : ''}` },
      h('b', {}, a.nom), h('span', {}, a.description),
      h('div', { class: 'actions-look' }, boutonEcoute(a.cle, 'Écouter'),
        h('button', { class: 'btn petit', disabled: active?.cle === a.cle, onclick: async () => {
          try {
            await api('POST', `/api/projets/${E.projetId}/reglages`, a.reglages);
            E.aMettreAJour = true;
            const direct = E.vue.projet.apercu?.rapport?.son_direct;
            toast(direct ? `Ambiance « ${a.nom} » appliquée : elle s’entend en direct sur l’aperçu (sauf bruit et écho retiré, au prochain aperçu).`
              : `Ambiance « ${a.nom} » appliquée : mettez l’aperçu à jour pour l’entendre dans le montage.`, 'ok');
          } catch (e) { toast(e.message, 'erreur'); }
        } }, active?.cle === a.cle ? 'Appliquée ✓' : 'Appliquer'))))),
  ]);
}

const GENRES_MEDIA = { image: 'Image', gif: 'GIF animé', video: 'Vidéo', son: 'Son' };

/** Adresse d'un média, versionnée par son fichier : un fichier remplacé change d'adresse, le navigateur ne
 *  ressert pas l'ancienne image de son cache. */
function urlMedia(m) {
  return `/api/projets/${E.projetId}/medias/${m.id}${m.fichier ? `?v=${encodeURIComponent(m.fichier)}` : ''}`;
}

function vignetteMedia(m) {
  const url = urlMedia(m);
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
    h('button', { class: 'btn petit doux', style: { marginTop: '8px' }, onclick: () => choisirDepuisDrive('medias') }, icone('google'), 'Choisir sur Google Drive'),
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
        h('button', { class: 'btn petit fantome', style: { width: '100%', marginTop: '2px' }, title: 'Remplacer le fichier : partout où ce média sert, le nouveau prend sa place',
          onclick: () => chargerMedia(ACCEPTE_MEDIA[m.genre === 'gif' ? 'image' : m.genre] || '*/*', () => toast(`« ${m.nom} » remplacé partout où il sert.`, 'ok'), m.id) },
        icone('relancer'), 'Remplacer le fichier'),
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
  const composer = h('div');
  moduleTexte(composer);
  const poses = h('div');
  corps.append(
    blocOutil('textes_composer', { icone: 'texte', titre: 'Composer un texte', famille: 'texte', resume: 'Écrire, choisir l’effet, glisser sur la vidéo' }, composer),
    blocOutil('textes_poses', { icone: 'baguette', titre: 'Posés dans la vidéo', famille: 'image',
      resume: p.incrustations.length ? `${p.incrustations.length} incrustation(s)` : 'Rien pour l’instant' }, poses));
  corps = poses;
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

/** Ce qu'une incrustation pose : son texte, son effet ou le nom de son média. */
function nomIncrustation(inc) {
  if (inc.texte) return `« ${inc.texte} »`;
  if (inc.effet) return EFFETS[inc.effet]?.[0] || inc.effet;
  return E.vue.projet.medias.find((m) => m.id === inc.media)?.nom || '(média retiré)';
}

async function retirerIncrustation(inc) {
  try {
    await api('DELETE', `/api/projets/${E.projetId}/incrustations/${inc.id}`);
    E.aMettreAJour = true;
    toast(`${nomIncrustation(inc)} retiré. Mettez à jour l’aperçu pour le voir.`, 'ok');
  } catch (e) { toast(e.message, 'erreur'); }
}

/** Arrivée d'une image à l'écran : clé, nom, petite animation de l'icône dans la palette. */
const ARRIVEES = [
  ['fondu', 'Fondu', 'Apparaît en douceur'], ['aucune', 'Sans transition', 'Là d’un coup'],
  ['splash', 'Splash', 'Jaillit, dépasse, se pose'], ['zoom', 'Zoom', 'Grandit jusqu’à sa taille'],
  ['secousse', 'Secousse', 'Arrive en tremblant'], ['glisse_gauche', 'Glissé gauche', 'Entre par la gauche'],
  ['glisse_droite', 'Glissé droite', 'Entre par la droite'], ['glisse_haut', 'Glissé haut', 'Descend du haut'],
  ['glisse_bas', 'Glissé bas', 'Monte du bas'],
];

/** Ajouter une incrustation sur une phrase, ou régler une incrustation existante (`existante`). */
function dialogueIncrustation(ph, existante = null) {
  const p = E.vue.projet;
  const visuels = p.medias.filter((m) => m.genre !== 'son');
  const choix = existante
    ? { genre: existante.effet ? 'effet' : 'media', media: existante.media, effet: existante.effet || 'secousse', position: existante.x != null ? 'libre' : (existante.position || 'centre'),
        taille: Math.round((existante.taille || 0.4) * 100), duree: existante.duree, decalage: existante.decalage || 0, fond_vert: !!existante.fond_vert, son: !!existante.son,
        entree: existante.entree || 'fondu' }
    : { genre: visuels.length ? 'media' : 'effet', media: visuels[0]?.id || '', effet: 'secousse', position: 'centre', taille: 40, duree: 2, decalage: 0, fond_vert: false, son: false, entree: 'fondu' };
  const dureePhrase = Math.max(0.5, ph.t1 - ph.t0);
  fenetre(existante ? 'Régler l’incrustation' : 'Ajouter une incrustation', (fermer) => {
    const zone = h('div');
    const redessiner = () => {
      const media = p.medias.find((m) => m.id === choix.media);
      remplir(zone, 
        h('p', { class: 'petit muet', style: { marginTop: 0 } }, `Sur la phrase : « ${ph.texte} »`),
        existante ? null : h('div', { class: 'segments' },
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
        choix.genre === 'media' ? h('div', { class: 'ligne-champ' }, h('label', {}, 'Arrivée à l’écran'),
          h('div', { class: 'palette-arrivees' }, ARRIVEES.map(([v, l, d]) =>
            h('button', { class: `arrivee arrivee-${v} ${choix.entree === v ? 'actif' : ''}`, title: d, onclick: () => { choix.entree = v; redessiner(); } },
              h('span', { class: 'arrivee-scene' }, h('i')), h('b', {}, l))))) : null,
        choix.genre === 'media' ? h('div', { class: 'ligne-champ' }, h('label', {}, choix.position === 'libre' ? 'Position (posée à la main sur la vidéo — choisir une place la remplace)' : 'Position'),
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
          existante ? h('button', { class: 'btn danger', style: { marginRight: 'auto' }, onclick: async () => { await retirerIncrustation(existante); fermer(); } }, icone('corbeille'), 'Retirer') : null,
          h('button', { class: 'btn', onclick: fermer }, 'Annuler'),
          existante ? h('button', { class: 'btn primaire', onclick: async () => {
            try {
              await api('POST', `/api/projets/${E.projetId}/incrustations/${existante.id}`, {
                media: choix.genre === 'media' ? choix.media : undefined, entree: choix.genre === 'media' ? choix.entree : undefined,
                position: choix.position === 'libre' ? undefined : choix.position, taille: choix.taille / 100,
                duree: choix.duree, decalage: choix.decalage, fond_vert: choix.fond_vert, son: choix.son,
              });
              E.aMettreAJour = true;
              fermer();
              toast('Incrustation réglée. Mettez à jour l’aperçu pour la voir.', 'ok');
            } catch (e) { toast(e.message, 'erreur'); }
          } }, icone('ok'), 'Enregistrer') : h('button', { class: 'btn primaire', disabled: choix.genre === 'media' && !choix.media, onclick: async () => {
            try {
              await api('POST', `/api/projets/${E.projetId}/incrustations`, {
                media: choix.genre === 'media' ? choix.media : '', effet: choix.genre === 'effet' ? choix.effet : '',
                phrase: ph.cle, decalage: choix.decalage, duree: choix.duree, position: choix.position,
                taille: choix.taille / 100, fond_vert: choix.fond_vert, son: choix.son, entree: choix.genre === 'media' ? choix.entree : '',
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
  basculerDiscussion(true);
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
function blocCommande(texte, copie = 'Commande copiée.') {
  return h('div', { style: { display: 'flex', gap: '6px', alignItems: 'center', marginTop: '6px' } },
    h('code', { style: { flex: 1, background: 'var(--surface-2)', padding: '8px 10px', borderRadius: '8px', fontSize: '13px', overflowX: 'auto', whiteSpace: 'nowrap' } }, texte),
    h('button', { class: 'btn icone fantome', title: 'Copier', onclick: async () => { try { await navigator.clipboard.writeText(texte); toast(copie); } catch { toast('Copie impossible : sélectionnez le texte.'); } } }, icone('copier')));
}

async function afficherParametres() {
  const P = await api('GET', '/api/parametres');
  const projets = await api('GET', '/api/projets');
  const jeux = (await api('GET', '/api/jeux').catch(() => ({ jeux: [] }))).jeux;
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
    jeux.length ? h('optgroup', { label: 'Jeux de réglages' }, jeux.map((j) => h('option', { value: `jeu:${j.id}`, selected: d.modele === `jeu:${j.id}` }, j.nom))) : null,
    h('optgroup', { label: 'Comme un projet' }, projets.map((p) => h('option', { value: p.id, selected: d.modele === p.id }, p.nom))));
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
      champ('Réglages des projets venus du Drive', modele, 'Un jeu de réglages enregistré, ou un projet dont vous aimez le montage (ses réglages, sa PostProd et ses consignes sont repris).'),
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

const CONSOLE_GOOGLE = 'https://console.cloud.google.com';
const PORTEES_GOOGLE = 'https://www.googleapis.com/auth/drive, https://www.googleapis.com/auth/youtube.upload';

/** Lien vers une page de la console Google, ouvert dans un nouvel onglet. */
function lienConsole(chemin, libelle) {
  return h('a', { class: 'btn petit doux', href: `${CONSOLE_GOOGLE}${chemin}`, target: '_blank', rel: 'noopener' }, libelle, ' ↗');
}

/** Lit le fichier « client_secret_….json » téléchargé chez Google et enregistre l'identifiant. */
async function deposerIdentifiantGoogle(fichier, retour) {
  let j;
  try { j = JSON.parse(await fichier.text()); } catch { toast('Ce fichier n’est pas un fichier JSON de Google.', 'erreur'); return; }
  if (j.installed) { toast('Cet identifiant est de type « Application de bureau » : créez-en un de type « Application Web » (étape 6).', 'erreur'); return; }
  const c = j.web;
  if (!c?.client_id || !c?.client_secret) { toast('Fichier inattendu : il faut celui de l’identifiant « Application Web » (client_secret_….json).', 'erreur'); return; }
  try {
    await api('POST', '/api/parametres', { google: { client_id: c.client_id, client_secret: c.client_secret } });
  } catch (e) { toast(e.message, 'erreur'); return; }
  const bonRetour = (c.redirect_uris || []).includes(retour);
  toast(bonRetour ? 'Identifiant Google enregistré. Essayez maintenant « Se connecter avec Google ».'
    : `Identifiant enregistré, mais l’adresse de retour ${retour} n’y figure pas : ajoutez-la dans la console (étape 6), sinon Google refusera la connexion.`,
  bonRetour ? 'ok' : 'erreur');
  afficherParametres();
}

/** Carte simple : se connecter avec son compte Google, voir ce qui est relié. Puis l'activation, pour la
 *  personne qui gère EDIVID, une seule fois. */
function sectionGoogle(P) {
  const g = P.google;
  const comptes = P.comptes || [];
  const moi = E.session?.compte ? comptes.find((c) => c.email === E.session.compte.email) || E.session.compte : null;
  const avantages = h('ul', { class: 'avantages' },
    h('li', {}, icone('telephone'), h('span', {}, 'Filmez à l’iPhone et déposez la vidéo dans votre Google Drive : EDIVID la monte tout seul.')),
    h('li', {}, icone('nuage'), h('span', {}, 'Le montage terminé revient dans votre Drive, et un courriel vous prévient si vous le souhaitez.')),
    h('li', {}, icone('youtube'), h('span', {}, 'Publiez sur votre chaîne YouTube depuis « Exporter ».')));
  const pastilles = (c) => h('div', { class: 'tres-petit', style: { marginTop: '4px', display: 'flex', gap: '6px', flexWrap: 'wrap' } },
    c.distant ? h('span', { class: 'pastille ok' }, 'Drive relié') : h('span', { class: 'pastille alerte' }, 'Drive non relié'),
    c.chaine ? h('span', { class: 'pastille ok' }, `YouTube : ${c.chaine}`) : null);
  const reautoriser = (c) => `/api/google/connexion?mode=liaison&compte=${encodeURIComponent(c.email)}`;
  const ligneCompte = (c) => h('div', { class: 'incrustation' },
    h('div', { style: { minWidth: 0 } }, h('b', { class: 'petit' }, c.nom || c.email), h('div', { class: 'tres-petit muet' }, c.email), pastilles(c)),
    h('div', { style: { display: 'flex', alignItems: 'center', gap: '10px' } },
      h('a', { class: 'btn petit doux', href: reautoriser(c), title: 'Refaire l’autorisation de ce compte chez Google (Drive et YouTube)' }, icone('relancer'), 'Reconnecter'),
      c.distant ? h('label', { class: 'bascule', title: 'Surveiller le dossier d’entrée de ce Drive' },
        h('input', { type: 'checkbox', checked: c.veille, onchange: async (e) => {
          try { await api('POST', '/api/comptes', { email: c.email, veille: e.target.checked }); toast(e.target.checked ? 'Veille de ce Drive activée.' : 'Veille de ce Drive arrêtée.', 'ok'); }
          catch (err) { toast(err.message, 'erreur'); e.target.checked = !e.target.checked; }
        } }), h('span', {}, 'Veille')) : null,
      h('button', { class: 'btn petit fantome danger', onclick: async () => {
        if (!(await confirmer('Retirer ce compte ?', `${c.email} ne pourra plus se connecter et son Drive sera délié.`, 'Retirer'))) return;
        await api('DELETE', `/api/comptes/${encodeURIComponent(c.email)}`);
        afficherParametres();
      } }, 'Retirer')));
  const carte = h('div', { class: 'carte carte-corps carte-google', style: { marginTop: '16px' } },
    h('h2', { style: { display: 'flex', gap: '8px', alignItems: 'center' } }, icone('google'), 'Votre compte Google'));
  if (!g.configure) {
    carte.append(avantages,
      h('div', { class: 'aide', style: { background: 'var(--alerte-doux)', marginTop: '14px' } }, icone('info'),
        h('span', {}, 'Pas encore disponible ici : la connexion avec Google s’active une seule fois, par la personne qui gère EDIVID (une quinzaine de minutes dans un navigateur, sans rien installer). Ensuite, chacun se connecte en un clic.')),
      h('button', { class: 'btn primaire', style: { marginTop: '12px' }, onclick: () => {
        const a = document.getElementById('activation-google');
        if (a) { a.open = true; a.scrollIntoView({ behavior: 'smooth', block: 'start' }); }
      } }, icone('engrenage'), 'Activer la connexion Google'));
  } else {
    if (moi) {
      carte.append(h('div', { class: 'compte-connecte' },
        h('span', { class: 'avatar grand' }, (moi.nom || moi.email).trim()[0]?.toUpperCase() || '?'),
        h('div', { style: { minWidth: 0 } }, h('b', {}, moi.nom || moi.email), h('div', { class: 'tres-petit muet' }, moi.email), pastilles(moi))));
      // Drive ou YouTube pas accordé : on le dit, avec le bouton qui refait l'autorisation de ce compte.
      if (!moi.distant || !moi.chaine) {
        carte.append(h('div', { class: 'aide', style: { background: 'var(--alerte-doux)', marginTop: '12px' } }, icone('alerte'),
          h('span', {}, `Google n’a pas encore accordé ${!moi.distant && !moi.chaine ? 'le Drive ni YouTube' : !moi.distant ? 'le Drive' : 'YouTube'} à EDIVID. `
            + 'Cliquez ci-dessous ; sur l’écran de Google, cochez les cases « Google Drive » et « YouTube » (elles peuvent être décochées), puis Continuer.')),
        h('a', { class: 'btn primaire', href: reautoriser(moi), style: { marginTop: '10px' } }, icone('google'), 'Autoriser le Drive et YouTube'));
      }
    } else {
      carte.append(avantages);
    }
    carte.append(
      h('a', { class: 'btn bouton-google grand', href: '/api/google/connexion?mode=liaison', style: { marginTop: '14px' } }, icone('google'),
        moi ? 'Relier un autre compte Google' : 'Se connecter avec Google'),
      h('p', { class: 'tres-petit muet' }, 'Google vous demande d’autoriser EDIVID à utiliser votre Drive et à publier sur YouTube : cochez les deux. Si un écran « Google n’a pas validé cette application » apparaît, cliquez « Continuer » (ou « Paramètres avancés », puis « Accéder à EDIVID »).'),
      comptes.length ? h('details', { style: { marginTop: '10px' }, open: comptes.length > 0 && !moi },
        h('summary', { style: { cursor: 'pointer', fontWeight: 600 } }, `Comptes reliés à cet EDIVID (${comptes.length})`),
        comptes.map(ligneCompte)) : null);
  }
  return [carte, carteActivationGoogle(P)];
}

/** Activation de la connexion Google : pour la personne qui gère EDIVID, une seule fois. Chaque étape ouvre
 *  la bonne page de la console et donne les valeurs à copier ; le fichier téléchargé se dépose ici. */
function carteActivationGoogle(P) {
  const g = P.google;
  const https = (P.url_publique || '').startsWith('https://');
  const retour = `${(P.url_publique || '').replace(/\/$/, '')}/api/google/retour`;
  const entreeG = (cle, attrs = {}) => {
    const el = h('input', { class: 'champ', value: g[cle] ?? '', ...attrs });
    el.addEventListener('input', () => { g[cle] = el.value; });
    return el;
  };
  const fichier = h('input', { type: 'file', accept: '.json,application/json', class: 'cache',
    onchange: (e) => { if (e.target.files[0]) deposerIdentifiantGoogle(e.target.files[0], retour); } });
  const depot = h('div', { class: 'depot', tabindex: '0', role: 'button', style: { marginTop: '8px', padding: '18px' }, onclick: () => fichier.click(),
    onkeydown: (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); fichier.click(); } },
    ondragover: (e) => { e.preventDefault(); depot.classList.add('survol'); },
    ondragleave: () => depot.classList.remove('survol'),
    ondrop: (e) => { e.preventDefault(); depot.classList.remove('survol'); if (e.dataTransfer.files[0]) deposerIdentifiantGoogle(e.dataTransfer.files[0], retour); } },
  icone('televerser'), h('strong', {}, 'Déposez ici le fichier téléchargé'), h('span', { class: 'petit' }, 'client_secret_….json — ou cliquez pour le choisir'), fichier);
  const etapes = [
    !https ? ['Ouvrir EDIVID en HTTPS', h('div', {},
      h('span', { class: 'petit muet' }, 'Google n’accepte qu’une adresse en HTTPS. Dans la console Tailscale (DNS), activez « HTTPS Certificates », puis sur azoth :'),
      blocCommande('sudo tailscale serve --bg 8030'),
      h('div', { class: 'tres-petit', style: { color: 'var(--alerte)', marginTop: '4px' } }, `Adresse actuelle : ${P.url_publique} — elle doit commencer par https:// (« Lien des courriels », plus bas).`))] : null,
    ['Créer le projet', h('div', {},
      h('span', { class: 'petit muet' }, 'Connectez-vous avec le compte Google qui gérera l’application, puis créez un projet nommé :'),
      blocCommande('EDIVID', 'Nom copié.'), h('div', { style: { marginTop: '8px' } }, lienConsole('/projectcreate', 'Créer le projet')))],
    ['Activer Drive et YouTube', h('div', {},
      h('span', { class: 'petit muet' }, 'Vérifiez que le projet EDIVID est choisi en haut de la page, puis cliquez « Activer ».'),
      h('div', { style: { marginTop: '8px' } }, lienConsole('/flows/enableapi?apiid=drive.googleapis.com,youtube.googleapis.com', 'Activer les deux API')))],
    ['Présenter l’application', h('div', {},
      h('span', { class: 'petit muet' }, '« Commencer » : nom de l’application EDIVID, votre adresse d’assistance, audience « Externe », votre adresse de contact, puis « Créer ».'),
      h('div', { style: { marginTop: '8px' } }, lienConsole('/auth/branding', 'Ouvrir « Branding »')))],
    ['Autoriser Drive et YouTube', h('div', {},
      h('span', { class: 'petit muet' }, '« Ajouter ou supprimer des champs d’application », puis en bas « Ajouter manuellement » : collez ces deux champs, « Ajouter au tableau », « Mettre à jour », puis « Enregistrer ».'),
      blocCommande(PORTEES_GOOGLE, 'Champs copiés.'), h('div', { style: { marginTop: '8px' } }, lienConsole('/auth/scopes', 'Ouvrir « Accès aux données »')))],
    ['Ajouter les personnes', h('div', {},
      h('span', { class: 'petit muet' }, '« Utilisateurs test » → « Add users » : l’adresse Gmail de chaque personne qui se connectera (la vôtre d’abord).'),
      h('div', { style: { marginTop: '8px' } }, lienConsole('/auth/audience', 'Ouvrir « Audience »')))],
    ['Créer l’identifiant', h('div', {},
      h('span', { class: 'petit muet' }, '« Créer un client » : type « Application Web », nom EDIVID ; sous « URI de redirection autorisés », « Ajouter un URI » et collez :'),
      blocCommande(retour, 'Adresse copiée.'),
      h('span', { class: 'petit muet' }, '« Créer », puis dans la fenêtre qui s’ouvre : « Télécharger le fichier JSON ».'),
      h('div', { style: { marginTop: '8px' } }, lienConsole('/auth/clients/create', 'Créer le client')))],
    ['Déposer le fichier', h('div', {}, depot,
      h('details', { style: { marginTop: '8px' } }, h('summary', { class: 'petit', style: { cursor: 'pointer' } }, 'Ou saisir l’identifiant à la main'),
        champ('ID client', entreeG('client_id', { placeholder: '…apps.googleusercontent.com' })),
        champ('Code secret du client', entreeG('client_secret', { type: 'password', autocomplete: 'new-password', placeholder: g.secret_defini ? '•••••••• (enregistré — laissez vide pour le garder)' : '' })),
        h('button', { class: 'btn primaire', style: { marginTop: '10px' }, onclick: async () => {
          try { await api('POST', '/api/parametres', { google: { client_id: g.client_id, client_secret: g.client_secret } }); toast('Identifiant Google enregistré.', 'ok'); afficherParametres(); }
          catch (e) { toast(e.message, 'erreur'); }
        } }, 'Enregistrer')))],
    ['Essayer, puis passer en production', h('div', {},
      h('span', { class: 'petit muet' }, 'Cliquez « Se connecter avec Google » dans la carte du dessus. Une fois l’essai réussi, dans « Audience », « Publier l’application » : sinon Google coupe la liaison au bout de 7 jours.'),
      h('div', { style: { marginTop: '8px' } }, lienConsole('/auth/audience', 'Publier l’application')))],
    ['Lever le verrou YouTube (facultatif)', h('span', { class: 'petit muet' },
      'Tant que l’application n’a pas passé l’audit de l’API YouTube, Google garde en privé les vidéos envoyées par EDIVID. La demande se fait par le formulaire « YouTube API Services – Audit and Quota Extension ». Rien n’assure qu’une vidéo déjà verrouillée soit libérée ensuite : en attendant, téléchargez l’export YouTube et envoyez-le à la main depuis YouTube Studio.')],
  ].filter(Boolean);
  return h('details', { class: 'carte carte-corps', id: 'activation-google', style: { marginTop: '16px' } },
    h('summary', { style: { cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '8px' } }, icone('engrenage'),
      h('b', {}, 'Activer la connexion Google'),
      h('span', { class: 'tres-petit muet' }, g.configure ? '— déjà activée' : '— une seule fois, par la personne qui gère EDIVID')),
    h('p', { class: 'petit muet' }, 'Google exige que chaque service qui accède au Drive ou à YouTube soit déclaré dans sa console. Chaque étape ouvre la bonne page dans un nouvel onglet ; revenez ici entre deux.'),
    h('div', { class: 'guide-liste' }, etapes.map(([titre, x], i) =>
      h('div', { class: 'guide-item' }, h('div', { class: 'num' }, i + 1), h('div', {}, h('h3', {}, titre), x)))));
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

/** Valeur signée pour un résumé : « +6 », « -3 », « 0 ». */
function signe(v) { return `${v > 0 ? '+' : ''}${v}`; }

function dessinerPostProd(corps) {
  const p = E.vue.projet;
  const pp = E.postprod = { ...p.postprod };
  E.postprodDe = E.projetId;
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
        E.vue.projet.postprod = { ...E.postprod };
        if (E.comparaison) comparer();
      } catch (e) { toast(e.message, 'erreur'); }
    }, 450);
    // La vidéo suit tout de suite, sans attendre l'enregistrement.
    majApercuDirect();
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
      h('span', {}, 'Le traitement de l’image et du son sur toute la vidéo. L’image se règle en direct : chaque look, chaque curseur se voit aussitôt sur la vidéo du lecteur (case « PostProd en direct » pour comparer avec l’image brute). Le rendu final l’inscrit dans l’image, sous les textes et sous-titres, qui restent nets.')),
    h('h3', { class: 'titre-pp' }, icone('image'), 'Image'),
    blocOutil('pp_looks', { icone: 'etincelles', titre: 'Looks', famille: 'image',
      resume: pp.look ? (catalogueLooks.looks.find((l) => l.cle === pp.look)?.nom || pp.look) : 'Réglage sur mesure' },
      h('div', { class: 'grille-looks' }, catalogueLooks.looks.map((l) => h('button', {
        class: `look ${pp.look === l.cle ? 'choisi' : ''}`, title: l.description,
        onclick: async () => {
          Object.assign(E.postprod, l.reglages, { look: l.cle });
          try {
            await api('POST', `/api/projets/${E.projetId}/postprod`, E.postprod);
            E.vue.projet.postprod = { ...E.postprod };
            toast(`Look « ${l.nom} » appliqué : il se voit en direct sur la vidéo.`, 'ok');
            dessinerPanneau();
            majApercuDirect();
            if (E.comparaison) comparer();
          } catch (e) { toast(e.message, 'erreur'); }
        },
      }, h('b', {}, l.nom), h('span', {}, l.description))))),
    blocOutil('pp_comparer', { icone: 'image', titre: 'Avant / après', famille: 'recettes', resume: 'Comparer sur l’image affichée dans le lecteur' },
      h('div', { class: 'reglage-aide' }, 'Sur l’image d’origine, à l’instant affiché dans le lecteur (mettez en pause là où le visage est bien visible).'),
      h('button', { class: 'btn petit doux', style: { marginTop: '8px' }, onclick: () => { E.comparaison = true; comparer(); } }, icone('image'), 'Comparer à l’instant affiché'),
      E.noeuds.comparaison),
    blocOutil('pp_couleur', { icone: 'palette', titre: 'Couleur', famille: 'cadre',
      resume: [catalogueLooks.couleurs.find((c) => c.cle === pp.couleur)?.nom, `expo ${signe(pp.exposition)}`, `contraste ${signe(pp.contraste)}`, `temp. ${signe(pp.temperature)}`].join(' · ') },
      h('div', { class: 'reglage' }, h('div', { class: 'reglage-titre' }, 'Colorimétrie'), h('div', { class: 'reglage-aide' }, 'L’ambiance générale des couleurs.'), couleur),
      curseur('exposition', 'Exposition', 'Plus clair ou plus sombre.', -100, 100),
      curseur('contraste', 'Contraste', null, -100, 100),
      curseur('saturation', 'Saturation', 'Intensité des couleurs.', -100, 100),
      curseur('temperature', 'Température', 'Vers le bleu (froid) ou l’ambre (chaud).', -100, 100)),
    blocOutil('pp_peau', { icone: 'peau', titre: 'Peau et lumière', famille: 'montage',
      resume: `lissage ${pp.lissage} · glow ${pp.diffusion} · netteté ${pp.nettete}` },
      curseur('lissage', 'Lissage du teint', 'Adoucit la peau et gomme les petites imperfections, sans flouter les yeux ni les contours.', 0, 100),
      curseur('diffusion', 'Diffusion (glow)', 'Un halo doux dans les lumières, comme un filtre « Pro-Mist » : peau lumineuse, image rêveuse.', 0, 100),
      curseur('nettete', 'Netteté', 'Relève les détails (cheveux, regard). À doser avec le lissage.', 0, 100)),
    blocOutil('pp_texture', { icone: 'grain', titre: 'Texture et cadre', famille: 'son',
      resume: `grain ${pp.grain} · vignette ${pp.vignette}${pp.bandes ? ' · bandes cinéma' : ''}` },
      curseur('grain', 'Grain', 'Une texture de pellicule fine et vivante.', 0, 100),
      curseur('vignette', 'Vignette', 'Assombrit doucement les bords pour guider le regard au centre.', 0, 100),
      h('div', { class: 'reglage' }, h('div', { class: 'reglage-tete' }, h('span', { class: 'reglage-titre' }, 'Bandes cinéma'),
        interrupteurSimple(pp.bandes, (v) => { pp.bandes = v; pp.look = ''; enregistrer(); })),
        h('div', { class: 'reglage-aide' }, horizontal ? 'Bandes noires au format 2,39:1, comme au cinéma.' : 'Pour les vidéos horizontales seulement : sans effet en vertical.'))),
    h('button', { class: 'btn petit fantome', onclick: async () => {
      E.postprod = { look: '', couleur: 'naturel', exposition: 0, contraste: 0, saturation: 0, temperature: 0, lissage: 0, diffusion: 0, nettete: 0, grain: 0, vignette: 0, bandes: false };
      await api('POST', `/api/projets/${E.projetId}/postprod`, E.postprod);
      E.vue.projet.postprod = { ...E.postprod };
      dessinerPanneau();
      majApercuDirect();
    } }, icone('relancer'), 'Remettre l’image à zéro'),
    h('h3', { class: 'titre-pp' }, icone('onde'), 'Son'),
    h('div', { class: 'reglage-aide', style: { marginBottom: '10px' } }, p.apercu?.rapport?.son_direct
      ? 'Le son se règle en direct sur l’aperçu : appliquez une ambiance ou changez un réglage marqué « en direct », il s’entend aussitôt (case « Son en direct » sous le lecteur). La réduction du bruit, le retrait de l’écho et le calage demandent un nouvel aperçu.'
      : 'Mettez l’aperçu à jour une fois : ensuite, le son se règlera en direct pendant la lecture. En attendant, écoutez une ambiance sur vingt secondes de votre rush.'));
  blocsSon(corps);
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
