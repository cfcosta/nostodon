use clap::Parser;
use eyre::Result;
use mastodon_async::prelude::Event;

mod mastodon;
mod metrics;
mod nostr;

use crate::{mastodon::MastodonClient, metrics::Timeable, nostr::NostrClient};

#[derive(Debug, Clone, Parser)]
pub struct Config {
    #[clap(flatten)]
    pub nostr: nostr::NostrConfig,

    #[clap(flatten)]
    pub mastodon: mastodon::MastodonConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    metrics::Provider::setup();

    let config = Config::parse();

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
