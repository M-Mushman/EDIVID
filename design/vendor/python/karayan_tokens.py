# Généré par packages/tokens/scripts/build.mjs — contrat de design KARAYAN 1.3.0. Ne pas éditer.
"""Tokens du contrat de design KARAYAN, pour Python (FastAPI/Jinja, sidecar, PDF, graphes).

    from karayan_tokens import brand, semantic, ramp, css_vars, VERSION
    semantic("karayan", "dark")["primary"]   -> "#…"
    css_vars("gnoz", "light")                -> {"--primary": "#…", …}
"""
import json as _json

_DATA = _json.loads(r"""{
  "version": "1.3.0",
  "base": {
    "text": {
      "xs": "clamp(0.75rem, 0.7rem + 0.25vw, 0.875rem)",
      "sm": "clamp(0.875rem, 0.8rem + 0.35vw, 1rem)",
      "base": "clamp(1rem, 0.95rem + 0.25vw, 1.125rem)",
      "lg": "clamp(1.125rem, 1rem + 0.75vw, 1.5rem)",
      "xl": "clamp(1.5rem, 1.2rem + 1.25vw, 2.25rem)",
      "2xl": "clamp(2rem, 1.2rem + 2.5vw, 3.5rem)",
      "3xl": "clamp(2.5rem, 1rem + 4vw, 5rem)",
      "hero": "clamp(3rem, 0.5rem + 6vw, 7rem)",
      "giant": "clamp(4rem, 1rem + 9vw, 11rem)"
    },
    "space": {
      "1": "0.25rem",
      "2": "0.5rem",
      "3": "0.75rem",
      "4": "1rem",
      "5": "1.25rem",
      "6": "1.5rem",
      "8": "2rem",
      "10": "2.5rem",
      "12": "3rem",
      "16": "4rem",
      "20": "5rem",
      "24": "6rem"
    },
    "radius": {
      "sm": "0.375rem",
      "md": "0.5rem",
      "lg": "0.75rem",
      "xl": "1rem",
      "full": "9999px"
    },
    "content": {
      "narrow": "640px",
      "default": "960px",
      "wide": "1200px"
    },
    "shadow": {
      "glow-sm": "0 0 12px color-mix(in oklch, var(--brand-primary-500) 25%, transparent)",
      "glow-md": "0 0 28px color-mix(in oklch, var(--brand-primary-500) 35%, transparent), 0 0 8px color-mix(in oklch, var(--brand-primary-500) 20%, transparent)",
      "glow-lg": "0 0 60px color-mix(in oklch, var(--brand-primary-500) 30%, transparent), 0 0 20px color-mix(in oklch, var(--brand-primary-500) 20%, transparent)"
    },
    "motion": {
      "interactive": [
        0.16,
        1,
        0.3,
        1
      ],
      "durationInteractive": "180ms"
    }
  },
  "brands": {
    "aristote": {
      "id": "aristote",
      "name": "Aristote Academy",
      "family": "morfo",
      "status": "existante",
      "light": {
        "background": "#f7f3ec",
        "foreground": "#201f1d",
        "card": "#f9f8f8",
        "primary": "#8b5f00",
        "muted": "#efeeed",
        "mutedForeground": "#6c6863",
        "border": "#c5c4c1",
        "ring": "#8b5f00",
        "accent": "#837ac5",
        "primaryForeground": "#ffffff",
        "accentForeground": "#0a0a0a"
      },
      "dark": {
        "background": "#141210",
        "foreground": "#f9f8f8",
        "card": "#201f1d",
        "primary": "#bda074",
        "muted": "#201f1d",
        "mutedForeground": "#a6a4a0",
        "border": "#35332f",
        "ring": "#bda074",
        "accent": "#a29dd5",
        "primaryForeground": "#0a0a0a",
        "accentForeground": "#0a0a0a"
      },
      "ramps": {
        "primary": {
          "50": "#fbf8f4",
          "100": "#f4eee5",
          "200": "#e7dccd",
          "300": "#d4c1a7",
          "400": "#bda074",
          "500": "#a67e40",
          "600": "#8b5f00",
          "700": "#684600",
          "800": "#462d00",
          "900": "#2c1b00",
          "950": "#150b00"
        },
        "accent": {
          "50": "#f8f8fe",
          "100": "#eeedf9",
          "200": "#dcdbf1",
          "300": "#c2c0e3",
          "400": "#a29dd5",
          "500": "#837ac5",
          "600": "#675aaf",
          "700": "#4c3f89",
          "800": "#32285f",
          "900": "#1e193c",
          "950": "#0d0a1d"
        },
        "neutral": {
          "50": "#f9f8f8",
          "100": "#efeeed",
          "200": "#dfdedc",
          "300": "#c5c4c1",
          "400": "#a6a4a0",
          "500": "#898680",
          "600": "#6c6863",
          "700": "#4f4d48",
          "800": "#35332f",
          "900": "#201f1d",
          "950": "#0e0d0c"
        }
      },
      "font": {
        "display": "Cormorant Garamond",
        "body": "Inter Variable",
        "mono": null
      },
      "radius": {
        "base": "0.5rem"
      },
      "motion": {
        "interactive": "180ms",
        "easing": "cubic-bezier(0.16, 1, 0.3, 1)"
      },
      "canvas": {
        "bg": "oklch(50% 0.010 82)",
        "primary": "oklch(57.2% 0.110 77)",
        "accent": "oklch(41.1% 0.131 288)",
        "highlight": "oklch(48.4% 0.094 153)"
      }
    },
    "gnoz": {
      "id": "gnoz",
      "name": "gnoz.io",
      "family": "morfo",
      "status": "hypothese",
      "light": {
        "background": "#f8f8f9",
        "foreground": "#1f1f21",
        "card": "#eeeeef",
        "primary": "#6b4ec7",
        "muted": "#eeeeef",
        "mutedForeground": "#69686e",
        "border": "#c4c4c6",
        "ring": "#6b4ec7",
        "accent": "#5a984c",
        "primaryForeground": "#ffffff",
        "accentForeground": "#0a0a0a"
      },
      "dark": {
        "background": "#0d0d0e",
        "foreground": "#f8f8f9",
        "card": "#1f1f21",
        "primary": "#a498e6",
        "muted": "#1f1f21",
        "mutedForeground": "#a4a4a8",
        "border": "#333236",
        "ring": "#a498e6",
        "accent": "#85b37b",
        "primaryForeground": "#0a0a0a",
        "accentForeground": "#0a0a0a"
      },
      "ramps": {
        "primary": {
          "50": "#f8f7ff",
          "100": "#eeecfc",
          "200": "#dddaf7",
          "300": "#c3bdee",
          "400": "#a498e6",
          "500": "#8773db",
          "600": "#6b4ec7",
          "700": "#50349d",
          "800": "#351f6e",
          "900": "#211346",
          "950": "#0e0722"
        },
        "accent": {
          "50": "#f5faf4",
          "100": "#e8f2e6",
          "200": "#d3e4cf",
          "300": "#b1ceab",
          "400": "#85b37b",
          "500": "#5a984c",
          "600": "#307c1b",
          "700": "#185d00",
          "800": "#0d3e00",
          "900": "#072701",
          "950": "#031201"
        },
        "neutral": {
          "50": "#f8f8f9",
          "100": "#eeeeef",
          "200": "#dededf",
          "300": "#c4c4c6",
          "400": "#a4a4a8",
          "500": "#86858b",
          "600": "#69686e",
          "700": "#4d4c52",
          "800": "#333236",
          "900": "#1f1f21",
          "950": "#0d0d0e"
        }
      },
      "font": {
        "display": "Space Grotesk Variable",
        "body": "Inter Variable",
        "mono": null
      },
      "radius": {
        "base": "0.5rem"
      },
      "motion": {
        "interactive": "180ms",
        "easing": "cubic-bezier(0.16, 1, 0.3, 1)"
      },
      "canvas": {
        "bg": "oklch(18% 0.02 290)",
        "primary": "oklch(55% 0.18 290)",
        "accent": "oklch(70% 0.15 140)",
        "highlight": "oklch(80% 0.05 290)"
      }
    },
    "karayan": {
      "id": "karayan",
      "name": "KARAYAN",
      "family": "flagship",
      "status": "existante",
      "light": {
        "background": "#ffffff",
        "foreground": "#1f1f23",
        "card": "#f8f8f9",
        "primary": "#bf0063",
        "muted": "#eeeef0",
        "mutedForeground": "#676872",
        "border": "#c3c4c8",
        "ring": "#bf0063",
        "accent": "#aa5cdc",
        "primaryForeground": "#ffffff",
        "accentForeground": "#0a0a0a"
      },
      "dark": {
        "background": "#0d0d1a",
        "foreground": "#f8f8f9",
        "card": "#1f1f23",
        "primary": "#ef78a0",
        "muted": "#1f1f23",
        "mutedForeground": "#a3a4aa",
        "border": "#323238",
        "ring": "#ef78a0",
        "accent": "#c08ae7",
        "primaryForeground": "#0a0a0a",
        "accentForeground": "#0a0a0a"
      },
      "ramps": {
        "primary": {
          "50": "#fff6f8",
          "100": "#ffe7ed",
          "200": "#fed0dc",
          "300": "#f7abc1",
          "400": "#ef78a0",
          "500": "#e13c80",
          "600": "#bf0063",
          "700": "#8f0049",
          "800": "#610030",
          "900": "#40001d",
          "950": "#21000c"
        },
        "accent": {
          "50": "#fbf6ff",
          "100": "#f4eafd",
          "200": "#e8d5f8",
          "300": "#d6b5ef",
          "400": "#c08ae7",
          "500": "#aa5cdc",
          "600": "#9129c8",
          "700": "#6f089e",
          "800": "#4c006e",
          "900": "#300346",
          "950": "#160223"
        },
        "neutral": {
          "50": "#f8f8f9",
          "100": "#eeeef0",
          "200": "#dddee0",
          "300": "#c3c4c8",
          "400": "#a3a4aa",
          "500": "#85858e",
          "600": "#676872",
          "700": "#4c4c54",
          "800": "#323238",
          "900": "#1f1f23",
          "950": "#0d0d0f"
        }
      },
      "font": {
        "display": "Space Grotesk Variable",
        "body": "Inter Variable",
        "mono": "Fira Code Variable"
      },
      "radius": {
        "base": "0.5rem"
      },
      "motion": {
        "interactive": "200ms",
        "easing": "cubic-bezier(0.16, 1, 0.3, 1)"
      },
      "canvas": {
        "bg": "oklch(16.6% 0.027 283.1)",
        "primary": "oklch(60.3% 0.244 0.4)",
        "accent": "oklch(50% 0.23 310)",
        "highlight": "oklch(94.8% 0.011 280.5)"
      }
    },
    "morfo": {
      "id": "morfo",
      "name": "Morfo Academy",
      "family": "morfo",
      "status": "hypothese",
      "light": {
        "background": "#f8f8f9",
        "foreground": "#1e1f21",
        "card": "#eeeeef",
        "primary": "#005ae7",
        "muted": "#eeeeef",
        "mutedForeground": "#66696e",
        "border": "#c3c4c6",
        "ring": "#005ae7",
        "accent": "#2c91bc",
        "primaryForeground": "#ffffff",
        "accentForeground": "#0a0a0a"
      },
      "dark": {
        "background": "#0d0d0e",
        "foreground": "#f8f8f9",
        "card": "#1e1f21",
        "primary": "#6fa3ff",
        "muted": "#1e1f21",
        "mutedForeground": "#a3a5a8",
        "border": "#313336",
        "ring": "#6fa3ff",
        "accent": "#6eaece",
        "primaryForeground": "#0a0a0a",
        "accentForeground": "#0a0a0a"
      },
      "ramps": {
        "primary": {
          "50": "#f5f9ff",
          "100": "#e6efff",
          "200": "#ccdfff",
          "300": "#a3c5ff",
          "400": "#6fa3ff",
          "500": "#377efe",
          "600": "#005ae7",
          "700": "#0042ad",
          "800": "#002b77",
          "900": "#001a4f",
          "950": "#000a2a"
        },
        "accent": {
          "50": "#f3fafd",
          "100": "#e5f1f7",
          "200": "#cce2ee",
          "300": "#a4cbdf",
          "400": "#6eaece",
          "500": "#2c91bc",
          "600": "#00739a",
          "700": "#005573",
          "800": "#00384e",
          "900": "#002332",
          "950": "#000f18"
        },
        "neutral": {
          "50": "#f8f8f9",
          "100": "#eeeeef",
          "200": "#dddedf",
          "300": "#c3c4c6",
          "400": "#a3a5a8",
          "500": "#84868a",
          "600": "#66696e",
          "700": "#4b4d51",
          "800": "#313336",
          "900": "#1e1f21",
          "950": "#0d0d0e"
        }
      },
      "font": {
        "display": "Space Grotesk Variable",
        "body": "Inter Variable",
        "mono": null
      },
      "radius": {
        "base": "0.5rem"
      },
      "motion": {
        "interactive": "180ms",
        "easing": "cubic-bezier(0.16, 1, 0.3, 1)"
      },
      "canvas": {
        "bg": "oklch(18% 0.02 261)",
        "primary": "oklch(56% 0.24 261)",
        "accent": "oklch(72% 0.13 231)",
        "highlight": "oklch(88% 0.03 240)"
      }
    },
    "swihn": {
      "id": "swihn",
      "name": "swihn.art",
      "family": "swihn",
      "status": "existante",
      "light": {
        "background": "#ffffff",
        "foreground": "#1f1f1f",
        "card": "#f8f8f8",
        "primary": "#a900af",
        "muted": "#eeeeee",
        "mutedForeground": "#696969",
        "border": "#c4c4c4",
        "ring": "#a900af",
        "accent": "#357eff",
        "primaryForeground": "#ffffff",
        "accentForeground": "#0a0a0a"
      },
      "dark": {
        "background": "#000000",
        "foreground": "#f8f8f8",
        "card": "#1f1f1f",
        "primary": "#e271e5",
        "muted": "#1f1f1f",
        "mutedForeground": "#a4a4a4",
        "border": "#333333",
        "ring": "#e271e5",
        "accent": "#6fa3ff",
        "primaryForeground": "#0a0a0a",
        "accentForeground": "#0a0a0a"
      },
      "ramps": {
        "primary": {
          "50": "#fff5ff",
          "100": "#fde6fc",
          "200": "#f8cef7",
          "300": "#eea8ee",
          "400": "#e271e5",
          "500": "#d320d9",
          "600": "#a900af",
          "700": "#7e0083",
          "800": "#560059",
          "900": "#38003a",
          "950": "#1c001d"
        },
        "accent": {
          "50": "#f5f9ff",
          "100": "#e6efff",
          "200": "#ccdfff",
          "300": "#a3c5ff",
          "400": "#6fa3ff",
          "500": "#357eff",
          "600": "#005be5",
          "700": "#0042ac",
          "800": "#002b77",
          "900": "#001a4e",
          "950": "#000a2a"
        },
        "neutral": {
          "50": "#f8f8f8",
          "100": "#eeeeee",
          "200": "#dedede",
          "300": "#c4c4c4",
          "400": "#a4a4a4",
          "500": "#868686",
          "600": "#696969",
          "700": "#4d4d4d",
          "800": "#333333",
          "900": "#1f1f1f",
          "950": "#0d0d0d"
        }
      },
      "font": {
        "display": "EB Garamond",
        "body": "Inter Variable",
        "mono": "Fira Code Variable"
      },
      "radius": {
        "base": "0.5rem"
      },
      "motion": {
        "interactive": "180ms",
        "easing": "cubic-bezier(0.16, 1, 0.3, 1)"
      },
      "canvas": {
        "bg": "oklch(0% 0 0)",
        "primary": "oklch(69% 0.320 326.6)",
        "accent": "oklch(56.3% 0.241 260.8)",
        "highlight": "oklch(52.5% 0.293 292.3)"
      }
    }
  }
}""")

VERSION = _DATA["version"]
BASE = _DATA["base"]
BRANDS = _DATA["brands"]


def brand(brand_id):
    """Dictionnaire complet d'une marque (light, dark, ramps, font, radius, motion, canvas)."""
    return BRANDS[brand_id]


def semantic(brand_id, theme="light"):
    """Table sémantique résolue (background, foreground, primary, …) pour un thème."""
    return BRANDS[brand_id][theme]


def ramp(brand_id, name):
    """Rampe 50-950 : name ∈ primary | accent | neutral."""
    return BRANDS[brand_id]["ramps"][name]


def _kebab(key):
    out = []
    for ch in key:
        out.append("-" + ch.lower() if ch.isupper() else ch)
    return "".join(out)


def css_vars(brand_id, theme="light"):
    """Variables CSS (nom → valeur) équivalentes à brands/<marque>.css pour ce thème."""
    b = BRANDS[brand_id]
    out = {}
    for group in ("text", "space", "radius", "content"):
        for k, v in BASE.get(group, {}).items():
            out[f"--{group}-{k}"] = v
    out["--radius"] = b["radius"]["base"]
    out["--font-display"] = f'"{b["font"]["display"]}"'
    out["--font-body"] = f'"{b["font"]["body"]}"'
    if b["font"].get("mono"):
        out["--font-mono"] = f'"{b["font"]["mono"]}"'
    for name, r in b["ramps"].items():
        for step, hexv in r.items():
            out[f"--brand-{name}-{step}"] = hexv
    for k, v in b[theme].items():
        out[f"--{_kebab(k)}"] = v
    for i, k in enumerate(("bg", "primary", "accent", "highlight")):
        out[f"--canvas-{i}"] = b["canvas"][k]
    return out
