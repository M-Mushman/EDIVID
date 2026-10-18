#!/usr/bin/env python3
"""design-check — vérificateur du contrat de design KARAYAN (règles R-01, R-02, R-03, R-06, R-07, R-10).

Dépendances : stdlib seulement (PyYAML optionnel, pour lire registre.yaml). Même contrainte que
outils/registre.py de KARAYAN TestLoop.

Usage : python3 design/vendor/design-check.py [--root DIR] [--no-report] [--quiet]
Sortie : rapport dans design/reports/<AAAA-MM-JJ>.txt ; code de retour 1 si au moins une erreur
non couverte par design/karayan-design.allow, 0 sinon.
"""
from __future__ import annotations

import argparse
import datetime as _dt
import fnmatch
import hashlib
import json
import os
import re
import sys
from pathlib import Path

CONTRACT = "karayan-design"
LOCK_NAME = "karayan-design.lock"
ALLOW_NAME = "karayan-design.allow"

CSS_LIKE = {".css", ".scss", ".sass", ".less"}
HTML_LIKE = {".html", ".htm", ".jinja", ".jinja2", ".j2", ".vue", ".svelte", ".astro"}
CODE_LIKE = {".tsx", ".jsx", ".ts", ".js", ".mjs", ".cjs", ".py", ".rs"}
SCANNED = CSS_LIKE | HTML_LIKE | CODE_LIKE

DEFAULT_EXCLUDE_DIRS = {
    ".git", "node_modules", "dist", "build", "target", ".next", "out", "__pycache__", "venv",
    ".venv", "coverage", "__baselines__", "__results__", ".turbo", "vendor", "reports",
    "tests", "test", "__tests__", "e2e", "docs",
}
FRONT_CODE = {".tsx", ".jsx", ".ts", ".js", ".mjs", ".cjs"}  # R-06 : le code de rendu, pas le backend
DEFAULT_EXCLUDE_GLOBS = ["*.min.*", "*.d.ts", "design/*", "*.test.*", "*.spec.*", "tests/*"]

GENERIC_FONTS = {
    "serif", "sans-serif", "monospace", "cursive", "fantasy", "system-ui", "ui-serif",
    "ui-sans-serif", "ui-monospace", "ui-rounded", "emoji", "math", "fangsong", "inherit",
    "initial", "unset", "revert",
}
TAILWIND_PALETTE = (
    "slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|"
    "blue|indigo|violet|purple|fuchsia|pink|rose"
)
RE_TAILWIND = re.compile(
    r"(?<![\w-])(?:bg|text|border|ring|fill|stroke|from|to|via|shadow|outline|decoration|accent|"
    r"caret|divide|placeholder)-(?:" + TAILWIND_PALETTE + r")-\d{2,3}(?![\w-])"
)
RE_HEX = re.compile(r"(?<![\w&/])#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})(?![\w-])")
RE_FUNC = re.compile(r"\b(?:rgba?|hsla?|oklch|oklab|lab|lch|hwb)\(\s*[\d.]")
RE_FONT_FAMILY = re.compile(r"font-family\s*:\s*([^;}\n]+)", re.I)
RE_FONT_CDN = re.compile(r"fonts\.googleapis\.com|fonts\.gstatic\.com|next/font/google|use\.typekit\.net", re.I)
RE_DARK_CLASS = re.compile(r"(?<![\w-])\.dark(?![\w-])")
RE_THEME_KEY = re.compile(r"localStorage\.(?:get|set|remove)Item\(\s*[\"']([^\"']*theme[^\"']*)[\"']", re.I)
RE_PROVIDERS = re.compile(
    r"\b(?:openai|chatgpt|gpt-?[345](?:\.\d)?|anthropic|claude|mistral|gemini|ollama|deepseek|llama|"
    r"qwen|cohere|perplexity|groq|huggingface|infomaniak)\b",
    re.I,
)
RE_SCALE = re.compile(r"\b(font-size|padding(?:-\w+)?|margin(?:-\w+)?|border-radius|gap|row-gap|column-gap)\s*:\s*([^;}\n]+)", re.I)
RE_SCALE_LITERAL = re.compile(r"(?<![\w.-])(?!0(?:px|rem|em)?\b)\d*\.?\d+(?:px|rem|em)\b")
RE_STRINGS = re.compile(r"\"(?:[^\"\\\n]|\\.)*\"|'(?:[^'\\\n]|\\.)*'|`(?:[^`\\]|\\.)*`", re.S)
RE_STYLE_BLOCK = re.compile(r"<style\b[^>]*>(.*?)</style>", re.S | re.I)
RE_STYLE_ATTR = re.compile(r"\b(?:style|fill|stroke|color|bgcolor)\s*=\s*([\"'])(.*?)\1", re.S | re.I)
RE_HTML_TAG = re.compile(r"<[^>]+>")
RE_HTML_COMMENT = re.compile(r"<!--.*?-->", re.S)


class Finding:
    __slots__ = ("rule", "severity", "path", "line", "excerpt", "tolerated", "registre")

    def __init__(self, rule, severity, path, line, excerpt):
        self.rule, self.severity, self.path, self.line = rule, severity, path, line
        self.excerpt = " ".join(excerpt.split())[:110]
        self.tolerated = False
        self.registre = None


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def line_of(text: str, pos: int) -> int:
    return text.count("\n", 0, pos) + 1


def load_json(path: Path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


# ── Registre du dépôt (pour valider les exceptions) ──────────────────────────

def registre_ids(root: Path) -> tuple[set[str] | None, str]:
    """Retourne (ids, source). ids=None si aucun registre trouvé."""
    ry = root / "registre.yaml"
    if ry.exists():
        try:
            import yaml  # type: ignore
            data = yaml.safe_load(ry.read_text(encoding="utf-8")) or {}
            ids = set()
            for key in ("registre", "dettes"):
                for e in data.get(key) or []:
                    if isinstance(e, dict) and e.get("id"):
                        ids.add(str(e["id"]))
            return ids, "registre.yaml"
        except ImportError:
            ids = set(re.findall(r"^\s*-\s*id:\s*([A-Z]+-\d+)", ry.read_text(encoding="utf-8"), re.M))
            return ids, "registre.yaml (sans PyYAML, lecture regex)"
    for name in ("ARCHITECTURE.md", "REGISTRE.md"):
        p = root / name
        if p.exists():
            ids = set(re.findall(r"\b([A-Z]{1,4}-\d{3})\b", p.read_text(encoding="utf-8")))
            return ids, name
    return None, "aucun"


# ── Analyse d'un fichier ──────────────────────────────────────────────────────

def css_regions(text: str, ext: str) -> list[tuple[int, str]]:
    """Portions de texte à traiter comme du CSS : (offset, fragment)."""
    if ext in CSS_LIKE:
        return [(0, text)]
    if ext in HTML_LIKE:
        text = RE_HTML_COMMENT.sub(lambda m: " " * len(m.group(0)), text)
        regions = [(m.start(1), m.group(1)) for m in RE_STYLE_BLOCK.finditer(text)]
        regions += [(m.start(2), m.group(2)) for m in RE_STYLE_ATTR.finditer(text)]
        return regions
    return []


def string_regions(text: str, ext: str) -> list[tuple[int, str]]:
    if ext in CODE_LIKE:
        return [(m.start(), m.group(0)) for m in RE_STRINGS.finditer(text)]
    return []


def html_text_regions(text: str) -> list[tuple[int, str]]:
    """Texte visible d'un fichier HTML (hors balises et commentaires), pour R-06."""
    text = RE_HTML_COMMENT.sub(lambda m: " " * len(m.group(0)), text)
    text = RE_STYLE_BLOCK.sub(lambda m: " " * len(m.group(0)), text)
    text = re.sub(r"<script\b[^>]*>.*?</script>", lambda m: " " * len(m.group(0)), text, flags=re.S | re.I)
    text = RE_HTML_TAG.sub(lambda m: " " * len(m.group(0)), text)
    return [(0, text)]


def scan_file(path: Path, rel: str, text: str, surface: str, fonts_allowed: set[str]) -> list[Finding]:
    ext = path.suffix.lower()
    out: list[Finding] = []
    css = css_regions(text, ext)
    strings = string_regions(text, ext)
    colour_regions = css + strings
    r06_sev = "erreur" if surface == "marketing" else "avertissement"

    # R-01 couleurs littérales
    for off, frag in colour_regions:
        for m in RE_HEX.finditer(frag):
            if ext in CODE_LIKE and len(m.group(0)) <= 5:
                # hex court dans une chaîne de code : « #1234 » peut être un numéro de ticket —
                # retenu seulement en position de valeur CSS ("#fff", "color: #fff;", "(#fff)").
                before = frag[m.start() - 1] if m.start() > 0 else ""
                after = frag[m.end()] if m.end() < len(frag) else ""
                if before not in "\"'`(: " or after not in "\"'`;), ":
                    continue
                if before == " " and not frag[: m.start()].rstrip().endswith(":"):
                    continue
            out.append(Finding("R-01", "erreur", rel, line_of(text, off + m.start()), f"couleur littérale {m.group(0)}"))
        for m in RE_FUNC.finditer(frag):
            out.append(Finding("R-01", "erreur", rel, line_of(text, off + m.start()), f"couleur littérale {frag[m.start():m.start()+24]}…"))
    # R-01 palette Tailwind par défaut (chaînes de classes : HTML et code)
    tw_regions = strings if ext in CODE_LIKE else ([(0, text)] if ext in HTML_LIKE else [])
    for off, frag in tw_regions:
        for m in RE_TAILWIND.finditer(frag):
            out.append(Finding("R-01", "erreur", rel, line_of(text, off + m.start()), f"palette Tailwind par défaut {m.group(0)}"))

    # R-02 polices
    for m in RE_FONT_CDN.finditer(text):
        out.append(Finding("R-02", "erreur", rel, line_of(text, m.start()), f"police non auto-hébergée : {m.group(0)}"))
    for off, frag in css + ([(0, text)] if ext in CODE_LIKE else []):
        for m in RE_FONT_FAMILY.finditer(frag):
            value = m.group(1)
            if "var(" in value:
                continue
            for fam in value.split(","):
                fam = fam.strip().strip("\"'").strip()
                if not fam or fam.lower() in GENERIC_FONTS or fam.startswith("$") or fam.startswith("{"):
                    continue
                if fam not in fonts_allowed:
                    out.append(Finding("R-02", "erreur", rel, line_of(text, off + m.start()), f"police hors contrat : {fam}"))

    # R-03 thème
    for off, frag in css:
        for m in RE_DARK_CLASS.finditer(frag):
            out.append(Finding("R-03", "erreur", rel, line_of(text, off + m.start()), "sélecteur .dark (utiliser [data-theme=dark])"))
    for m in RE_THEME_KEY.finditer(text):
        if m.group(1) != "karayan_theme":
            out.append(Finding("R-03", "erreur", rel, line_of(text, m.start()), f"clé de thème « {m.group(1)} » (attendu karayan_theme)"))

    # R-06 souveraineté : chaînes du code de rendu (jamais le backend .py/.rs, qui nomme
    # légitimement ses fournisseurs) et texte visible des gabarits HTML/Jinja
    r06_regions = strings if ext in FRONT_CODE else (html_text_regions(text) if ext in HTML_LIKE else [])
    for off, frag in r06_regions:
        for m in RE_PROVIDERS.finditer(frag):
            out.append(Finding("R-06", r06_sev, rel, line_of(text, off + m.start()), f"nom de fournisseur d'IA : {m.group(0)}"))

    # R-07 échelle (avertissement)
    for off, frag in css:
        for m in RE_SCALE.finditer(frag):
            value = m.group(2)
            if "var(" in value or "calc(" in value or "clamp(" in value:
                continue
            if RE_SCALE_LITERAL.search(value):
                out.append(Finding("R-07", "avertissement", rel, line_of(text, off + m.start()), f"{m.group(1)}: {value.strip()[:40]} (token attendu)"))
    return out


# ── Parcours ──────────────────────────────────────────────────────────────────

def iter_files(root: Path, include: list[str], exclude_globs: list[str]):
    bases = [root / i for i in include] if include else [root]
    for base in bases:
        if base.is_file():
            yield base
            continue
        for dirpath, dirnames, filenames in os.walk(base):
            dirnames[:] = [d for d in dirnames if d not in DEFAULT_EXCLUDE_DIRS and not d.startswith(".")]
            for fn in filenames:
                p = Path(dirpath) / fn
                if p.suffix.lower() not in SCANNED:
                    continue
                rel = p.relative_to(root).as_posix()
                if any(fnmatch.fnmatch(rel, g) or fnmatch.fnmatch(fn, g) for g in exclude_globs):
                    continue
                yield p


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--root", default=None, help="racine du dépôt (défaut : dossier parent de design/)")
    ap.add_argument("--no-report", action="store_true", help="ne pas écrire design/reports/<date>.txt")
    ap.add_argument("--quiet", action="store_true")
    args = ap.parse_args()

    here = Path(__file__).resolve()
    root = Path(args.root).resolve() if args.root else (here.parent.parent.parent if here.parent.name == "vendor" else Path.cwd())
    design_dir = root / "design"
    lock_path = design_dir / LOCK_NAME
    errors: list[Finding] = []
    findings: list[Finding] = []
    lines: list[str] = []
    today = _dt.date.today().isoformat()

    def err(rule, msg):
        f = Finding(rule, "erreur", "design/", 0, msg)
        findings.append(f)

    if not lock_path.exists():
        err("R-10", f"lock absent : {lock_path} — lancer design-sync.py")
        lock = {}
    else:
        lock = load_json(lock_path)

    version = lock.get("version", "?")
    brand = lock.get("brand", "?")
    stack = lock.get("stack", "?")
    # surface : marketing (défaut pour stack web) → R-06 bloquante ; produit | admin → avertissement
    surface = lock.get("surface") or ("marketing" if stack == "web" else "produit")
    vendor_dir = (root / lock.get("vendor_dir", "design/vendor")).resolve()
    manifest = {}
    mpath = vendor_dir / "manifest.json"
    if mpath.exists():
        manifest = load_json(mpath)
        if manifest.get("version") != version:
            err("R-10", f"version du lock ({version}) ≠ version du manifeste vendoré ({manifest.get('version')})")
    elif lock:
        err("R-10", f"manifeste absent : {mpath}")

    brands_known = manifest.get("brands") or []
    if lock and brands_known and brand not in brands_known:
        err("R-10", f"marque « {brand} » inconnue du contrat ({', '.join(brands_known)})")

    # Intégrité des fichiers vendorés et des miroirs
    for relf, expected in (lock.get("files") or {}).items():
        p = vendor_dir / relf
        if not p.exists():
            err("R-10", f"fichier vendoré manquant : {relf}")
        elif sha256(p) != expected:
            err("R-10", f"fichier vendoré modifié : {relf} (sha256 ≠ lock)")
    for dest, src in (lock.get("mirrors") or {}).items():
        pd, ps = root / dest, vendor_dir / src
        if not pd.exists():
            err("R-10", f"miroir manquant : {dest}")
        elif not ps.exists():
            err("R-10", f"source du miroir manquante : {src}")
        elif sha256(pd) != sha256(ps):
            err("R-10", f"miroir désynchronisé : {dest} ≠ {src} (relancer design-sync --mirror)")

    # Polices autorisées : celles de la marque
    fonts_allowed: set[str] = set()
    bj = vendor_dir / "brands" / f"{brand}.json"
    if bj.exists():
        font = load_json(bj).get("font") or {}
        fonts_allowed = {v for v in font.values() if v}
        # variantes usuelles des noms fontsource
        for v in list(fonts_allowed):
            fonts_allowed.add(v.replace(" Variable", ""))
    else:
        for b in brands_known:
            pj = vendor_dir / "brands" / f"{b}.json"
            if pj.exists():
                fonts_allowed |= {v for v in (load_json(pj).get("font") or {}).values() if v}

    # Parcours des sources
    scan = lock.get("scan") or {}
    include = scan.get("include") or []
    # les miroirs sont des copies du contrat : jamais analysés comme des sources
    exclude = DEFAULT_EXCLUDE_GLOBS + (scan.get("exclude") or []) + list((lock.get("mirrors") or {}).keys())
    nfiles = 0
    for p in iter_files(root, include, exclude):
        try:
            text = p.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        nfiles += 1
        findings.extend(scan_file(p, p.relative_to(root).as_posix(), text, surface, fonts_allowed))

    # R-03 : un commutateur de thème existe-t-il ?
    if lock and stack != "rust":
        has_switch = any(
            any(k in t for k in ("data-theme", "karayan_theme", "ThemeScript", "ThemeToggle", "useTheme"))
            for t in (p.read_text(encoding="utf-8", errors="ignore") for p in iter_files(root, include, exclude))
        )
        if not has_switch:
            findings.append(Finding("R-03", "avertissement", "(dépôt)", 0, "aucun commutateur data-theme / karayan_theme trouvé dans les sources"))

    # Exceptions
    allow_path = design_dir / ALLOW_NAME
    allow = []
    if allow_path.exists():
        try:
            allow = load_json(allow_path)
        except json.JSONDecodeError as e:
            err("R-10", f"{ALLOW_NAME} illisible : {e}")
    ids, reg_source = registre_ids(root)
    for a in allow:
        rid = a.get("registre")
        if not rid or ids is None or rid not in ids:
            err("R-10", f"exception {a.get('regle')} {a.get('chemin')} : entrée de registre « {rid} » introuvable ({reg_source})")
            continue
        for f in findings:
            if f.rule == a.get("regle") and fnmatch.fnmatch(f.path, a.get("chemin", "")):
                f.tolerated, f.registre = True, rid

    errors = [f for f in findings if f.severity == "erreur" and not f.tolerated]
    warnings = [f for f in findings if f.severity == "avertissement" and not f.tolerated]
    tolerated = [f for f in findings if f.tolerated]
    verdict = "VERT" if not errors else "ROUGE"

    lines.append(f"design-check — contrat {CONTRACT} {version} — {today}")
    lines.append(f"dépôt : {root}")
    lines.append(f"marque : {brand} · stack : {stack} · surface : {surface} · source : {lock.get('source', '?')} · registre : {reg_source}")
    lines.append(f"fichiers analysés : {nfiles}")
    lines.append(f"verdict : {verdict} — {len(errors)} erreur(s), {len(warnings)} avertissement(s), {len(tolerated)} tolérée(s)")
    by_rule: dict[str, int] = {}
    for f in findings:
        by_rule[f.rule] = by_rule.get(f.rule, 0) + 1
    if by_rule:
        lines.append("par règle : " + ", ".join(f"{k}={v}" for k, v in sorted(by_rule.items())))
    lines.append("")
    for title, group in (("ERREURS", errors), ("AVERTISSEMENTS", warnings), ("TOLÉRÉES (allow)", tolerated)):
        if not group:
            continue
        lines.append(f"== {title} ({len(group)}) ==")
        for f in group[:400]:
            loc = f"{f.path}:{f.line}" if f.line else f.path
            suffix = f"  [{f.registre}]" if f.registre else ""
            lines.append(f"{f.rule}  {loc}  {f.excerpt}{suffix}")
        if len(group) > 400:
            lines.append(f"… {len(group) - 400} de plus")
        lines.append("")

    report = "\n".join(lines)
    if not args.quiet:
        print(report)
    if not args.no_report and lock:
        rdir = design_dir / "reports"
        rdir.mkdir(parents=True, exist_ok=True)
        (rdir / f"{today}.txt").write_text(report + "\n", encoding="utf-8")
        if not args.quiet:
            print(f"rapport : {rdir / (today + '.txt')}")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
