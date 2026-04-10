use std::net::SocketAddr;

use daedalus_api::router;
use daedalus_service::DaedalusService;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let service = DaedalusService::bootstrap_default("daemon")?;
    let config = service.current_config()?;
    let address: SocketAddr = format!("{}:{}", config.daemon.host, config.daemon.port).parse()?;
    let listener = tokio::net::TcpListener::bind(address).await?;

    info!("daedalus daemon listening on http://{}", address);
    axum::serve(listener, router(service)).await?;
    Ok(())
}
