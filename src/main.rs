use clap::Parser;
use eyre::Result;

mod nostr;
mod util;

use crate::{nostr::NostrClient, util::Timeable};

#[derive(Debug, Clone, Parser)]
pub struct Config {
    #[clap(flatten)]
    pub nostr: nostr::NostrConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::parse();
    let nostr = nostr::Nostr::connect(config.nostr)
        .time_as("nostr.connect")
        .await?;

    let event_id = nostr
        .publish(nostr::Note::new_text("Hello World"))
        .time_as("nostr.publish")
        .await?;
    dbg!(event_id);

    Ok(())
}
