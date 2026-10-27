//! Catalogue des réglages : source unique pour l'interface (titres, explications) et pour le LLM
//! (valeurs permises). Un réglage absent ou invalide prend sa valeur par défaut.

use std::collections::BTreeMap;

use serde_json::{json, Value};

pub type Reglages = BTreeMap<String, Value>;

pub enum Genre {
    Choix(&'static [(&'static str, &'static str)]),
    Interrupteur,
    Curseur { min: f64, max: f64, unite: &'static str },
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
        aide: "Pendant une phrase forte, le cadre avance d'un cran vers le visage, en un mouvement continu.",
        genre: Genre::Interrupteur,
        defaut: || json!(true),
    },
    Reglage {
        section: "Zooms",
        cle: "ecart_cadrage",
        titre: "Écart entre deux cadrages",
        aide: "Le pas entre deux cadrages qui se suivent. Entre deux changements, le cadre ne bouge pas ; quand il change, c'est d'un pas entier, assez grand pour se voir comme voulu (20 % au moins), jamais d'un micro-recadrage. Les transitions restent libres.",
        genre: Genre::Curseur { min: 20.0, max: 60.0, unite: "%" },
        defaut: || json!(30),
    },
    Reglage {
        section: "Zooms",
        cle: "punch_jumpcut",
        titre: "Recadrer à chaque coupe",
        aide: "Quand une phrase est coupée en deux, le cadre change d'un cran franc : la coupe passe pour un effet voulu. Pas plus d'un changement toutes les deux secondes et demie.",
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
        cle: "fondu_ouverture",
        titre: "Fondu d'ouverture",
        aide: "La vidéo s'ouvre en sortant du noir ou du blanc, avant la première séquence (générique compris), image et son ensemble.",
        genre: Genre::Choix(&[("aucun", "Aucun"), ("noir", "Depuis le noir"), ("blanc", "Depuis le blanc")]),
        defaut: || json!("aucun"),
    },
    Reglage {
        section: "Transitions",
        cle: "fondu_fin",
        titre: "Fondu de fin",
        aide: "La dernière image s'efface vers le noir ou le blanc, et le son avec elle.",
        genre: Genre::Choix(&[("aucun", "Aucun"), ("noir", "Vers le noir"), ("blanc", "Vers le blanc")]),
        defaut: || json!("aucun"),
    },
    Reglage {
        section: "Transitions",
        cle: "duree_fondus",
        titre: "Durée des fondus d'ouverture et de fin",
        aide: "En secondes, au dixième : 0,2 à 0,5 s pour un fondu bref, 1 à 3 s pour une ouverture ou une fin lentes.",
        genre: Genre::Curseur { min: 0.2, max: 3.0, unite: "s" },
        defaut: || json!(1.0),
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
        cle: "nettete_ia",
        titre: "Netteté IA",
        aide: "Expérimental. Redessine les plans zoomés par IA (Real-ESRGAN) ; l'option « visage » retouche aussi les traits (GFPGAN). Sur une captation iPhone, l'IA lisse la peau et donne un aspect peint : essayez-la d'abord sur un plan avec « Ce plan en qualité finale ». Sans elle, les zooms sont agrandis au plus près des pixels du rush.",
        genre: Genre::Choix(&[
            ("non", "Non (fidèle au rush)"),
            ("auto", "Sur les zooms (expérimental)"),
            ("visage", "Zooms + visage (expérimental)"),
        ]),
        defaut: || json!("non"),
    },
    Reglage {
        section: "Image",
        cle: "rognage_haut",
        titre: "Rogner le haut de la source",
        aide: "Retire une bande en haut de l'image d'origine sur tous les plans : un logo (CapCut, TikTok) ou un bord abîmé disparaît.",
        genre: Genre::Curseur { min: 0.0, max: 20.0, unite: "%" },
        defaut: || json!(0),
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
        section: "Image",
        cle: "cadence",
        titre: "Cadence du rendu final",
        aide: "Comme le rush : 60 images par seconde pour un iPhone réglé en 60, mouvement fluide. 30 i/s : fichier plus léger.",
        genre: Genre::Choix(&[("source", "Comme le rush"), ("30", "30 i/s")]),
        defaut: || json!("source"),
    },
    Reglage {
        section: "Image",
        cle: "plage_dynamique",
        titre: "Plage dynamique du rendu final",
        aide: "HDR comme le rush : l'image de l'iPhone (HLG) sans conversion, hautes lumières et couleurs intactes sur iPhone, Mac et YouTube ; la couleur de la PostProd n'y est pas appliquée et l'aperçu reste en SDR. SDR : lu partout, couleurs de la PostProd appliquées.",
        genre: Genre::Choix(&[("sdr", "SDR"), ("hdr", "HDR comme le rush")]),
        defaut: || json!("sdr"),
    },
    Reglage {
        section: "Image",
        cle: "codec_final",
        titre: "Format du rendu final",
        aide: "HEVC 10 bits : un seul encodage, dégradés sans marches, lu par l'iPhone, le Mac et YouTube ; certains navigateurs ne le lisent pas. H.264 : lu partout, en 8 bits.",
        genre: Genre::Choix(&[("hevc", "HEVC 10 bits"), ("h264", "H.264")]),
        defaut: || json!("hevc"),
    },
    Reglage {
        section: "Image",
        cle: "definition",
        titre: "Définition du rendu final",
        aide: "1080p convient à presque tout, et laisse aux zooms les vrais pixels d'un rush 4K jusqu'à ×2. En 4K, chaque zoom agrandit l'image du rush : les zooms sont alors limités à ×1,5. Le rendu 4K est plus long et plus lourd.",
        genre: Genre::Choix(&[("1080", "1080p (Full HD)"), ("2160", "2160p (4K)")]),
        defaut: || json!("1080"),
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
        cle: "timbre_voix",
        titre: "Timbre de la voix",
        aide: "Avec « Voix plus présente ». Studio chaleureux : chaîne de studio (nettoyage, chaleur, présence, air, deux compressions douces, sifflantes, rondeur), à combiner au timbre de référence. Clair : voix en avant, aigus soulignés. Chaud : plus de corps, aigus adoucis. Naturel : seulement lissée. \
               Podcast : voix de studio, très régulière. Graves renforcés, Aigus adoucis : une seule correction. Radio, Téléphone : effets de style.",
        genre: Genre::Choix(&[("studio_chaud", "Studio chaleureux"), ("clair", "Clair et présent"), ("chaud", "Chaud et profond"), ("naturel", "Naturel"), ("podcast", "Podcast"),
                              ("graves", "Graves renforcés"), ("aigus_doux", "Aigus adoucis"), ("radio", "Radio"), ("telephone", "Téléphone"), ("proche", "Proche"),
                              ("exterieur", "Extérieur")]),
        defaut: || json!("clair"),
    },
    Reglage {
        section: "Son",
        cle: "reverberation",
        titre: "Écho de la pièce",
        aide: "Pièce qui résonne (murs nus, carrelage) : baisse la traîne qui suit chaque phrase. Forte : plus net, mais peut raboter la fin des mots doux. \
               L'écho mêlé à la voix pendant qu'elle parle reste.",
        genre: Genre::Choix(&[("non", "Non"), ("attenuer", "Atténuer"), ("forte", "Atténuer fortement")]),
        defaut: || json!("non"),
    },
    Reglage {
        section: "Son",
        cle: "dereverberation",
        titre: "Retirer l'écho de la voix",
        aide: "Sèche la voix par IA : la pièce (réverbération) et le bruit retirés, pendant que la voix parle comme entre les phrases, sans \
               changer les mots (TF-GridNet, qui estime la voix sèche sans la recréer ; une à deux minutes par rush sur la carte graphique, la \
               première fois). Douce : en partie, le son garde de l'air.",
        genre: Genre::Choix(&[("non", "Non"), ("douce", "Douce"), ("forte", "Forte")]),
        defaut: || json!("non"),
    },
    Reglage {
        section: "Son",
        cle: "reference_timbre",
        titre: "Timbre de référence",
        aide: "Égaliseur par correspondance : le spectre moyen de votre voix est ramené vers celui d'une référence. « Cette voix, sans la pièce » : \
               votre propre voix reconstruite en studio par IA (on n'en garde que la couleur, jamais le son). « Voix de studio » : une voix parlée \
               enregistrée en studio. « Un son de la médiathèque » : votre enregistrement de référence (bon micro).",
        genre: Genre::Choix(&[("aucune", "Aucune"), ("cette_voix", "Cette voix, sans la pièce"), ("studio", "Voix de studio"), ("media", "Un son de la médiathèque")]),
        defaut: || json!("aucune"),
    },
    Reglage {
        section: "Son",
        cle: "reference_media",
        titre: "Son de référence",
        aide: "Avec « Un son de la médiathèque » : l'enregistrement dont on emprunte la couleur.",
        genre: Genre::MediaSon,
        defaut: || json!(""),
    },
    Reglage {
        section: "Son",
        cle: "intensite_timbre",
        titre: "Intensité du timbre de référence",
        aide: "100 % : la correction complète (jusqu'à ±8 dB par bande). 60 % emprunte la couleur sans copier la voix de référence.",
        genre: Genre::Curseur { min: 0.0, max: 100.0, unite: "%" },
        defaut: || json!(60),
    },
    Reglage {
        section: "Son",
        cle: "espace",
        titre: "Acoustique d'un lieu",
        aide: "Prête à la voix la résonance d'un lieu : un studio (très court, enveloppant, après avoir séché la voix), une petite pièce, une salle de conférence, une grande salle, une cathédrale.",
        genre: Genre::Choix(&[("aucun", "Aucune"), ("studio", "Studio (enveloppant)"), ("chambre", "Petite pièce"), ("conference", "Salle de conférence"),
                              ("salle", "Grande salle"), ("cathedrale", "Cathédrale")]),
        defaut: || json!("aucun"),
    },
    Reglage {
        section: "Son",
        cle: "restauration",
        titre: "Voix de studio (IA)",
        aide: "La voix est séchée (TF-GridNet), puis refaite en qualité studio par VoiceFixer, à partir de deux voix séchées différentes. \
               Whisper vérifie chaque tranche de 1 à 4 s : on garde une version dont les mots sont ceux du rush ; s'il n'y en a pas (un nom \
               propre déformé, un « et » avalé), la tranche garde la voix séchée, égalisée vers le même timbre. Quelques minutes par rush sur \
               la carte graphique, la première fois.",
        genre: Genre::Interrupteur,
        defaut: || json!(false),
    },
    Reglage {
        section: "Son",
        cle: "harmoniques",
        titre: "Harmoniques",
        aide: "Ajoute à la voix des harmoniques dans les aigus (excitateur) : plus de présence et d'air, une voix qui « brille ». \
               À doser : fortes, elles rendent la voix agressive.",
        genre: Genre::Choix(&[("non", "Aucune"), ("leger", "Légères"), ("moyen", "Moyennes"), ("fort", "Fortes")]),
        defaut: || json!("non"),
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
        cle: "calage_son",
        titre: "Calage son/image",
        aide: "Si la voix et les lèvres sont décalées dans vos rushs eux-mêmes (micro externe, vidéo exportée d'une application) : un nombre positif retarde le son, négatif l'avance.",
        genre: Genre::Curseur { min: -300.0, max: 300.0, unite: "ms" },
        defaut: || json!(0),
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
        genre: Genre::Curseur { min: 0.0, max: 100.0, unite: "%" },
        defaut: || json!(20),
    },
];

/// Réglages lus par le LLM au moment du montage : les changer demande de remonter.
pub const DEMANDENT_REMONTAGE: &[&str] = &["coupe_redites", "accroche"];

pub fn chercher(cle: &str) -> Option<&'static Reglage> {
    CATALOGUE.iter().find(|r| r.cle == cle)
}

/// Libellé affiché d'une valeur d'un réglage à choix (« leger » → « Légère »).
pub fn libelle_choix(cle: &str, valeur: &str) -> Option<String> {
    match chercher(cle)?.genre {
        Genre::Choix(choix) => choix.iter().find(|(v, _)| *v == valeur).map(|(_, l)| l.to_string()),
        _ => None,
    }
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
        Genre::Curseur { min, max, unite } => valeur
            .as_f64()
            // Entier : 30 et 30.0 seraient deux valeurs différentes pour la comparaison des réglages. Les durées en
            // secondes se règlent au dixième.
            .map(|v| if *unite == "s" { json!((v.clamp(*min, *max) * 10.0).round() / 10.0) } else { json!(v.clamp(*min, *max).round() as i64) })
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
    accorder_son(reglages);
}

/// Les réglages du son qui se contredisent sont accordés d'office (renvoie ce qui a changé, pour le dire) :
/// - la reconstruction du timbre part toujours de la voix débruitée à fond : retrait de l'écho « Forte » ;
/// - le débruiteur IA retire déjà le bruit : la réduction du bruit classique (afftdn), qui ajoutait un son
///   métallique par-dessus, s'éteint.
pub fn accorder_son(r: &mut Reglages) -> Vec<String> {
    let mut faits = Vec::new();
    if vrai(r, "restauration") && texte(r, "dereverberation") != "forte" {
        r.insert("dereverberation".into(), json!("forte"));
        faits.push("Retirer l'écho de la voix → Forte (la reconstruction du timbre part de la voix débruitée à fond)".to_string());
    }
    if texte(r, "dereverberation") != "non" && texte(r, "debruitage") != "non" {
        r.insert("debruitage".into(), json!("non"));
        faits.push("Réduction du bruit → Non (l'IA retire déjà le bruit)".to_string());
    }
    faits
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
                    Genre::Curseur { min, max, unite } => ("curseur", vec![], json!({"min": min, "max": max, "unite": unite})),
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
                Genre::Curseur { min, max, unite } => format!("nombre de {min} à {max} ({unite})"),
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
