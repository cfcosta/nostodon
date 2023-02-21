use eyre::Result;
use nostr_sdk::prelude::*;

use crate::{
    health::Timeable,
    postgres::{job_queue::ScheduledPost, Postgres, Profile},
};

#[derive(Debug, Clone, Default)]
pub struct Note {
    pub text: String,
    pub tags: Vec<Tag>,
}

impl Note {
    pub async fn build(postgres: &Postgres, post: &ScheduledPost) -> Result<Self> {
        let mut tags = vec![];

        if let Some(root) = &post.in_reply_to {
            if let Some(event_id) = postgres.fetch_nostr_id(root.to_string()).await? {
                // Tracks the relationship between replies
                tags.push(Tag::Event(
                    EventId::from_bech32(event_id)?,
                    // TODO: Add one of the configured relays here
                    None,
                    Some("root".into()),
                ));
            }
        }

        Ok(Self {
            text: html2md::parse_html(&post.content),
            tags,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Nostr {
    client: Client,
}

impl Nostr {
    pub async fn connect(postgres: &Postgres, keypair: Keys) -> Result<Self> {
        let opts = Options::new().wait_for_connection(true).wait_for_send(true);
        let relays = postgres.fetch_nostr_relays().await?;

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

        Ok(self
            .client
            .update_profile(metadata)
            .time_as("nostr.update_profile")
            .await?)
    }
}
