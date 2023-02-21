use clap::Parser;
use eyre::Result;
use tokio::task;

mod health;
mod listener;
mod mastodon;
mod nostr;
mod poster;
mod postgres;
mod util;

use crate::postgres::*;

#[derive(Debug, Clone, Parser)]
pub struct Config {
    #[clap(flatten)]
    pub postgres: postgres::PostgresConfig,

    #[clap(long = "skip-posting", short = 'p', env = "NOSTODON_SKIP_POSTING")]
    /// Only schedule posting on the database, do not actually post them
    pub skip_posting: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    health::Provider::setup();

    let config = Config::parse();
    let postgres = Postgres::init(config.clone().postgres).await?;

    postgres.health_check().await?;
    postgres.migrate().await?;

    if !config.skip_posting {
        task::spawn(poster::spawn(postgres.clone()));
    }

    listener::spawn(postgres).await?;

    Ok(())
}
