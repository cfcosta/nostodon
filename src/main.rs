use ::metrics::increment_counter;
use clap::Parser;
use eyre::Result;
use mastodon_async::{prelude::Event, Visibility};
use nostr_sdk::prelude::{Url, ToBech32};

mod health;
mod mastodon;
mod nostr;
mod storage;

use crate::{
    health::{Timeable, EVENTS_PROCESSED, POSTS_CREATED, PROFILES_UPDATED},
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

    let postgres = postgres::Postgres::init(config.clone().postgres).time_as("postgres.init").await?;
    postgres.health_check().time_as("postgres.health_check").await?;

    let mastodon = mastodon::Mastodon::connect(config.clone().mastodon)?;

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
                        .time_as("postgres.fetch_or_create_instance")
                        .await?;

                    let nip05 = format!(
                        "{}.{}",
                        status.account.username.clone(),
                        instance_url.host().unwrap()
                    );

                    let user_id = postgres
                        .fetch_or_create_user(instance_id, nip05.clone())
                        .time_as("postgres.fetch_or_create_user")
                        .await?;

                    let creds = postgres.fetch_credentials(user_id).time_as("postgres.fetch_credentials").await?;

                    let nostr = nostr::Nostr::connect(creds, config.nostr.clone().relays)
                        .time_as("nostr.connect")
                        .await?;

                    let event_id = nostr
                        .publish(nostr::Note::new_text(html2md::parse_html(&status.content)))
                        .time_as("nostr.publish")
                        .await?;

                    let profile = Profile {
                        instance_id,
                        name: status.account.username.clone(),
                        display_name: status.account.display_name.clone(),
                        about: status.account.note.clone(),
                        user_id,
                        nip05,
                        picture: status.account.avatar.clone(),
                        banner: status.account.header.clone(),
                    };

                    if postgres.update_profile(profile.clone()).await?.changed() {
                        nostr.update_user_profile(profile).time_as("nostr.update_user_proile").await?;
                        increment_counter!(PROFILES_UPDATED);
                    }

                    let post = MastodonPost {
                        instance_id,
                        user_id,
                        mastodon_id: status.id.to_string(),
                        nostr_id: event_id.to_string(),
                        status: MastodonPostStatus::Posted,
                    };

                    postgres.add_post(post).time_as("postgres.add_post").await?;

                    increment_counter!(POSTS_CREATED);
                    dbg!(event_id.to_bech32()?);

                    break;
                }
                _ => continue,
            },
            Err(_) => continue,
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}
