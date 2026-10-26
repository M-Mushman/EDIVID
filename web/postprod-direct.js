// PostProd en direct : l'image du lecteur est redessinée par WebGL avec les réglages en cours, à chaque image
// de la vidéo. Même chaîne que le rendu final (src/postprod.rs), dans le même ordre :
//   couleur (table calculée par ffmpeg : couleurs exactes) → lissage du teint (flou bilatéral) → diffusion
//   (halo en mode « écran ») → netteté (masque flou, luminance) → grain → vignette (cos⁴) → bandes cinéma.
// Les traitements spatiaux sont des approximations fidèles des filtres ffmpeg, pas leur copie bit à bit.

const SOMMETS = `#version 300 es
out vec2 uv;
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  uv = p;
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`;

const ENTETE = `#version 300 es
precision highp float;
in vec2 uv;
out vec4 sortie;
uniform sampler2D image;
uniform vec2 pixel;
float luma(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }
// YUV « vidéo » (BT.601, plage limitée), comme les images que reçoivent les filtres ffmpeg du rendu.
vec3 versYuv(vec3 c) {
  return (vec3(16.0, 128.0, 128.0) + mat3(65.481, -37.797, 112.0, 128.553, -74.203, -93.786, 24.966, 112.0, -18.214) * c) / 255.0;
}
vec3 versRgb(vec3 y) {
  vec3 v = y * 255.0 - vec3(16.0, 128.0, 128.0);
  return vec3(1.164 * v.x + 1.596 * v.z, 1.164 * v.x - 0.392 * v.y - 0.813 * v.z, 1.164 * v.x + 2.017 * v.y) / 255.0;
}
`;

// Table de couleurs : 64 tuiles de 64 × 64 (une par niveau de bleu), rangées 8 × 8 dans une texture 512 × 512.
const COULEUR = ENTETE + `
uniform sampler2D table;
uniform float avecTable;
vec3 tuile(float b, vec2 rg) {
  vec2 coin = vec2(mod(b, 8.0), floor(b / 8.0)) * 64.0;
  return texture(table, (coin + rg * 63.0 + 0.5) / 512.0).rgb;
}
void main() {
  vec3 c = texture(image, vec2(uv.x, 1.0 - uv.y)).rgb;
  if (avecTable > 0.5) {
    float b = c.b * 63.0;
    float b0 = floor(b);
    c = mix(tuile(b0, c.rg), tuile(min(b0 + 1.0, 63.0), c.rg), b - b0);
  }
  sortie = vec4(c, 1.0);
}`;

// Lissage du teint : flou bilatéral (les contours, très différents en couleur, ne se mélangent pas).
const LISSAGE = ENTETE + `
uniform float sigmaS;
uniform float sigmaR;
uniform float opacite;
void main() {
  vec3 c0 = texture(image, uv).rgb;
  vec3 somme = vec3(0.0);
  float poids = 0.0;
  for (int i = -3; i <= 3; i++) {
    for (int j = -3; j <= 3; j++) {
      vec2 d = vec2(float(i), float(j)) * sigmaS * 0.6;
      vec3 c = texture(image, uv + d * pixel).rgb;
      vec3 e = c - c0;
      float w = exp(-dot(d, d) / (2.0 * sigmaS * sigmaS)) * exp(-dot(e, e) / (2.0 * sigmaR * sigmaR));
      somme += c * w;
      poids += w;
    }
  }
  sortie = vec4(mix(c0, somme / poids, opacite), 1.0);
}`;

// Flou gaussien séparable (un sens par passe), pour le halo de la diffusion.
const FLOU = ENTETE + `
uniform vec2 sens;
uniform float sigma;
void main() {
  vec3 somme = vec3(0.0);
  float poids = 0.0;
  for (int i = -6; i <= 6; i++) {
    float x = float(i) / 3.0;
    float w = exp(-0.5 * x * x);
    somme += texture(image, uv + sens * pixel * x * sigma).rgb * w;
    poids += w;
  }
  sortie = vec4(somme / poids, 1.0);
}`;

const FINAL = ENTETE + `
uniform sampler2D halo;
uniform float diffusion;
uniform float nettete;
uniform float grain;
uniform float graine;
uniform float vignette;
uniform float bandes;
uniform vec2 taille;
float hasard(vec2 p) { return fract(sin(dot(p + graine, vec2(12.9898, 78.233))) * 43758.5453); }
void main() {
  vec3 c = texture(image, uv).rgb;
  if (diffusion > 0.0) {
    // ffmpeg fond le halo en mode « écran » plan par plan, en YUV : la couleur du halo se décale comme au rendu.
    vec3 a = versYuv(c);
    vec3 g = versYuv(texture(halo, uv).rgb);
    c = versRgb(mix(a, 1.0 - (1.0 - a) * (1.0 - g), diffusion));
  }
  if (nettete > 0.0) {
    float flou = 0.0;
    for (int i = -2; i <= 2; i++) for (int j = -2; j <= 2; j++) flou += luma(texture(image, uv + vec2(float(i), float(j)) * pixel).rgb);
    c += nettete * (luma(c) - flou / 25.0);
  }
  // Grain sur la luminance seule, comme « noise=c0s » (en YUV, un pas de Y vaut 1,164 pas en RGB).
  if (grain > 0.0) c += (hasard(floor(uv * taille)) * 2.0 - 1.0) * grain * 1.164;
  if (vignette > 0.0) {
    float d = length((uv - 0.5) * taille) / length(taille * 0.5);
    float k = cos(vignette * min(d, 1.0));
    k = k * k * k * k;
    // ffmpeg multiplie Y (avec son noir à 16) et rapproche la couleur du neutre : le noir s'assombrit aussi.
    vec3 y = versYuv(c);
    c = versRgb(vec3(y.x * k, (y.yz - 128.0 / 255.0) * k + 128.0 / 255.0));
  }
  if (bandes > 0.0 && (uv.y * taille.y < bandes || uv.y * taille.y > taille.y - bandes)) c = vec3(0.0);
  sortie = vec4(clamp(c, 0.0, 1.0), 1.0);
}`;

function programme(gl, source) {
  const p = gl.createProgram();
  for (const [genre, texte] of [[gl.VERTEX_SHADER, SOMMETS], [gl.FRAGMENT_SHADER, source]]) {
    const s = gl.createShader(genre);
    gl.shaderSource(s, texte);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    gl.attachShader(p, s);
  }
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  const u = {};
  const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
  for (let i = 0; i < n; i++) { const nom = gl.getActiveUniform(p, i).name; u[nom] = gl.getUniformLocation(p, nom); }
  return { p, u };
}

function texture(gl) {
  const t = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, t);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  return t;
}

/** Hald CLUT de ffmpeg (rouge le plus rapide, puis vert, puis bleu, sur 512 de large) → 64 tuiles 8 × 8. */
function enTuiles(img) {
  const c = document.createElement('canvas');
  c.width = 512; c.height = 512;
  const g = c.getContext('2d');
  g.drawImage(img, 0, 0);
  const src = g.getImageData(0, 0, 512, 512).data;
  const dst = new Uint8Array(512 * 512 * 4);
  for (let b = 0; b < 64; b++) {
    const tx = (b % 8) * 64;
    const ty = Math.floor(b / 8) * 64;
    for (let v = 0; v < 64; v++) {
      for (let r = 0; r < 64; r++) {
        const i = (r + v * 64 + b * 4096) * 4;
        const o = ((ty + v) * 512 + tx + r) * 4;
        dst[o] = src[i]; dst[o + 1] = src[i + 1]; dst[o + 2] = src[i + 2]; dst[o + 3] = 255;
      }
    }
  }
  return dst;
}

/** Aperçu en direct sur `video`, dessiné dans un canevas posé par-dessus, dans `conteneur`. Null sans WebGL 2. */
export function creerApercuDirect(video, conteneur) {
  const canevas = document.createElement('canvas');
  canevas.className = 'pp-direct cache';
  const gl = canevas.getContext('webgl2', { premultipliedAlpha: false, antialias: false });
  if (!gl) return null;
  let progs;
  try {
    progs = { couleur: programme(gl, COULEUR), lissage: programme(gl, LISSAGE), flou: programme(gl, FLOU), final: programme(gl, FINAL) };
  } catch (e) {
    console.warn('PostProd en direct indisponible :', e);
    return null;
  }
  video.after(canevas);
  gl.bindVertexArray(gl.createVertexArray());
  const tVideo = texture(gl);
  const tTable = texture(gl);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([0, 0, 0, 255]));
  const cibles = {};
  const cible = (nom, l, h) => {
    let c = cibles[nom];
    if (!c || c.l !== l || c.h !== h) {
      const t = c?.t || texture(gl);
      gl.bindTexture(gl.TEXTURE_2D, t);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, l, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
      const f = c?.f || gl.createFramebuffer();
      gl.bindFramebuffer(gl.FRAMEBUFFER, f);
      gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, t, 0);
      c = cibles[nom] = { t, f, l, h };
    }
    return c;
  };
  let pp = null;
  let actif = false;
  let tableUrl = '';
  let avecTable = false;
  let rappel = null;

  const passe = (prog, vers, entree, l, h, uniformes) => {
    gl.useProgram(prog.p);
    gl.bindFramebuffer(gl.FRAMEBUFFER, vers ? vers.f : null);
    gl.viewport(0, 0, l, h);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, entree);
    gl.uniform1i(prog.u.image, 0);
    gl.uniform2f(prog.u.pixel, 1 / l, 1 / h);
    uniformes?.(prog.u);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  };

  /** Place le canevas exactement sur l'image (la vidéo est en « contain ») ; laisse voir les commandes. */
  const placer = () => {
    const lv = video.videoWidth;
    const hv = video.videoHeight;
    const lc = conteneur.clientWidth;
    const hc = conteneur.clientHeight;
    if (!lv || !hv || !lc || !hc) return false;
    const k = Math.min(lc / lv, hc / hv);
    const l = lv * k;
    const h = hv * k;
    Object.assign(canevas.style, { width: `${l}px`, height: `${h}px`, left: `${(lc - l) / 2}px`, top: `${(hc - h) / 2}px` });
    // Les commandes du lecteur occupent le bas de la vidéo : on ne les recouvre pas quand elles sont visibles.
    canevas.style.setProperty('--clip-bas', `${Math.max(0, 52 - (hc - h) / 2)}px`);
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const lp = Math.max(2, Math.round(Math.min(lv, l * dpr)));
    const hp = Math.max(2, Math.round(lp * hv / lv));
    if (canevas.width !== lp || canevas.height !== hp) { canevas.width = lp; canevas.height = hp; }
    return true;
  };

  const dessiner = () => {
    if (!actif || !pp || video.readyState < 2 || !placer()) return;
    const l = canevas.width;
    const h = canevas.height;
    const echelle = Math.min(l, h) / 1080;
    gl.bindTexture(gl.TEXTURE_2D, tVideo);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, video);
    const a = cible('a', l, h);
    passe(progs.couleur, a, tVideo, l, h, (u) => {
      gl.activeTexture(gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, tTable);
      gl.uniform1i(u.table, 1);
      gl.uniform1f(u.avecTable, avecTable ? 1 : 0);
    });
    let courant = a;
    if (pp.lissage > 0) {
      const b = cible('b', l, h);
      passe(progs.lissage, b, a.t, l, h, (u) => {
        gl.uniform1f(u.sigmaS, (4 + 6 * pp.lissage / 100) * echelle);
        gl.uniform1f(u.sigmaR, 0.06 + 0.08 * pp.lissage / 100);
        gl.uniform1f(u.opacite, 0.85 * pp.lissage / 100);
      });
      courant = b;
    }
    let halo = null;
    if (pp.diffusion > 0) {
      // Le halo se calcule au quart de la définition : il est très flou, la différence ne se voit pas.
      const lq = Math.max(2, Math.round(l / 4));
      const hq = Math.max(2, Math.round(h / 4));
      const c1 = cible('c1', lq, hq);
      const c2 = cible('c2', lq, hq);
      const sigma = (14 * echelle) / 4;
      passe(progs.flou, c1, courant.t, lq, hq, (u) => { gl.uniform2f(u.sens, 1, 0); gl.uniform1f(u.sigma, sigma); });
      passe(progs.flou, c2, c1.t, lq, hq, (u) => { gl.uniform2f(u.sens, 0, 1); gl.uniform1f(u.sigma, sigma); });
      halo = c2.t;
    }
    passe(progs.final, null, courant.t, l, h, (u) => {
      gl.activeTexture(gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, halo || courant.t);
      gl.uniform1i(u.halo, 1);
      gl.uniform1f(u.diffusion, halo ? 0.35 * pp.diffusion / 100 : 0);
      gl.uniform1f(u.nettete, pp.nettete / 100 * 1.2);
      gl.uniform1f(u.grain, (pp.grain / 100 * 22) / 255);
      gl.uniform1f(u.graine, video.paused ? 0 : (performance.now() % 1000));
      gl.uniform1f(u.vignette, pp.vignette > 0 ? 0.25 + 0.55 * pp.vignette / 100 : 0);
      gl.uniform1f(u.bandes, pp.bandes && video.videoWidth > video.videoHeight ? Math.max(0, (h - l / 2.39) / 2) : 0);
      gl.uniform2f(u.taille, l, h);
    });
  };

  const boucle = () => {
    dessiner();
    if (actif && 'requestVideoFrameCallback' in video) rappel = video.requestVideoFrameCallback(boucle);
    else if (actif) rappel = requestAnimationFrame(boucle);
  };
  for (const ev of ['seeked', 'loadeddata', 'pause', 'play']) video.addEventListener(ev, dessiner);
  new ResizeObserver(dessiner).observe(conteneur);

  const chargerTable = (url) => {
    if (url === tableUrl) return;
    tableUrl = url;
    if (!url) { avecTable = false; dessiner(); return; }
    const img = new Image();
    img.onload = () => {
      if (url !== tableUrl) return;
      gl.bindTexture(gl.TEXTURE_2D, tTable);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, 512, 512, 0, gl.RGBA, gl.UNSIGNED_BYTE, enTuiles(img));
      avecTable = true;
      dessiner();
    };
    img.src = url;
  };

  return {
    /** Réglages PostProd à montrer, et adresse de leur table de couleurs ('' si la couleur est neutre). */
    regler(reglages, url) {
      pp = reglages;
      chargerTable(url);
      dessiner();
    },
    activer(oui) {
      if (oui === actif) return;
      actif = oui;
      canevas.classList.toggle('cache', !oui);
      if (rappel !== null) {
        if ('cancelVideoFrameCallback' in video) video.cancelVideoFrameCallback(rappel);
        cancelAnimationFrame(rappel);
        rappel = null;
      }
      if (oui) boucle();
    },
  };
}
