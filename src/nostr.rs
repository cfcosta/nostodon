use clap::Parser;
use eyre::Result;
use nostr_sdk::prelude::*;
use tokio::sync::broadcast::Receiver;

use crate::metrics::Timeable;

#[derive(Debug, Clone, Parser)]
pub struct NostrConfig {
    #[clap(short = 'k', long = "private-key", env = "NOSTODON_NOSTR_PRIVATE_KEY")]
    pub private_key: String,

    #[clap(short = 'r', long = "relays", env = "NOSTODON_NOSTR_RELAYS")]
    pub relays: Vec<String>,
}

pub struct UserProfile {
    pub name: String,
    pub display_name: String,
    pub about: String,
    pub picture: String,
    pub banner: String,
    pub nip05: String,
}

#[async_trait::async_trait]
pub trait NostrClient
where
    Self: Sized,
{
    type EventId;

    async fn publish(&self, note: Note) -> Result<Self::EventId>;
    async fn update_user_profile(&self, profile: UserProfile) -> Result<Self::EventId>;
}

#[derive(Debug, Clone, Default)]
pub struct Note {
    pub text: String,
    pub tags: Vec<nostr_sdk::prelude::Tag>,
}

impl Note {
    pub fn new_text<T: Into<String>>(text: T) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct Nostr {
    client: Client,
}

impl Nostr {
    pub async fn connect(config: NostrConfig) -> Result<Self> {
        let opts = Options::new().wait_for_connection(true).wait_for_send(true);
        let keypair = Keys::from_sk_str(&config.private_key)?;

        let this = Self {
            client: Client::new_with_opts(&keypair, opts),
        };

        for relay in config.relays {
            this.client
                .add_relay(&relay, None)
                .time_as("nostr.connect.client_add_relay")
                .await?;
        }

        this.client
            .connect()
            .time_as("nostr.connect.client_connect")
            .await;

        Ok(this)
    }
}

#[async_trait::async_trait]
impl NostrClient for Nostr {
    type EventId = EventId;

    async fn publish(&self, note: Note) -> Result<Self::EventId> {
        Ok(self
            .client
            .publish_text_note(&note.text, &note.tags)
            .time_as("nostr.publish.client_publish")
            .await?)
    }

    async fn update_user_profile(&self, profile: UserProfile) -> Result<Self::EventId> {
        todo!()
    }
}
