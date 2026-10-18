/* Patron « splash » du contrat de design KARAYAN (1.2.0) : fond animé d'écran d'accueil,
   d'installation, de chargement ou de page « À propos ». Une aurore fluide (WebGL 1) aux couleurs
   de LA MARQUE DU CONTEXTE : les quatre couleurs sont lues au montage dans les variables CSS du
   contrat (--background, --brand-primary-500, --brand-accent-700, --brand-primary-300), jamais
   écrites ici (R-01). Clair et sombre suivent data-theme (R-03), en fondu.

   Le nuage dérive seul. Le pointeur ne le déplace pas : il le REMUE localement, comme un fluide —
   une traînée des dernières positions imprime des tourbillons qui s'estompent avec la distance
   et le temps. Sans WebGL ou avec « mouvement réduit », le dégradé CSS de .kd-splash__fond reste.

   Usage, sans dépendance :
     <canvas class="kd-splash__fond" data-kd-splash></canvas>      montage automatique au chargement
     var s = KdSplash.mount(canvas, { quality: 0.75 }); s.destroy();   montage manuel (webview, SPA)
   Référence vivante : https://karayan.tech/ (page « en construction », 2026-10-09). */
(function (global) {
  'use strict';
  var N = 16; // points de la traînée du pointeur

  var VERT = 'attribute vec2 a_pos; void main(){ gl_Position = vec4(a_pos, 0.0, 1.0); }';
  var FRAG = [
    'precision highp float;',
    'const int N = ' + N + ';',
    'uniform vec2 u_res; uniform float u_time; uniform float u_light;',
    'uniform vec3 u_fond; uniform vec3 u_primaire; uniform vec3 u_profond; uniform vec3 u_accent;',
    'uniform vec4 u_pts[N]; uniform float u_age[N];',
    'float hash21(vec2 p){ p = fract(p*vec2(123.34,456.21)); p += dot(p, p+45.32); return fract(p.x*p.y); }',
    'float vnoise(vec2 p){ vec2 i = floor(p), f = fract(p); vec2 u = f*f*(3.0-2.0*f);',
    '  return mix(mix(hash21(i),hash21(i+vec2(1,0)),u.x), mix(hash21(i+vec2(0,1)),hash21(i+vec2(1,1)),u.x), u.y); }',
    'float fbm(vec2 p){ float s = 0.0, a = 0.5; for (int i=0;i<5;i++){ s += a*vnoise(p); p *= 2.02; a *= 0.5; } return s; }',
    'void main(){',
    '  vec2 sc = u_res / min(u_res.x, u_res.y);',
    '  vec2 uv = (gl_FragCoord.xy - 0.5*u_res) / min(u_res.x, u_res.y);',
    '  vec2 remous = vec2(0.0);',
    '  for (int i = 0; i < N; i++) {',
    '    vec2 c = (u_pts[i].xy - 0.5) * sc;',
    '    vec2 d = uv - c;',
    '    float poids = exp(-dot(d,d)*22.0) * exp(-u_age[i]*0.9);',
    '    remous += (vec2(-d.y, d.x) * 2.5 + u_pts[i].zw * sc * 5.0) * poids;',
    '  }',
    '  vec2 p = uv * 1.4 + remous;',
    '  float t = u_time * 0.06;',
    '  vec2 q = vec2(fbm(p + t), fbm(p + vec2(5.2,1.3) - t));',
    '  vec2 r = vec2(fbm(p + 4.0*q + vec2(1.7,9.2)), fbm(p + 4.0*q + vec2(8.3,2.8)));',
    '  float f = fbm(p + 4.0*r + t*0.5);',
    /* Quatre couleurs de la marque : le fond, la profondeur (accent 700), la primaire vive (500)
       et la lueur (primaire 300), éclaircies en thème clair. */
    '  vec3 nuit    = u_fond;',
    '  vec3 profond = mix(u_profond, mix(u_profond, vec3(1.0), 0.75), u_light);',
    '  vec3 vif     = u_primaire;',
    '  vec3 clair   = mix(u_accent, mix(u_accent, vec3(1.0), 0.5), u_light);',
    '  vec3 col = mix(nuit, profond, smoothstep(0.0,0.7,f));',
    '  col = mix(col, vif, smoothstep(0.45,0.9,length(q)) * mix(0.85, 0.55, u_light));',
    '  col = mix(col, clair, smoothstep(0.78,1.0,f) * 0.6);',
    '  float scan = 0.5 + 0.5*sin(gl_FragCoord.y*1.2 + u_time*2.0);',
    '  col *= 1.0 - 0.06*scan*(1.0-u_light);',
    '  col *= 1.0 - 0.30*dot(uv,uv);',
    '  gl_FragColor = vec4(col, 1.0);',
    '}'
  ].join('\n');

  function reduit() {
    return global.matchMedia && global.matchMedia('(prefers-reduced-motion: reduce)').matches;
  }

  /* Une variable CSS du contrat → [r, g, b] en 0..1. Les jetons sont en hexadécimal ; un
     canvas 2D sert d'analyseur pour toute autre syntaxe (oklch, rgb()). */
  function couleur(el, nom) {
    var v = getComputedStyle(el).getPropertyValue(nom).trim();
    var c = document.createElement('canvas').getContext('2d');
    c.fillStyle = v;
    var h = c.fillStyle; // normalisé en #rrggbb par le navigateur
    if (h.charAt(0) === '#' && h.length === 7) {
      return [parseInt(h.slice(1, 3), 16) / 255, parseInt(h.slice(3, 5), 16) / 255, parseInt(h.slice(5, 7), 16) / 255];
    }
    var m = /rgba?\(([^)]+)\)/.exec(h);
    if (m) { var n = m[1].split(',').map(parseFloat); return [n[0] / 255, n[1] / 255, n[2] / 255]; }
    return [0.5, 0.5, 0.5];
  }

  function mount(canvas, options) {
    options = options || {};
    var rien = { destroy: function () {} };
    if (!canvas || reduit()) { return rien; }
    var gl = canvas.getContext('webgl', { antialias: false, alpha: false, powerPreference: 'low-power' });
    if (!gl) { return rien; }
    var quality = options.quality || 0.75;
    var interactive = options.interactive !== false;
    var racine = document.documentElement;

    function compile(type, src) {
      var s = gl.createShader(type);
      gl.shaderSource(s, src); gl.compileShader(s);
      return gl.getShaderParameter(s, gl.COMPILE_STATUS) ? s : null;
    }
    var vs = compile(gl.VERTEX_SHADER, VERT), fs = compile(gl.FRAGMENT_SHADER, FRAG);
    if (!vs || !fs) { canvas.dataset.kdSplash = 'erreur-shader'; return rien; }
    var prog = gl.createProgram();
    gl.attachShader(prog, vs); gl.attachShader(prog, fs); gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) { canvas.dataset.kdSplash = 'erreur-programme'; return rien; }
    gl.useProgram(prog);
    canvas.dataset.kdSplash = 'ok';
    var buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    var loc = gl.getAttribLocation(prog, 'a_pos');
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
    var u = {};
    ['u_res', 'u_time', 'u_light', 'u_fond', 'u_primaire', 'u_profond', 'u_accent', 'u_pts', 'u_age']
      .forEach(function (n) { u[n] = gl.getUniformLocation(prog, n); });

    /* Couleurs de la marque, relues à chaque changement de thème (les sémantiques changent). */
    var teintes = {};
    function lireCouleurs() {
      teintes.fond = couleur(canvas, '--background');
      teintes.primaire = couleur(canvas, '--brand-primary-500');
      teintes.profond = couleur(canvas, '--brand-accent-700');   /* la profondeur : accent sombre */
      teintes.accent = couleur(canvas, '--brand-primary-300');  /* la lueur : primaire claire */
    }
    lireCouleurs();
    var observateur = new MutationObserver(lireCouleurs);
    observateur.observe(racine, { attributes: true, attributeFilter: ['data-theme', 'data-brand'] });

    /* Traînée du pointeur : N derniers points (position, vitesse, instant), au plus un toutes
       les 25 ms pour étaler le remous le long du geste. Coordonnées relatives au canvas. */
    var pts = new Float32Array(N * 4), ages = new Float32Array(N), temps = new Float64Array(N);
    for (var k = 0; k < N; k++) { temps[k] = -1e9; }
    var tete = 0, dernier = { x: 0.5, y: 0.5, t: 0 };
    function onMove(e) {
      var now = performance.now();
      if (now - dernier.t < 25) { return; }
      var r = canvas.getBoundingClientRect();
      if (!r.width || !r.height) { return; }
      var x = (e.clientX - r.left) / r.width, y = 1 - (e.clientY - r.top) / r.height;
      if (x < -0.2 || x > 1.2 || y < -0.2 || y > 1.2) { return; }
      var dt = Math.max((now - dernier.t) / 1000, 0.016);
      var vx = (x - dernier.x) / dt, vy = (y - dernier.y) / dt;
      var v = Math.hypot(vx, vy), vmax = 2.5; /* vitesse bornée : un geste vif ne déchire pas le champ */
      if (v > vmax) { vx *= vmax / v; vy *= vmax / v; }
      pts[tete * 4] = x; pts[tete * 4 + 1] = y; pts[tete * 4 + 2] = vx * 0.08; pts[tete * 4 + 3] = vy * 0.08;
      temps[tete] = now;
      tete = (tete + 1) % N;
      dernier = { x: x, y: y, t: now };
    }
    if (interactive) { global.addEventListener('pointermove', onMove, { passive: true }); }

    function resize() {
      var dpr = Math.min(global.devicePixelRatio || 1, 2) * quality;
      var r = canvas.getBoundingClientRect();
      var w = Math.max(1, Math.floor(r.width * dpr)), h = Math.max(1, Math.floor(r.height * dpr));
      if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; gl.viewport(0, 0, w, h); }
    }
    global.addEventListener('resize', resize, { passive: true });
    resize();

    var visible = true, light = racine.getAttribute('data-theme') === 'dark' ? 0 : 1;
    function onVis() { visible = !document.hidden; }
    document.addEventListener('visibilitychange', onVis);
    var debut = performance.now(), raf = 0;
    function tick() {
      raf = requestAnimationFrame(tick);
      if (!visible) { return; }
      var now = performance.now();
      light += ((racine.getAttribute('data-theme') === 'dark' ? 0 : 1) - light) * 0.08;
      for (var i = 0; i < N; i++) { ages[i] = Math.min((now - temps[i]) / 1000, 60); }
      gl.uniform2f(u.u_res, canvas.width, canvas.height);
      gl.uniform1f(u.u_time, (now - debut) / 1000);
      gl.uniform1f(u.u_light, light);
      gl.uniform3fv(u.u_fond, teintes.fond);
      gl.uniform3fv(u.u_primaire, teintes.primaire);
      gl.uniform3fv(u.u_profond, teintes.profond);
      gl.uniform3fv(u.u_accent, teintes.accent);
      gl.uniform4fv(u.u_pts, pts);
      gl.uniform1fv(u.u_age, ages);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    }
    raf = requestAnimationFrame(tick);

    return {
      destroy: function () {
        cancelAnimationFrame(raf);
        observateur.disconnect();
        global.removeEventListener('resize', resize);
        global.removeEventListener('pointermove', onMove);
        document.removeEventListener('visibilitychange', onVis);
        gl.deleteBuffer(buf); gl.deleteProgram(prog); gl.deleteShader(vs); gl.deleteShader(fs);
      }
    };
  }

  global.KdSplash = { mount: mount };
  document.addEventListener('DOMContentLoaded', function () {
    document.querySelectorAll('canvas[data-kd-splash]').forEach(function (c) { mount(c); });
  });
}(window));
