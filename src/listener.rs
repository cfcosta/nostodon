use eyre::{eyre, Result};
use futures_util::future::try_join_all;
use mastodon_async::{prelude::Status, Visibility};
use metrics::increment_counter;
use tracing::{debug, error};

use crate::{
    health::*,
    mastodon::*,
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
            Ok(status) => match process_status(postgres.clone(), status)
                .time_as("mastodon.process_status")
                .await
            {
                Ok(_) => continue,
                Err(e) => {
                    error!("Error while processing update: {e}");
                    continue;
                }
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

    let user = postgres
        .fetch_or_create_user(instance.id, nip05.clone())
        .await?;

    if postgres.is_user_blacklisted(user.id).await? {
        debug!("Skipping update {:?} because user is blacklisted", &status);
        increment_counter!(EVENTS_SKIPPED, "visibility" => visibility_text, "reason" => "user_blacklist");

        return Ok(());
    }

    let profile = Profile::build(instance.id, user.id, &status)?;

    postgres
        .listener()
        .push(ScheduledPost {
            content: status.content,
            instance_id: instance.id,
            user_id: user.id,
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
