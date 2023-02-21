use ::metrics::increment_counter;
use clap::Parser;
use eyre::{eyre, ErrReport, Result};
use futures_util::future::try_join_all;
use health::{Timeable, POSTS_DELETED};
use mastodon_async::{
    prelude::{Event, Status},
    Visibility,
};
use nostr_sdk::prelude::{EventId, FromBech32};
use postgres::job_queue::ScheduledPost;
use tokio::task;
use tracing::{debug, error};

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
    pub postgres: postgres::PostgresConfig,

    #[clap(long = "skip-posting", short = 'p', env = "NOSTODON_SKIP_POSTING")]
    pub skip_posting: bool,
}

async fn spawn_poster(postgres: Postgres) -> Result<()> {
    let task = |postgres: Postgres| async move {
        let mut stream = postgres.listener().update_stream().await?;

        let item = stream.recv().await?;
        task::spawn(async move {
            let creds = postgres.fetch_credentials(item.user_id).await?;
            let nostr = nostr::Nostr::connect(&postgres, creds).await?;

            let event_id = match nostr
                .publish(nostr::Note::new_text(html2md::parse_html(&item.content)))
                .await
            {
                Ok(id) => id,
                e => {
                    postgres.listener().error(item.mastodon_id.clone()).await?;
                    e?
                }
            };

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

            Ok::<_, ErrReport>(())
        });

        Ok::<_, ErrReport>(())
    };

    loop {
        match task(postgres.clone()).await {
            Ok(_) => continue,
            Err(e) => {
                error!("Got an error when fetching mastodon updates: {:?}", e);
                continue;
            }
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}

async fn process_status(postgres: Postgres, status: Status) -> Result<()> {
    let visibility_text = match status.visibility {
        Visibility::Direct => "direct",
        Visibility::Private => "private",
        Visibility::Unlisted => "unlisted",
        Visibility::Public => "public",
    };

    if status.visibility != Visibility::Public {
        debug!("Skipping update {:?} because it is not public", &status);
        increment_counter!(EVENTS_SKIPPED, "visibility" => visibility_text, "reason" => "visibility");

        return Ok(());
    }

    if status.url.is_none() {
        todo!("No Url");
    }

    let instance_url = extract_instance_url(status.url.as_ref().unwrap())?;

    let instance = postgres
        .fetch_or_create_instance(instance_url.as_str())
        .await?;

    if instance.blacklisted {
        debug!(
            "Skipping update {:?} because instance is blacklisted",
            &status
        );
        increment_counter!(EVENTS_SKIPPED, "visibility" => visibility_text, "reason" => "instance_blacklist");
    }

    let nip05 = format!(
        "{}.{}",
        status.account.username.clone(),
        instance_url.host().unwrap()
    );

    let user_id = postgres
        .fetch_or_create_user(instance.id, nip05.clone())
        .await?;

    if postgres.is_user_blacklisted(user_id).await? {
        debug!("Skipping update {:?} because user is blacklisted", &status);
        increment_counter!(EVENTS_SKIPPED, "visibility" => visibility_text, "reason" => "user_blacklist");

        return Ok(());
    }

    let creds = postgres.fetch_credentials(user_id).await?;

    let profile = Profile::build(instance.id, user_id, &status)?;

    if postgres.update_profile(profile.clone()).await?.changed() {
        let nostr = nostr::Nostr::connect(&postgres, creds).await?;
        nostr.update_user_profile(profile).await?;
        increment_counter!(PROFILES_UPDATED);
    }

    postgres
        .listener()
        .push(ScheduledPost {
            content: status.content,
            instance_id: instance.id,
            user_id,
            mastodon_id: status.id.to_string(),
        })
        .await?;

    Ok(())
}

async fn spawn(server: MastodonServer, postgres: Postgres) -> Result<()> {
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

                            let nostr = nostr::Nostr::connect(&postgres, creds).await?;

                            nostr.delete_event(event_id).await?;

                            increment_counter!(POSTS_DELETED);
                        }
                        _ => continue,
                    }
                }
                Event::Update(status) => {
                    match process_status(postgres.clone(), status)
                        .time_as("mastodon.process_status")
                        .await
                    {
                        Ok(_) => continue,
                        Err(e) => {
                            error!("Error while processing update: {e}");
                            continue;
                        }
                    }
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
    tracing_subscriber::fmt::init();
    health::Provider::setup();
    // TODO: init migrations

    let config = Config::parse();

    let postgres = Postgres::init(config.clone().postgres).await?;
    postgres.health_check().await?;

    let mut tasks = vec![];

    for server in postgres.fetch_servers().await?.into_iter() {
        tasks.push(spawn(server, postgres.clone()));
    }

    if !config.skip_posting {
        task::spawn(spawn_poster(postgres.clone()));
    }

    if tasks.is_empty() {
        return Err(eyre!(
            "There are no configured servers. Please add some on mastodon_servers."
        ));
    }

    try_join_all(tasks).await?;

    Ok(())
}
