//! Courriels : montage prêt, rendu final prêt, erreur.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use lettre::message::{header::ContentType, Mailbox};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::etat::EtatPartage;
use crate::modele::Projet;
use crate::parametres::Courriel;

pub async fn envoyer(c: &Courriel, sujet: &str, texte: &str) -> Result<()> {
    if c.serveur.trim().is_empty() {
        return Err(anyhow!("aucun serveur d'envoi n'est configuré"));
    }
    if c.destinataires.is_empty() {
        return Err(anyhow!("aucun destinataire"));
    }
    let expediteur: Mailbox = c.expediteur.parse().context("adresse d'expéditeur invalide")?;
    let mut message = Message::builder().from(expediteur).subject(sujet);
    for d in &c.destinataires {
        message = message.to(d.parse().with_context(|| format!("adresse invalide : {d}"))?);
    }
    let message = message.header(ContentType::TEXT_PLAIN).body(texte.to_string())?;
    let serveur = c.serveur.trim();
    let mut transport = match c.securite.as_str() {
        "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(serveur)?,
        "aucune" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(serveur),
        _ => AsyncSmtpTransport::<Tokio1Executor>::relay(serveur)?,
    }
    .port(c.port)
    .timeout(Some(Duration::from_secs(20)));
    if !c.identifiant.is_empty() {
        transport = transport.credentials(Credentials::new(c.identifiant.clone(), c.mot_de_passe.clone()));
    }
    transport.build().send(message).await.map_err(|e| anyhow!("envoi refusé : {e}"))?;
    Ok(())
}

fn lien(url: &str, p: &Projet) -> String {
    format!("{}/#/projet/{}", url.trim_end_matches('/'), p.id)
}

fn minutes(s: f64) -> String {
    let s = s.round().max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Prévient par courriel, sans jamais bloquer ni faire échouer le travail qui appelle.
pub async fn prevenir(etat: &EtatPartage, genre: &str, p: &Projet, detail: &str) {
    let parametres = etat.parametres.read().await.clone();
    let c = &parametres.courriel;
    let voulu = c.actif
        && match genre {
            "montage" => c.montage,
            "rendu" => c.rendu,
            _ => c.erreur,
        };
    if !voulu {
        return;
    }
    let url = lien(&parametres.url_publique, p);
    let (sujet, texte) = match genre {
        "montage" => {
            let r = p.apercu.as_ref().map(|s| &s.rapport);
            let chiffres = r
                .map(|r| format!(" ({}, {} plans)", minutes(r["duree_montage"].as_f64().unwrap_or(0.0)), r["plans"]))
                .unwrap_or_default();
            (format!("EDIVID — le montage de « {} » est prêt", p.nom),
             format!("Bonjour,\n\nLe LLM a monté « {} »{chiffres}. Un aperçu vous attend : regardez-le, puis ajustez en \
                      discutant avec l'assistant ou fabriquez la vidéo finale.\n\n{url}\n\n— EDIVID", p.nom))
        }
        "rendu" => {
            let r = p.rendu.as_ref().map(|s| &s.rapport);
            let chiffres = r
                .map(|r| format!(" ({}, {}×{})", minutes(r["duree_montage"].as_f64().unwrap_or(0.0)), r["largeur"], r["hauteur"]))
                .unwrap_or_default();
            let drive = p.depot_drive.as_ref().map(|d| format!("\n\nDéposée sur votre Google Drive : {d}")).unwrap_or_default();
            (format!("EDIVID — la vidéo « {} » est prête", p.nom),
             format!("Bonjour,\n\nLa vidéo finale de « {} » est prête{chiffres}.{drive}\n\nPour la regarder ou la \
                      télécharger :\n{url}\n\n— EDIVID", p.nom))
        }
        _ => (format!("EDIVID — problème sur « {} »", p.nom),
              format!("Bonjour,\n\nUn travail n'a pas abouti sur « {} » :\n{detail}\n\nLe projet :\n{url}\n\n— EDIVID", p.nom)),
    };
    let mut c = c.clone();
    // Le compte Google d'où vient le projet est prévenu aussi.
    if p.compte.contains('@') && !c.destinataires.iter().any(|d| d.eq_ignore_ascii_case(&p.compte)) {
        c.destinataires.push(p.compte.clone());
    }
    let nom = p.nom.clone();
    tokio::spawn(async move {
        match envoyer(&c, &sujet, &texte).await {
            Ok(()) => tracing::info!("courriel « {sujet} » envoyé"),
            Err(e) => tracing::warn!("courriel pour « {nom} » non envoyé : {e:#}"),
        }
    });
}
