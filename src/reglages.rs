//! Catalogue des réglages : source unique pour l'interface (titres, explications) et pour le LLM
//! (valeurs permises). Un réglage absent ou invalide prend sa valeur par défaut.

use std::collections::BTreeMap;

use serde_json::{json, Value};

pub type Reglages = BTreeMap<String, Value>;

pub enum Genre {
    Choix(&'static [(&'static str, &'static str)]),
    Interrupteur,
    Curseur { min: f64, max: f64 },
    /// Un son de la médiathèque, ou "" pour aucun.
    MediaSon,
}

pub struct Reglage {
    pub section: &'static str,
    pub cle: &'static str,
    pub titre: &'static str,
    pub aide: &'static str,
    pub genre: Genre,
    pub defaut: fn() -> Value,
}

pub const CATALOGUE: &[Reglage] = &[
    Reglage {
        section: "Autocut",
        cle: "coupe_silences",
        titre: "Couper les silences",
        aide: "Retire les blancs entre les mots. C'est la base d'un montage rythmé : le spectateur décroche dès qu'un silence s'installe.",
        genre: Genre::Interrupteur,
        defaut: || json!(true),
    },
    Reglage {
        section: "Autocut",
        cle: "rythme",
        titre: "Rythme",
        aide: "Longueur des silences gardés à l'intérieur d'une phrase. Les moments d'émotion gardent toujours plus d'air.",
        genre: Genre::Choix(&[
            ("ample", "Ample"),
            ("equilibre", "Équilibré"),
            ("nerveux", "Nerveux"),
        ]),
        defaut: || json!("equilibre"),
    },
    Reglage {
        section: "Autocut",
        cle: "coupe_hesitations",
        titre: "Couper les hésitations",
        aide: "Retire les « euh » et « hum » repérés dans la transcription.",
        genre: Genre::Interrupteur,
        defaut: || json!(true),
    },
    Reglage {
        section: "Autocut",
        cle: "coupe_redites",
        titre: "Couper les redites",
        aide: "Le LLM retire répétitions, faux départs et digressions. Plus c'est fort, plus la vidéo raccourcit.",
        genre: Genre::Choix(&[
            ("aucune", "Aucune"),
            ("legere", "Légère"),
            ("normale", "Normale"),
            ("forte", "Forte"),
        ]),
        defaut: || json!("normale"),
    },
    Reglage {
        section: "Zooms",
        cle: "zooms",
        titre: "Intensité des zooms",
        aide: "Le cadre se resserre sur le visage selon l'importance de la phrase. « Aucun » garde le plan d'origine.",
        genre: Genre::Choix(&[
            ("aucun", "Aucun"),
            ("doux", "Doux"),
            ("normal", "Normal"),
            ("fort", "Fort"),
        ]),
        defaut: || json!("normal"),
    },
    Reglage {
        section: "Zooms",
        cle: "yeux",
        titre: "Gros plans sur les yeux",
        aide: "Très gros plans sur le regard, réservés aux deux ou trois phrases les plus intenses.",
        genre: Genre::Interrupteur,
        defaut: || json!(true),
    },
    Reglage {
        section: "Zooms",
        cle: "zoom_anime",
        titre: "Zoom qui avance",
        aide: "Pendant une phrase forte, le cadre avance lentement vers le visage.",
        genre: Genre::Interrupteur,
        defaut: || json!(true),
    },
    Reglage {
        section: "Zooms",
        cle: "punch_jumpcut",
        titre: "Recadrer à chaque coupe",
        aide: "Quand une phrase est coupée en deux, le cadre se resserre un peu : la coupe passe pour un effet voulu.",
        genre: Genre::Interrupteur,
        defaut: || json!(true),
    },
    Reglage {
        section: "Transitions",
        cle: "transition",
        titre: "Style de transition",
        aide: "Effet joué au début de chaque partie de la vidéo. « Au choix du LLM » varie selon l'ambiance.",
        genre: Genre::Choix(&[
            ("coupe", "Coupe franche"),
            ("flash", "Flash blanc"),
            ("fondu", "Fondu au noir"),
            ("zoom", "Zoom"),
            ("glisse", "Glissé flou"),
            ("glitch", "Glitch"),
            ("auto", "Au choix du LLM"),
        ]),
        defaut: || json!("auto"),
    },
    Reglage {
        section: "Transitions",
        cle: "bruitage",
        titre: "Bruitage de transition",
        aide: "Un souffle (« whoosh ») accompagne chaque transition.",
        genre: Genre::Interrupteur,
        defaut: || json!(true),
    },
    Reglage {
        section: "Structure",
        cle: "accroche",
        titre: "Accroche en ouverture",
        aide: "Les phrases les plus fortes sont montées au tout début, pour accrocher dans les cinq premières secondes.",
        genre: Genre::Interrupteur,
        defaut: || json!(true),
    },
    Reglage {
        section: "Structure",
        cle: "titres_chapitres",
        titre: "Titres de partie",
        aide: "Un titre court s'affiche au début de chaque partie.",
        genre: Genre::Interrupteur,
        defaut: || json!(true),
    },
    Reglage {
        section: "Sous-titres",
        cle: "sous_titres",
        titre: "Sous-titres",
        aide: "La plupart des vidéos sont regardées sans le son : les sous-titres retiennent le spectateur.",
        genre: Genre::Interrupteur,
        defaut: || json!(true),
    },
    Reglage {
        section: "Sous-titres",
        cle: "style_sous_titres",
        titre: "Style",
        aide: "« Dynamique » : gros, mot à mot, mots forts en couleur. « Sobre » : la phrase, simplement.",
        genre: Genre::Choix(&[("dynamique", "Dynamique"), ("sobre", "Sobre")]),
        defaut: || json!("dynamique"),
    },
    Reglage {
        section: "Sous-titres",
        cle: "position_sous_titres",
        titre: "Position",
        aide: "En bas, ils ne cachent jamais le visage ; au milieu, ils captent davantage le regard.",
        genre: Genre::Choix(&[("bas", "En bas"), ("milieu", "Au milieu")]),
        defaut: || json!("bas"),
    },
    Reglage {
        section: "Image",
        cle: "filtre",
        titre: "Filtre",
        aide: "Couleur générale de la vidéo.",
        genre: Genre::Choix(&[
            ("naturel", "Naturel"),
            ("eclatant", "Éclatant"),
            ("chaud", "Chaud"),
            ("froid", "Froid"),
            ("cinema", "Cinéma"),
            ("noir_blanc", "Noir et blanc"),
            ("vintage", "Vintage"),
        ]),
        defaut: || json!("naturel"),
    },
    Reglage {
        section: "Image",
        cle: "nettete_ia",
        titre: "Netteté IA",
        aide: "Recrée de la définition sur les plans zoomés (Real-ESRGAN). L'option « visage » restaure aussi les traits (GFPGAN), au risque de les retoucher un peu. Appliquée au rendu final seulement : l'aperçu reste rapide.",
        genre: Genre::Choix(&[
            ("non", "Non"),
            ("auto", "Sur les zooms"),
            ("visage", "Zooms + visage"),
        ]),
        defaut: || json!("auto"),
    },
    Reglage {
        section: "Image",
        cle: "format",
        titre: "Format",
        aide: "Vertical pour Shorts et Reels, horizontal pour YouTube. Le cadre suit le visage si le format change.",
        genre: Genre::Choix(&[
            ("auto", "Comme la source"),
            ("vertical", "Vertical 9:16"),
            ("horizontal", "Horizontal 16:9"),
            ("carre", "Carré 1:1"),
        ]),
        defaut: || json!("auto"),
    },
    Reglage {
        section: "Son",
        cle: "debruitage",
        titre: "Réduction du bruit",
        aide: "Atténue le souffle, la ventilation, le bruit de la pièce.",
        genre: Genre::Choix(&[("non", "Non"), ("leger", "Légère"), ("fort", "Forte")]),
        defaut: || json!("leger"),
    },
    Reglage {
        section: "Son",
        cle: "voix",
        titre: "Voix plus présente",
        aide: "Coupe les graves parasites, éclaircit la voix et lisse les écarts de volume.",
        genre: Genre::Interrupteur,
        defaut: || json!(true),
    },
    Reglage {
        section: "Son",
        cle: "deesseur",
        titre: "Adoucir les « s »",
        aide: "Atténue les sifflantes trop aiguës.",
        genre: Genre::Interrupteur,
        defaut: || json!(false),
    },
    Reglage {
        section: "Son",
        cle: "musique",
        titre: "Musique de fond",
        aide: "Un son de la médiathèque, joué en boucle sous la voix.",
        genre: Genre::MediaSon,
        defaut: || json!(""),
    },
    Reglage {
        section: "Son",
        cle: "volume_musique",
        titre: "Volume de la musique",
        aide: "La musique baisse toute seule quand la personne parle.",
        genre: Genre::Curseur { min: 0.0, max: 100.0 },
        defaut: || json!(20),
    },
];

/// Réglages lus par le LLM au moment du montage : les changer demande de remonter.
pub const DEMANDENT_REMONTAGE: &[&str] = &["coupe_redites", "accroche"];

pub fn chercher(cle: &str) -> Option<&'static Reglage> {
    CATALOGUE.iter().find(|r| r.cle == cle)
}

/// Valeur acceptée pour ce réglage, ou un message qui dit pourquoi non.
pub fn valider(cle: &str, valeur: &Value) -> Result<Value, String> {
    let r = chercher(cle).ok_or_else(|| format!("réglage inconnu : {cle}"))?;
    match &r.genre {
        Genre::Choix(options) => {
            let v = valeur.as_str().unwrap_or_default();
            if options.iter().any(|(o, _)| *o == v) {
                Ok(json!(v))
            } else {
                Err(format!("valeur « {v} » impossible pour « {} »", r.titre))
            }
        }
        Genre::Interrupteur => valeur
            .as_bool()
            .map(Value::Bool)
            .ok_or_else(|| format!("« {} » attend oui ou non", r.titre)),
        Genre::Curseur { min, max } => valeur
            .as_f64()
            .map(|v| json!(v.clamp(*min, *max).round()))
            .ok_or_else(|| format!("« {} » attend un nombre", r.titre)),
        Genre::MediaSon => Ok(json!(valeur.as_str().unwrap_or_default())),
    }
}

/// Complète les réglages manquants ou invalides par leur valeur par défaut.
pub fn completer(reglages: &mut Reglages) {
    for r in CATALOGUE {
        let ok = reglages.get(r.cle).map(|v| valider(r.cle, v).is_ok()).unwrap_or(false);
        if !ok {
            reglages.insert(r.cle.to_string(), (r.defaut)());
        }
    }
    reglages.retain(|k, _| chercher(k).is_some());
}

pub fn defauts() -> Reglages {
    let mut r = Reglages::new();
    completer(&mut r);
    r
}

pub fn texte(r: &Reglages, cle: &str) -> String {
    r.get(cle).and_then(Value::as_str).unwrap_or_default().to_string()
}

pub fn vrai(r: &Reglages, cle: &str) -> bool {
    r.get(cle).and_then(Value::as_bool).unwrap_or(false)
}

pub fn nombre(r: &Reglages, cle: &str) -> f64 {
    r.get(cle).and_then(Value::as_f64).unwrap_or(0.0)
}

/// Le catalogue tel que l'interface l'affiche.
pub fn catalogue_json() -> Value {
    Value::Array(
        CATALOGUE
            .iter()
            .map(|r| {
                let (genre, options, bornes) = match &r.genre {
                    Genre::Choix(o) => (
                        "choix",
                        o.iter().map(|(v, l)| json!({"valeur": v, "libelle": l})).collect(),
                        Value::Null,
                    ),
                    Genre::Interrupteur => ("interrupteur", vec![], Value::Null),
                    Genre::Curseur { min, max } => ("curseur", vec![], json!({"min": min, "max": max})),
                    Genre::MediaSon => ("media_son", vec![], Value::Null),
                };
                json!({"section": r.section, "cle": r.cle, "titre": r.titre, "aide": r.aide, "genre": genre,
                       "options": options, "bornes": bornes, "defaut": (r.defaut)(),
                       "remontage": DEMANDENT_REMONTAGE.contains(&r.cle)})
            })
            .collect(),
    )
}

/// Description compacte pour le LLM : clé, valeurs permises, sens.
pub fn pour_llm(actuels: &Reglages) -> String {
    CATALOGUE
        .iter()
        .map(|r| {
            let permis = match &r.genre {
                Genre::Choix(o) => o.iter().map(|(v, _)| *v).collect::<Vec<_>>().join(" | "),
                Genre::Interrupteur => "true | false".into(),
                Genre::Curseur { min, max } => format!("nombre de {min} à {max}"),
                Genre::MediaSon => "nom d'un son de la médiathèque, ou \"\" pour aucun".into(),
            };
            format!(
                "- {} = {} (permis : {}) — {} : {}",
                r.cle,
                actuels.get(r.cle).cloned().unwrap_or(Value::Null),
                permis,
                r.titre,
                r.aide
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Schéma JSON des réglages modifiables, pour la sortie structurée du LLM.
pub fn schema_llm() -> Value {
    let mut proprietes = serde_json::Map::new();
    for r in CATALOGUE {
        let s = match &r.genre {
            Genre::Choix(o) => json!({"type": "string", "enum": o.iter().map(|(v, _)| *v).collect::<Vec<_>>()}),
            Genre::Interrupteur => json!({"type": "boolean"}),
            Genre::Curseur { .. } => json!({"type": "number"}),
            Genre::MediaSon => json!({"type": "string"}),
        };
        proprietes.insert(r.cle.to_string(), s);
    }
    json!({"type": "object", "properties": proprietes})
}
