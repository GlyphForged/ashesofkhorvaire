use ashes_wiki::{AppState, config::Config, create_pool, initialize, router};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ashes_wiki=info,tower_http=info".into()),
        )
        .init();
    let config = Config::from_env()?;
    let pool = create_pool(&config.database_url).await?;
    initialize(&pool).await?;
    let listener = TcpListener::bind(&config.bind_address).await?;
    tracing::info!(address = %config.bind_address, "Ashes of Khorvaire wiki ready");
    axum::serve(listener, router(AppState { pool })).await?;
    Ok(())
}
