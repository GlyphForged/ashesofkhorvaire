use std::{env, net::SocketAddr};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_address: SocketAddr,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://ashes.db".into());
        let bind_address = env::var("BIND_ADDRESS")
            .unwrap_or_else(|_| "127.0.0.1:3000".into())
            .parse()?;
        Ok(Self {
            database_url,
            bind_address,
        })
    }
}
