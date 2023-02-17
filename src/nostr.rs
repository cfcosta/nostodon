use clap::Parser;
use eyre::Result;
use nostr_sdk::prelude::*;

use crate::{health::Timeable, storage::Profile};

#[derive(Debug, Clone, Parser)]
pub struct NostrConfig {
    #[clap(short = 'r', long = "relays", env = "NOSTODON_NOSTR_RELAYS")]
    pub relays: Vec<String>,
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
    pub async fn connect(keypair: Keys, relays: Vec<String>) -> Result<Self> {
        let opts = Options::new().wait_for_connection(true).wait_for_send(true);

        let this = Self {
            client: Client::new_with_opts(&keypair, opts),
        };

        for relay in relays {
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

    pub async fn publish(&self, note: Note) -> Result<EventId> {
        Ok(self
            .client
            .publish_text_note(&note.text, &note.tags)
            .time_as("nostr.publish.client_publish")
            .await?)
    }

    pub async fn update_user_profile(&self, profile: Profile) -> Result<EventId> {
        let metadata = Metadata::new()
            .name(&profile.name)
            .display_name(format!("[Unofficial Mirror] {}", profile.display_name))
            .banner(Url::parse(&profile.banner)?)
            .picture(Url::parse(&profile.picture)?)
            .nip05(format!("{}@nostodon.org", &profile.nip05))
            .about(format!(
                "THIS IS AN UNNOFICIAL MIRROR. CHECK THE PROFILE FOR CORRECT INFO.\n\n{}",
                profile.about
            ));

        Ok(self.client.update_profile(metadata).await?)
    }

    pub async fn delete_event(&self, event: EventId) -> Result<EventId> {
        Ok(self
            .client
            .delete_event(event, Some("deleted from remote source"))
            .await?)
    }
}
