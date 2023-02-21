use eyre::{ErrReport, Result};
use metrics::increment_counter;
use tracing::error;

use crate::{
    health::*,
    nostr::{Nostr, Note},
    postgres::{job_queue::ScheduledPost, *},
};

async fn process_item(postgres: Postgres, item: ScheduledPost) -> Result<()> {
    let creds = postgres.fetch_credentials(item.user_id).await?;
    let nostr = Nostr::connect(&postgres, creds).await?;

    let profile: Profile = item.clone().into();
    if postgres.update_profile(&profile).await?.changed() {
        nostr.update_user_profile(profile).await?;
        increment_counter!(PROFILES_UPDATED);
    }

    let event_id = nostr
        .publish(Note::new_text(html2md::parse_html(&item.content)))
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

    Ok(())
}

pub async fn spawn(postgres: Postgres) -> Result<()> {
    let mut stream = postgres.listener().update_stream().await?;

    let task = |postgres: Postgres, item: ScheduledPost| async move {
        match process_item(postgres.clone(), item.clone()).await {
            Ok(_) => {
                postgres.listener().finish(item.mastodon_id).await?;
            }
            Err(e) => {
                postgres
                    .listener()
                    .error(item.mastodon_id, e.to_string())
                .await?;
            }
        }

        Ok::<_, ErrReport>(())
    };

    loop {
        let item = match stream.recv().await {
            Ok(item) => item,
            Err(_) => continue
        };

        match task(postgres.clone(), item).await {
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
