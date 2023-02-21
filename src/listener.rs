use eyre::{eyre, Result};
use futures_util::future::try_join_all;
use mastodon_async::{
    prelude::{Event, Status},
    Visibility,
};
use metrics::increment_counter;
use nostr_sdk::prelude::{EventId, FromBech32};
use tracing::{debug, error};

use crate::{
    health::*,
    mastodon::*,
    nostr::Nostr,
    postgres::{job_queue::*, *},
    util::*,
};

pub async fn spawn(postgres: Postgres) -> Result<()> {
    let mut tasks = vec![];

    for server in postgres.fetch_servers().await?.into_iter() {
        tasks.push(spawn_listener(server, postgres.clone()));
    }

    if tasks.is_empty() {
        return Err(eyre!(
            "There are no configured servers. Please add some on mastodon_servers."
        ));
    }

    try_join_all(tasks).await?;

    Ok(())
}

async fn spawn_listener(server: MastodonServer, postgres: Postgres) -> Result<()> {
    let mastodon = Mastodon::connect(server)?;

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

                            let nostr = Nostr::connect(&postgres, creds).await?;

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

    let profile = Profile::build(instance.id, user_id, &status)?;

    postgres
        .listener()
        .push(ScheduledPost {
            content: status.content,
            instance_id: instance.id,
            user_id,
            mastodon_id: status.id.to_string(),
            profile_name: profile.name,
            profile_display_name: profile.display_name,
            profile_about: profile.about,
            profile_picture: profile.picture,
            profile_nip05: profile.nip05,
            profile_banner: profile.banner,
        })
        .await?;

    Ok(())
}
