use eyre::Result;
use clap::Parser;

mod nostr;

use crate::nostr::NostrClient;

#[derive(Debug, Clone, Parser)]
pub struct Config {
    #[clap(flatten)]
    pub nostr: nostr::NostrConfig
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::parse();
    let nostr = nostr::Nostr::connect(config.nostr).await?;

    let event_id = nostr.publish(nostr::Note::new_text("Hello World")).await?;
    dbg!(event_id);

    Ok(())
}
