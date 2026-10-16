//! Le seul LLM d'EDIVID : il monte, se relit, et répond dans la conversation par des actions.
//! Réflexion décidée ici, pas par l'utilisateur : montage et critique avec, conversation sans.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

use crate::decoupe::court;
use crate::etat::EtatPartage;
use crate::modele::{maintenant, Chapitre, Decision, Montage, Phrase, Projet};
use crate::reglages::{self, texte, vrai};

const REGIMES: &[&str] = &["respiration", "rafale", "impact"];
const CADRES: &[&str] = &["large", "moyen", "visage", "yeux"];
const TRANSITIONS: &[&str] = &["flash", "fondu", "zoom", "glisse", "glitch"];
const POSITIONS: &[&str] = &["plein", "centre", "haut_gauche", "haut_droite", "bas_gauche", "bas_droite"];

pub async fn appeler(etat: &EtatPartage, projet: &str, systeme: &str, contenu: &str, schema: &Value,
                     reflexion: bool) -> Result<Value> {
    etat.carte.llm_debut(etat, projet).await?;
    let reponse = etat
        .http
        .post(format!("{}/api/chat", etat.cfg.ollama))
        .json(&json!({
            "model": etat.cfg.modele_llm, "stream": false, "think": reflexion, "format": schema,
            "keep_alive": "10m",
            "options": {"num_ctx": 32768, "temperature": 0.4},
            "messages": [{"role": "system", "content": systeme}, {"role": "user", "content": contenu}]
        }))
        .timeout(Duration::from_secs(3600))
        .send()
        .await;
    etat.carte.llm_fin().await;
    let corps: Value = reponse.context("Ollama injoignable")?.json().await?;
    if let Some(e) = corps.get("error") {
        return Err(anyhow!("Ollama : {e}"));
    }
    let message = corps["message"]["content"].as_str().unwrap_or_default();
    serde_json::from_str(message).with_context(|| format!("réponse du LLM illisible : {}", court(message)))
}

fn schema_montage() -> Value {
    json!({
        "type": "object",
        "properties": {
            "accroche": {"type": "array", "items": {"type": "integer"}},
            "chapitres": {"type": "array", "items": {"type": "object", "properties": {
                "phrase": {"type": "integer"}, "titre": {"type": "string"}}, "required": ["phrase", "titre"]}},
            "phrases": {"type": "array", "items": {"type": "object", "properties": {
                "id": {"type": "integer"},
                "garder": {"type": "boolean"},
                "regime": {"type": "string", "enum": REGIMES},
                "cadre": {"type": "string", "enum": CADRES},
                "transition": {"anyOf": [{"type": "string", "enum": TRANSITIONS}, {"type": "null"}]},
                "mots_forts": {"type": "array", "items": {"type": "string"}}},
                "required": ["id", "garder", "regime", "cadre", "transition", "mots_forts"]}},
            "notes": {"type": "array", "items": {"type": "string"}}
        },
        "required": ["accroche", "chapitres", "phrases", "notes"]
    })
}

fn consigne_monteur(p: &Projet, plusieurs_rushs: bool) -> String {
    let r = &p.reglages;
    let redites = match texte(r, "coupe_redites").as_str() {
        "aucune" => "Ne coupe presque rien : seulement les phrases ratées, inaudibles ou reprises aussitôt.",
        "legere" => "Coupe peu : vise à retirer environ 10 à 15 % de la durée parlée.",
        "forte" => "Coupe franchement : vise à retirer environ 35 à 45 % de la durée parlée, sans perdre une idée essentielle.",
        _ => "Vise à retirer environ 20 à 30 % de la durée parlée, jamais une idée essentielle.",
    };
    let cadres = if texte(r, "zooms") == "aucun" {
        "cadre : toujours \"large\" (les zooms sont désactivés).".to_string()
    } else {
        let yeux = if vrai(r, "yeux") {
            "\"yeux\" (très gros plan sur le regard — rare, réservé aux deux ou trois phrases les plus intenses de toute la vidéo)"
        } else {
            "jamais \"yeux\" (désactivé)"
        };
        format!("cadre : \"large\" (image entière), \"moyen\" (punch-in léger), \"visage\" (gros plan), {yeux}. \
                 En \"rafale\", ne garde jamais le même cadre deux phrases de suite. \"impact\" prend \"visage\" ou \"yeux\". \
                 \"respiration\" prend \"large\" ou \"moyen\".")
    };
    let transitions = match texte(r, "transition").as_str() {
        "coupe" => "transition : toujours null (coupes franches).".to_string(),
        "auto" => "transition : au début d'une nouvelle partie seulement (changement de sujet net), sinon null ; trois à six au total. \
                   Choisis le style selon l'ambiance : \"flash\" (énergique), \"fondu\" (émotion, gravité), \"zoom\" (révélation), \
                   \"glisse\" (enchaînement), \"glitch\" (humour, décalage)."
            .to_string(),
        s => format!("transition : \"{s}\" au début d'une nouvelle partie seulement (changement de sujet net), sinon null ; trois à six au total."),
    };
    let accroche = if vrai(r, "accroche") {
        "accroche : une à trois phrases (numéros) parmi les plus fortes de la vidéo, montées en ouverture pour accrocher dans les \
         cinq premières secondes ; durée totale sous 8 secondes. Choisis-les APRÈS la première minute : l'accroche ne doit jamais \
         redire l'introduction, qui suit immédiatement."
    } else {
        "accroche : liste vide (désactivée)."
    };
    let rushs = if plusieurs_rushs {
        "\nLes phrases viennent de plusieurs rushs (numéro indiqué). Une même idée dite dans plusieurs prises : garde la meilleure, coupe les autres."
    } else {
        ""
    };
    format!(
        "Tu es monteur vidéo pour YouTube. Tu montes une vidéo face caméra (une personne qui parle) pour la rendre rythmée et \
captivante, comme les meilleurs créateurs : coupes sèches, zooms, variations de rythme.

Tu reçois la transcription en phrases numérotées : début et fin en secondes, silence qui précède, énergie de la voix (dB par \
rapport à la moyenne ; positif = voix plus engagée).{rushs}

Phrase par phrase, tu décides :
- garder : true ou false. Coupe les répétitions, faux départs, phrases ratées ou reprises, digressions sans valeur, formules \
creuses. Si la même idée est dite deux fois de suite, garde la meilleure prise (souvent la seconde). Garde ce qui porte le sens, \
l'émotion, la personnalité. {redites}
- regime : \"respiration\" (récit, émotion, confidence, idée centrale : plan plus large, on laisse vivre les silences), \
\"rafale\" (informatif, énumérations, listes : coupes serrées, cadres qui alternent), \"impact\" (phrase choc, formule qui \
résume, chute, chiffre marquant : zoom serré, coupe sèche). La majorité en \"rafale\" ; \"respiration\" et \"impact\" sont des \
événements (chacun environ 10 à 20 % des phrases).
- {cadres}
- {transitions}
- mots_forts : un à trois mots de la phrase à mettre en couleur dans les sous-titres (mots porteurs de sens), ou liste vide.

Tu choisis aussi :
- {accroche}
- chapitres : les grandes parties, avec le numéro de la phrase où chacune commence et un titre court (2 à 5 mots).
- notes : deux à quatre phrases courtes qui expliquent tes choix principaux, pour la personne qui monte.

Réponds uniquement en JSON."
    )
}

const CONSIGNE_CRITIQUE: &str = "Tu es chef monteur. Un monteur a proposé le montage ci-dessous pour une vidéo YouTube face \
caméra. Relis-le comme un spectateur exigeant qui décroche à la moindre monotonie, puis rends la version corrigée complète.

Vérifie :
- l'accroche est-elle la plus forte possible, compréhensible sans contexte, et ne redit-elle pas l'introduction ?
- y a-t-il des tunnels : plus de 4 phrases de suite au même régime, ou plus de 25 secondes sans changement de cadre ?
- en \"rafale\", deux phrases consécutives ont-elles le même cadre ?
- les \"impact\" sont-ils de vraies formules choc ? Les \"yeux\" sont-ils rares (trois au plus) et mérités ?
- les \"respiration\" tombent-elles sur les vrais moments d'émotion ou de récit ?
- a-t-on coupé une idée essentielle, un souvenir personnel, une phrase forte ? A-t-on gardé des répétitions ou faux départs ?
- les chapitres découpent-ils bien la vidéo ?
Respecte les mêmes règles que le monteur (valeurs permises, réglages).

Rends le montage corrigé complet, au même format JSON. Dans \"notes\", trois à cinq phrases courtes : ce que tu as changé et pourquoi.";

fn transcription(ps: &[Phrase], rangs: &HashMap<String, usize>) -> String {
    ps.iter()
        .enumerate()
        .map(|(i, p)| {
            let rush = if rangs.len() > 1 { format!(" | rush {}", rangs[&p.rush] + 1) } else { String::new() };
            format!("[{i}] {:.1}–{:.1}s | silence {:.1}s | {:+.0} dB{rush} | {}", p.t0, p.t1, p.silence_avant, p.db, p.texte)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn vers_montage(v: &Value, ps: &[Phrase]) -> Montage {
    let cle = |i: &Value| i.as_u64().and_then(|i| ps.get(i as usize)).map(|p| p.cle.clone());
    let mut decisions: HashMap<String, Decision> = HashMap::new();
    for d in v["phrases"].as_array().into_iter().flatten() {
        let Some(c) = cle(&d["id"]) else { continue };
        let choisir = |champ: &str, permis: &[&str], defaut: &str| {
            d[champ].as_str().filter(|s| permis.contains(s)).unwrap_or(defaut).to_string()
        };
        decisions.insert(c.clone(), Decision {
            cle: c,
            garder: d["garder"].as_bool().unwrap_or(true),
            regime: choisir("regime", REGIMES, "rafale"),
            cadre: choisir("cadre", CADRES, "moyen"),
            transition: d["transition"].as_str().filter(|s| TRANSITIONS.contains(s)).map(String::from),
            mots_forts: d["mots_forts"].as_array().into_iter().flatten().filter_map(|m| m.as_str().map(String::from)).collect(),
        });
    }
    // Une phrase oubliée par le LLM est gardée plutôt que perdue.
    let phrases = ps
        .iter()
        .map(|p| decisions.remove(&p.cle).unwrap_or(Decision {
            cle: p.cle.clone(),
            garder: true,
            regime: "rafale".into(),
            cadre: "moyen".into(),
            transition: None,
            mots_forts: vec![],
        }))
        .collect();
    Montage {
        cree: maintenant(),
        phrases,
        accroche: v["accroche"].as_array().into_iter().flatten().filter_map(cle).collect(),
        chapitres: v["chapitres"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| Some(Chapitre { cle: cle(&c["phrase"])?, titre: c["titre"].as_str()?.to_string() }))
            .collect(),
        notes: v["notes"].as_array().into_iter().flatten().filter_map(|n| n.as_str().map(String::from)).collect(),
    }
}

fn vers_json(m: &Montage, ps: &[Phrase]) -> Value {
    let rang: HashMap<&str, usize> = ps.iter().enumerate().map(|(i, p)| (p.cle.as_str(), i)).collect();
    json!({
        "accroche": m.accroche.iter().filter_map(|c| rang.get(c.as_str())).collect::<Vec<_>>(),
        "chapitres": m.chapitres.iter().filter_map(|c| Some(json!({"phrase": rang.get(c.cle.as_str())?, "titre": c.titre}))).collect::<Vec<_>>(),
        "phrases": m.phrases.iter().filter_map(|d| Some(json!({"id": rang.get(d.cle.as_str())?, "garder": d.garder,
            "regime": d.regime, "cadre": d.cadre, "transition": d.transition, "mots_forts": d.mots_forts}))).collect::<Vec<_>>(),
        "notes": m.notes,
    })
}

/// Montage complet : proposition puis relecture critique.
pub async fn monter(etat: &EtatPartage, p: &Projet, ps: &[Phrase]) -> Result<Montage> {
    let rangs: HashMap<String, usize> = p.rushs.iter().enumerate().map(|(i, r)| (r.id.clone(), i)).collect();
    let texte_ps = transcription(ps, &rangs);
    let consigne = consigne_monteur(p, rangs.len() > 1);
    let schema = schema_montage();
    etat.signaler(&p.id, "Le LLM monte la vidéo", 10.0);
    let premier = vers_montage(&appeler(etat, &p.id, &consigne, &texte_ps, &schema, true).await?, ps);
    etat.signaler(&p.id, "Le LLM relit son montage", 55.0);
    let contenu = format!("RÉGLAGES ET RÈGLES DU MONTEUR :\n{consigne}\n\nTRANSCRIPTION :\n{texte_ps}\n\nMONTAGE PROPOSÉ :\n{}",
                          vers_json(&premier, ps));
    match appeler(etat, &p.id, CONSIGNE_CRITIQUE, &contenu, &schema, true).await {
        Ok(v) => Ok(vers_montage(&v, ps)),
        Err(e) => {
            tracing::warn!("critique en échec, montage d'origine gardé : {e}");
            Ok(premier)
        }
    }
}

// ---------------------------------------------------------------- conversation

fn schema_discussion() -> Value {
    json!({
        "type": "object",
        "properties": {
            "reponse": {"type": "string"},
            "reglages": reglages::schema_llm(),
            "phrases": {"type": "array", "items": {"type": "object", "properties": {
                "id": {"type": "integer"},
                "garder": {"type": "boolean"},
                "regime": {"type": "string", "enum": REGIMES},
                "cadre": {"type": "string", "enum": CADRES}},
                "required": ["id"]}},
            "ajouter_incrustations": {"type": "array", "items": {"type": "object", "properties": {
                "media": {"type": "string"},
                "effet": {"type": "string", "enum": ["", "secousse", "noir_blanc"]},
                "phrase": {"type": "integer"},
                "duree": {"type": "number"},
                "position": {"type": "string", "enum": POSITIONS},
                "taille": {"type": "number"},
                "fond_vert": {"type": "boolean"},
                "son": {"type": "boolean"}},
                "required": ["media", "effet", "phrase", "duree", "position", "taille", "fond_vert", "son"]}},
            "retirer_incrustations": {"type": "array", "items": {"type": "string"}},
            "remonter": {"type": "boolean"}
        },
        "required": ["reponse", "reglages", "phrases", "ajouter_incrustations", "retirer_incrustations", "remonter"]
    })
}

const CONSIGNE_DISCUSSION: &str = "Tu es EDIVID, l'assistant de montage vidéo. Tu parles avec la personne qui monte sa vidéo \
YouTube. Tu réponds en français, en deux à quatre phrases simples, sans jargon technique : tu dis ce que tu as fait et, en une \
phrase, pourquoi c'est un bon choix de montage. Si la demande est floue, tu proposes une option concrète.

Tu peux agir sur le montage en remplissant les champs de ta réponse :
- reglages : seulement les réglages à changer (valeurs permises indiquées), {} sinon ;
- phrases : changer une phrase (garder/couper, régime, cadre) par son numéro ;
- ajouter_incrustations : poser un média de la médiathèque (champ media = son nom exact) ou un effet intégré (\"secousse\", \
\"noir_blanc\" ; media vide) sur une phrase, avec une durée en secondes, une position et une taille (0,1 à 1 de la largeur) ; \
fond_vert à true si le média a un fond vert à retirer ; son à true pour garder le son d'une vidéo ;
- retirer_incrustations : identifiants des incrustations à enlever ;
- la musique de fond se choisit par le réglage musique (nom exact d'un son de la médiathèque) et volume_musique \
(« discrète » : 10 à 15 ; « présente » : 30 à 40) ; jamais par une incrustation ;
- remonter : true seulement si la demande touche la structure entière (refaire le montage, autre durée, autre ton) : le LLM \
remontera toute la vidéo avec les nouveaux réglages. Un simple changement de réglage ou de phrase ne demande pas de remonter.
Tu ne dis jamais que tu as fait une chose que tu n'as pas mise dans les champs. Après tes changements, un nouvel aperçu se \
prépare tout seul : dis-le si tu as changé quelque chose.";

fn contexte(p: &Projet, ps: &[Phrase]) -> String {
    let decisions: HashMap<&str, &Decision> =
        p.montage.as_ref().map(|m| m.phrases.iter().map(|d| (d.cle.as_str(), d)).collect()).unwrap_or_default();
    let phrases = ps
        .iter()
        .enumerate()
        .map(|(i, ph)| match decisions.get(ph.cle.as_str()) {
            Some(d) => format!("[{i}] {} {} {} | {}", if d.garder { "gardée" } else { "COUPÉE" }, d.regime, d.cadre, ph.texte),
            None => format!("[{i}] (pas encore montée) | {}", ph.texte),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let medias = if p.medias.is_empty() {
        "(vide)".to_string()
    } else {
        p.medias.iter().map(|m| format!("- {} ({})", m.nom, m.genre)).collect::<Vec<_>>().join("\n")
    };
    let rang: HashMap<&str, usize> = ps.iter().enumerate().map(|(i, ph)| (ph.cle.as_str(), i)).collect();
    let incrustations = if p.incrustations.is_empty() {
        "(aucune)".to_string()
    } else {
        p.incrustations
            .iter()
            .map(|i| {
                let quoi = if i.effet.is_empty() {
                    p.medias.iter().find(|m| m.id == i.media).map(|m| m.nom.clone()).unwrap_or_default()
                } else {
                    i.effet.clone()
                };
                format!("- id {} : {quoi} sur la phrase {} ({} s, {})", i.id,
                        rang.get(i.phrase.as_str()).map(|r| r.to_string()).unwrap_or("?".into()), i.duree, i.position)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let notes = p.montage.as_ref().map(|m| m.notes.join(" ")).unwrap_or_default();
    format!("RÉGLAGES ACTUELS :\n{}\n\nMÉDIATHÈQUE :\n{medias}\n\nINCRUSTATIONS :\n{incrustations}\n\n\
             NOTES DU DERNIER MONTAGE : {notes}\n\nPHRASES :\n{phrases}",
            reglages::pour_llm(&p.reglages))
}

pub async fn discuter(etat: &EtatPartage, p: &Projet, ps: &[Phrase], message: &str) -> Result<Value> {
    let historique = p
        .discussion
        .iter()
        .rev()
        .take(8)
        .rev()
        .map(|e| format!("{} : {}", if e.role == "moi" { "PERSONNE" } else { "EDIVID" }, e.texte))
        .collect::<Vec<_>>()
        .join("\n");
    let contenu = format!("{}\n\nCONVERSATION RÉCENTE :\n{historique}\n\nNOUVEAU MESSAGE DE LA PERSONNE :\n{message}",
                          contexte(p, ps));
    appeler(etat, &p.id, CONSIGNE_DISCUSSION, &contenu, &schema_discussion(), false).await
}
