// R-03 du contrat de design KARAYAN : data-theme sur <html>, clé localStorage `karayan_theme`,
// prefers-color-scheme comme défaut, `?theme=` prioritaire (captures). À charger dans <head>
// (avant le rendu, anti-FOUC) ; le bouton .kd-theme-toggle est câblé au chargement du DOM.
(function () {
  var KEY = "karayan_theme";
  var root = document.documentElement;
  function preferred() {
    var q = new URLSearchParams(location.search).get("theme");
    if (q === "light" || q === "dark") return q;
    var s = null;
    try { s = localStorage.getItem(KEY); } catch (e) {}
    if (s === "light" || s === "dark") return s;
    return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  function apply(t) { root.setAttribute("data-theme", t); }
  apply(preferred());
  document.addEventListener("DOMContentLoaded", function () {
    document.querySelectorAll(".kd-theme-toggle").forEach(function (b) {
      b.addEventListener("click", function () {
        var t = root.getAttribute("data-theme") === "dark" ? "light" : "dark";
        apply(t);
        try { localStorage.setItem(KEY, t); } catch (e) {}
      });
    });
  });
})();
