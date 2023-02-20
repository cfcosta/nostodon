use ::metrics::increment_counter;
use clap::Parser;
use eyre::{eyre, ErrReport, Result};
use futures_util::future::try_join_all;
use health::POSTS_DELETED;
use mastodon_async::{
    prelude::{Event, Status},
    Visibility,
};
use nostr_sdk::prelude::{EventId, FromBech32, ToBech32};
use postgres::job_queue::ScheduledPost;
use tokio::task;

mod health;
mod mastodon;
mod nostr;
mod postgres;
mod util;

use crate::{
    health::{EVENTS_SKIPPED, POSTS_CREATED, PROFILES_UPDATED},
    mastodon::MastodonClient,
    postgres::*,
    util::extract_instance_url,
};

#[derive(Debug, Clone, Parser)]
pub struct Config {
    #[clap(flatten)]
    pub nostr: nostr::NostrConfig,

    #[clap(flatten)]
    pub postgres: postgres::PostgresConfig,
}

async fn spawn_poster(postgres: Postgres, config: Config) -> Result<()> {
    let task = |postgres: Postgres, config: Config| async move {
        let mut stream = postgres.listener().update_stream().await?;

        let item = stream.recv().await?;
        let creds = postgres.fetch_credentials(item.user_id).await?;
        let nostr = nostr::Nostr::connect(creds, config.nostr.clone().relays).await?;

        let event_id = nostr
            .publish(nostr::Note::new_text(html2md::parse_html(&item.content)))
            .await?;

        let post = MastodonPost {
            instance_id: item.instance_id,
            user_id: item.user_id,
            mastodon_id: item.mastodon_id.clone(),
            nostr_id: event_id.to_string(),
            status: MastodonPostStatus::Posted,
        };

        postgres.add_post(post).await?;
        postgres.listener().finish(item.mastodon_id).await?;

        increment_counter!(POSTS_CREATED);
        dbg!(event_id.to_bech32()?);

        #[allow(unreachable_code)]
        Ok::<_, ErrReport>(())
    };

    loop {
        match task(postgres.clone(), config.clone()).await {
            Ok(_) => continue,
            Err(e) => {
                println!("Got an error: {e}");
                println!("Stream died, restarting...");
                continue;
            }
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}

async fn process_status(postgres: Postgres, status: Status, relays: Vec<String>) -> Result<()> {
    if status.visibility != Visibility::Public {
        let visibility_text = match status.visibility {
            Visibility::Direct => "direct",
            Visibility::Private => "private",
            Visibility::Unlisted => "unlisted",
            Visibility::Public => "public",
        };

        println!("Skipping update {:?} because it is not public", &status);
        increment_counter!(EVENTS_SKIPPED, "visibility" => visibility_text);

        return Ok(());
    }

    if status.url.is_none() {
        panic!("No Url")
    }

    let instance_url = extract_instance_url(status.url.as_ref().unwrap())?;

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

    let creds = postgres.fetch_credentials(user_id).await?;

    let profile = Profile::build(instance_id, user_id, &status)?;

    if postgres.update_profile(profile.clone()).await?.changed() {
        let nostr = nostr::Nostr::connect(creds, relays).await?;
        nostr.update_user_profile(profile).await?;
        increment_counter!(PROFILES_UPDATED);
    }

    postgres
        .listener()
        .push(ScheduledPost {
            content: status.content,
            instance_id,
            user_id,
            mastodon_id: status.id.to_string(),
        })
        .await?;

    Ok(())
}

async fn spawn(server: MastodonServer, config: Config, postgres: Postgres) -> Result<()> {
    let mastodon = mastodon::Mastodon::connect(server)?;

    let mut rx = mastodon.update_stream().await?;

    loop {
        match rx.try_recv() {
            Ok(ev) => match ev {
                Event::Delete(id) => {
                    let result = postgres.delete_post(id.clone()).await;

                    match result {
                        Ok(Some((user_id, event_id))) => {
                            let event_id = EventId::from_bech32(event_id)?;
                            let creds = postgres.fetch_credentials(user_id).await?;

                            let nostr =
                                nostr::Nostr::connect(creds, config.nostr.clone().relays).await?;

                            nostr.delete_event(event_id).await?;

                            increment_counter!(POSTS_DELETED);
                        }
                        _ => continue,
                    }
                }
                Event::Update(status) => match process_status(postgres.clone(), status, config.nostr.relays.clone()).await {
                    Ok(_) => continue,
                    Err(e) => {
                        println!("Error while processing update: {e}");
                        continue
                    },
                },
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

    let postgres = Postgres::init(config.clone().postgres).await?;
    postgres.health_check().await?;

    let mut tasks = vec![];

    for server in postgres.fetch_servers().await?.into_iter() {
        tasks.push(spawn(server, config.clone(), postgres.clone()));
    }

    task::spawn(spawn_poster(postgres.clone(), config.clone()));

    if tasks.is_empty() {
        return Err(eyre!(
            "There are no configured servers. Please add some on mastodon_servers."
        ));
    }

    try_join_all(tasks).await?;

    Ok(())
}
