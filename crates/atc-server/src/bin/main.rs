use anyhow::Context;
use atc_server::{build_router, config::ServerConfig};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = ServerConfig::from_file("server.toml")
        .context("load server config from server.toml")?;
    let bind_address = config.bind_address.clone();
    let (router, _state) = build_router(config);

    let listener = TcpListener::bind(&bind_address)
        .await
        .with_context(|| format!("bind server listener on {bind_address}"))?;
    tracing::info!("atc-server listening on {}", bind_address);

    axum::serve(listener, router)
        .await
        .context("run axum server")?;
    Ok(())
}
