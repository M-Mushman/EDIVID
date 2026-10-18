#!/usr/bin/env python3
"""design-sync — installe ou met à jour la copie vendorée du contrat de design KARAYAN.

Dépendances : stdlib seulement. Ne touche jamais aux sources du dépôt : écrit design/vendor/,
design/karayan-design.lock et les miroirs déclarés.

Exemples :
  python3 design-sync.py --brand karayan --stack fastapi
  python3 design-sync.py --brand gnoz --stack web --from /home/ubuntu/karayan-web/packages/tokens/dist
  python3 design-sync.py --brand karayan --stack web --workspace ../../packages/tokens/dist
  python3 design/vendor/design-sync.py --version 1.1.0            # mise à jour, reprend le lock
  python3 design-sync.py --brand karayan --stack fastapi --mirror apps/api/static/karayan.css=brands/karayan.css
"""
from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import shutil
import sys
import urllib.request
from pathlib import Path

CONTRACT = "karayan-design"
DEFAULT_BASE = "https://mcp.mcp-zone.net/design"
STACKS = ("web", "tauri", "fastapi", "rust")
LOCK_NAME = "karayan-design.lock"


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class Source:
    def __init__(self, base: str | None, local: Path | None, version: str):
        self.base, self.local, self.version = base, local, version

    def describe(self) -> str:
        return str(self.local) if self.local else f"{self.base}/{self.version}/"

    def read(self, rel: str) -> bytes:
        if self.local:
            return (self.local / rel).read_bytes()
        url = f"{self.base}/{self.version}/{rel}"
        with urllib.request.urlopen(url, timeout=30) as r:  # noqa: S310 — URL du contrat, choisie par l'appelant
            return r.read()


def select_files(manifest: dict, stack: str, brand: str) -> list[str]:
    profiles = manifest.get("profiles") or {}
    globs = [g.replace("{brand}", brand) for g in (profiles.get("common") or []) + (profiles.get(stack) or [])]
    files = sorted(manifest.get("files") or {})
    chosen = [f for f in files if any(fnmatch.fnmatch(f, g) for g in globs)]
    missing = [g for g in globs if "*" not in g and g not in files and g != "manifest.json"]
    if missing:
        print(f"[design-sync] avertissement : absent du manifeste : {', '.join(missing)}", file=sys.stderr)
    return chosen


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--brand", help="marque de l'app (karayan, swihn, aristote, morfo, gnoz)")
    ap.add_argument("--stack", choices=STACKS, help="web | tauri | fastapi | rust")
    ap.add_argument("--surface", choices=("marketing", "produit", "admin"), default=None, help="R-06 bloquante seulement pour marketing (défaut : marketing si stack web, produit sinon)")
    ap.add_argument("--version", default=None, help="version du contrat (défaut : latest, ou celle du lock)")
    ap.add_argument("--base", default=None, help=f"URL de distribution (défaut {DEFAULT_BASE})")
    ap.add_argument("--from", dest="from_dir", default=None, help="dossier dist local au lieu de l'URL")
    ap.add_argument("--workspace", default=None, help="monorepo : chemin RELATIF vers packages/tokens/dist, aucune copie")
    ap.add_argument("--root", default=None, help="racine du dépôt (défaut : cwd, ou parent de design/ si lancé depuis vendor/)")
    ap.add_argument("--mirror", action="append", default=[], metavar="DEST=SRC", help="copie SRC (chemin dans le contrat) vers DEST (chemin dans le dépôt)")
    ap.add_argument("--include", action="append", default=None, help="restreint design-check à ces dossiers (répétable)")
    ap.add_argument("--exclude", action="append", default=None, help="globs exclus de design-check (répétable)")
    args = ap.parse_args()

    here = Path(__file__).resolve()
    root = Path(args.root).resolve() if args.root else (here.parent.parent.parent if here.parent.name == "vendor" else Path.cwd().resolve())
    design_dir = root / "design"
    lock_path = design_dir / LOCK_NAME
    prev = json.loads(lock_path.read_text(encoding="utf-8")) if lock_path.exists() else {}

    brand = args.brand or prev.get("brand")
    stack = args.stack or prev.get("stack")
    surface = args.surface or prev.get("surface")
    if not brand or not stack:
        ap.error("--brand et --stack sont requis la première fois (aucun lock trouvé)")
    version = args.version or ("latest" if not prev else prev.get("version", "latest"))
    base = args.base or (prev.get("base") if not args.from_dir else None) or DEFAULT_BASE
    mirrors = dict(prev.get("mirrors") or {})
    for m in args.mirror:
        dest, _, src = m.partition("=")
        if not src:
            ap.error(f"--mirror attend DEST=SRC, reçu « {m} »")
        mirrors[dest] = src
    scan = dict(prev.get("scan") or {})
    if args.include is not None:
        scan["include"] = args.include
    if args.exclude is not None:
        scan["exclude"] = args.exclude

    # Mode monorepo : pas de copie, le lock pointe sur dist/ et n'enregistre que les sha
    if args.workspace or prev.get("source") == "workspace":
        rel = args.workspace or prev.get("vendor_dir")
        vendor_dir = (root / rel).resolve()
        manifest = json.loads((vendor_dir / "manifest.json").read_text(encoding="utf-8"))
        files = select_files(manifest, stack, brand)
        lock = {
            "contract": CONTRACT, "version": manifest["version"], "brand": brand, "stack": stack, "surface": surface,
            "source": "workspace", "vendor_dir": rel,
            "files": {f: sha256_bytes((vendor_dir / f).read_bytes()) for f in files if (vendor_dir / f).exists()},
            "mirrors": mirrors, "scan": scan,
        }
        design_dir.mkdir(parents=True, exist_ok=True)
        lock_path.write_text(json.dumps(lock, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        print(f"[design-sync] lock workspace écrit : {lock_path} (contrat {manifest['version']}, {len(lock['files'])} fichiers suivis)")
        return 0

    local = Path(args.from_dir).resolve() if args.from_dir else None
    if local is None and prev and prev.get("base") is None and prev.get("source") and Path(prev["source"]).is_dir():
        local = Path(prev["source"]).resolve()  # mise à jour d'un lock installé depuis un dossier local
    src = Source(None if local else base, local, version)
    manifest = json.loads(src.read("manifest.json").decode("utf-8"))
    real_version = manifest["version"]
    if not local and version == "latest":
        src = Source(base, None, real_version)  # figer sur la version réelle, pas sur l'alias
    if brand not in (manifest.get("brands") or [brand]):
        ap.error(f"marque « {brand} » inconnue du contrat {real_version} : {', '.join(manifest.get('brands') or [])}")

    files = select_files(manifest, stack, brand)
    vendor_dir = design_dir / "vendor"
    if vendor_dir.exists():
        shutil.rmtree(vendor_dir)
    vendor_dir.mkdir(parents=True)
    lock_files: dict[str, str] = {}
    for rel in files:
        data = src.read(rel)
        expected = manifest["files"][rel]["sha256"]
        got = sha256_bytes(data)
        if got != expected:
            print(f"[design-sync] ERREUR sha256 sur {rel} : {got[:12]} ≠ {expected[:12]} (manifeste)", file=sys.stderr)
            return 2
        dest = vendor_dir / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
        lock_files[rel] = got
    # le manifeste lui-même, toujours
    mdata = src.read("manifest.json")
    (vendor_dir / "manifest.json").write_bytes(mdata)
    lock_files["manifest.json"] = sha256_bytes(mdata)

    for dest, s in mirrors.items():
        sp = vendor_dir / s
        if not sp.exists():
            print(f"[design-sync] ERREUR miroir : {s} n'est pas dans la sélection de la stack {stack}", file=sys.stderr)
            return 2
        dp = root / dest
        dp.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(sp, dp)
        print(f"[design-sync] miroir : {dest} ← {s}")

    lock = {
        "contract": CONTRACT, "version": real_version, "brand": brand, "stack": stack, "surface": surface,
        "source": src.describe(), "base": None if local else base, "vendor_dir": "design/vendor",
        "files": lock_files, "mirrors": mirrors, "scan": scan,
    }
    lock_path.write_text(json.dumps(lock, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"[design-sync] contrat {real_version} · marque {brand} · stack {stack} · {len(lock_files)} fichiers → {vendor_dir}")
    print(f"[design-sync] lock : {lock_path}")
    print("[design-sync] suite : python3 design/vendor/design-check.py ; git add design/ ; entrée de registre.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
