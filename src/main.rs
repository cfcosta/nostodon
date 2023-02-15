use ::metrics::increment_counter;
use clap::Parser;
use eyre::Result;
use mastodon_async::{prelude::Event, Visibility};
use nostr_sdk::prelude::Url;

mod health;
mod mastodon;
mod nostr;
mod storage;

use crate::{
    health::{Timeable, EVENTS_PROCESSED},
    mastodon::MastodonClient,
    nostr::NostrClient,
    storage::*,
};

#[derive(Debug, Clone, Parser)]
pub struct Config {
    #[clap(flatten)]
    pub nostr: nostr::NostrConfig,

    #[clap(flatten)]
    pub mastodon: mastodon::MastodonConfig,

    #[clap(flatten)]
    pub postgres: postgres::PostgresConfig,
}

fn base_url(mut url: Url) -> Result<Url> {
    {
        let mut path = url.path_segments_mut().unwrap();
        path.clear();
    }

    url.set_query(None);

    Ok(url)
}

#[tokio::main]
async fn main() -> Result<()> {
    health::Provider::setup();
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
                    println!("DELETE - {id}");
                }
                Event::Update(status) => {
                    dbg!(&status);
                    if status.visibility != Visibility::Public {
                        let visibility_text = match status.visibility {
                            Visibility::Direct => "direct",
                            Visibility::Private => "private",
                            Visibility::Unlisted => "unlisted",
                            Visibility::Public => "public",
                        };

                        println!("Skipping update {:?} because it is not public", &status);
                        increment_counter!(EVENTS_PROCESSED, "visibility" => visibility_text);

                        continue;
                    }

                    let instance_url = match status.url {
                        Some(v) => base_url(Url::parse(&v)?)?,
                        None => continue,
                    };

                    let instance_id = postgres
                        .fetch_or_create_instance(instance_url.as_str())
                        .await?;

                    let nip05 = format!(
                        "{}.{}",
                        status.account.username.clone(),
                        instance_url.host().unwrap()
                    );
                    let user_id = postgres
                        .fetch_or_create_user(instance_id, nip05.clone())
                        .await?;

                    let profile = Profile {
                        instance_id,
                        name: status.account.username.clone(),
                        display_name: status.account.display_name.clone(),
                        about: status.account.note.clone(),
                        user_id,
                        nip05,
                        picture: status.account.avatar.clone(),
                    };

                    postgres.update_profile(profile).await?;
                }
                _ => continue,
            },
            Err(_) => continue,
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}
