use ::metrics::increment_counter;
use clap::Parser;
use eyre::Result;
use futures_util::future::try_join_all;
use health::POSTS_DELETED;
use mastodon_async::{prelude::Event, Visibility};
use nostr_sdk::prelude::{EventId, FromBech32, ToBech32};

mod health;
mod mastodon;
mod nostr;
mod storage;
mod util;

use crate::{
    health::{Timeable, EVENTS_SKIPPED, POSTS_CREATED, PROFILES_UPDATED},
    mastodon::MastodonClient,
    storage::*, util::extract_instance_url,
};

#[derive(Debug, Clone, Parser)]
pub struct Config {
    #[clap(flatten)]
    pub nostr: nostr::NostrConfig,

    #[clap(flatten)]
    pub postgres: postgres::PostgresConfig,
}

async fn spawn(server: MastodonServer, config: Config, postgres: postgres::Postgres) -> Result<()> {
    let mastodon = mastodon::Mastodon::connect(server)?;

    let mut rx = mastodon.update_stream().await?;

    loop {
        match rx.try_recv() {
            Ok(ev) => match ev {
                Event::Delete(id) => {
                    let result = postgres
                        .delete_post(id.clone())
                        .time_as("postgres.delete_post")
                        .await;

                    match result {
                        Ok(Some((user_id, event_id))) => {
                            let event_id = EventId::from_bech32(event_id)?;
                            let creds = postgres
                                .fetch_credentials(user_id)
                                .time_as("postgres.fetch_credentials")
                                .await?;

                            let nostr = nostr::Nostr::connect(creds, config.nostr.clone().relays)
                                .time_as("nostr.connect")
                                .await?;

                            nostr
                                .delete_event(event_id)
                                .time_as("nostr.delete_event")
                                .await?;

                            increment_counter!(POSTS_DELETED);
                        }
                        _ => continue,
                    }
                }
                Event::Update(status) => {
                    if status.visibility != Visibility::Public {
                        let visibility_text = match status.visibility {
                            Visibility::Direct => "direct",
                            Visibility::Private => "private",
                            Visibility::Unlisted => "unlisted",
                            Visibility::Public => "public",
                        };

                        println!("Skipping update {:?} because it is not public", &status);
                        increment_counter!(EVENTS_SKIPPED, "visibility" => visibility_text);

                        continue;
                    }

                    if status.url.is_none() {
                        continue;
                    }

                    let instance_url = extract_instance_url(status.url.as_ref().unwrap())?;

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

                    let creds = postgres
                        .fetch_credentials(user_id)
                        .time_as("postgres.fetch_credentials")
                        .await?;

                    let nostr = nostr::Nostr::connect(creds, config.nostr.clone().relays)
                        .time_as("nostr.connect")
                        .await?;

                    let event_id = nostr
                        .publish(nostr::Note::new_text(html2md::parse_html(&status.content)))
                        .time_as("nostr.publish")
                        .await?;

                    let profile = Profile::build(instance_id, user_id, &status)?;

                    if postgres.update_profile(profile.clone()).await?.changed() {
                        nostr
                            .update_user_profile(profile)
                            .time_as("nostr.update_user_proile")
                            .await?;
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

#[tokio::main]
async fn main() -> Result<()> {
    health::Provider::setup();
    // TODO: init migrations

    let config = Config::parse();

    let postgres = postgres::Postgres::init(config.clone().postgres)
        .time_as("postgres.init")
        .await?;
    postgres
        .health_check()
        .time_as("postgres.health_check")
        .await?;

    let mut tasks = vec![];

    for server in postgres.fetch_servers().await?.into_iter() {
        tasks.push(spawn(server, config.clone(), postgres.clone()));
    }

    try_join_all(tasks).await?;

    Ok(())
}
