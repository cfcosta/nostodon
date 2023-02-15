use clap::Parser;
use eyre::Result;
use mastodon_async::prelude::Event;

mod mastodon;
mod metrics;
mod nostr;
mod storage;

use crate::{mastodon::MastodonClient, metrics::Timeable, nostr::NostrClient, storage::*};

#[derive(Debug, Clone, Parser)]
pub struct Config {
    #[clap(flatten)]
    pub nostr: nostr::NostrConfig,

    #[clap(flatten)]
    pub mastodon: mastodon::MastodonConfig,

    #[clap(flatten)]
    pub postgres: postgres::PostgresConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    metrics::Provider::setup();
    // TODO: init migrations

    let config = Config::parse();

    let postgres = postgres::Postgres::init(config.postgres).await?;
    postgres.health_check().await?;

    let nostr = nostr::Nostr::connect(config.nostr)
        .time_as("nostr.connect")
        .await?;

    let event_id = nostr
        .publish(nostr::Note::new_text("Hello World"))
        .time_as("nostr.publish")
        .await?;
    let mastodon = mastodon::Mastodon::connect(config.mastodon)?;

    let mut rx = mastodon.update_stream().await?;

    loop {
        match rx.try_recv() {
            Ok(ev) => match ev {
                Event::Delete(id) => {
                    todo!();
                }
                Event::Update(status) => {
                    dbg!(status);
                    todo!();
                }
                _ => continue,
            },
            Err(_) => continue,
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}
