//! EDIVID — monteur vidéo IA local, piloté par un seul LLM.

mod acces;
mod api;
mod bibliotheque;
mod carte;
mod config;
mod courriel;
mod decoupe;
mod drive;
mod etat;
mod google;
mod llm;
mod modele;
mod parametres;
mod postprod;
mod reglages;
mod rendu;
mod travaux;
mod youtube;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("edivid=info")))
        .init();
    let cfg = config::Config::charger()?;
    let adresses = cfg.ecoute.clone();
    let nvenc = rendu::nvenc_disponible().await;
    tracing::info!("encodeur matériel NVENC : {}", if nvenc { "disponible" } else { "absent, x264 utilisé" });
    let etat = etat::Etat::nouveau(cfg, nvenc).await?;
    travaux::demarrer(etat.clone());
    carte::demarrer_veille(etat.clone());
    drive::demarrer(etat.clone());
    travaux::rattraper_proxys(&etat).await;
    let app = api::routes(etat.clone());
    let mut serveurs = Vec::new();
    for adresse in adresses {
        match tokio::net::TcpListener::bind(adresse).await {
            Ok(ecoute) => {
                tracing::info!("EDIVID écoute sur http://{adresse}");
                let app = app.clone();
                // L'adresse du client sert au garde d'accès (identité Tailscale crue seulement en local).
                serveurs.push(tokio::spawn(async move {
                    axum::serve(ecoute, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await
                }));
            }
            Err(e) => tracing::warn!("impossible d'écouter sur {adresse} : {e}"),
        }
    }
    if serveurs.is_empty() {
        anyhow::bail!("aucune adresse d'écoute disponible");
    }
    // Arrêt propre (systemctl stop) : la carte est rendue avant de partir.
    let mut terme = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        _ = terme.recv() => {}
        _ = tokio::signal::ctrl_c() => {}
        r = futures_util::future::select_all(serveurs) => { r.0??; }
    }
    tracing::info!("arrêt : bail du LLM rendu");
    etat.carte.tout_rendre().await;
    Ok(())
}
