use eyre::{ErrReport, Result};
use sqlx::{postgres::PgListener, Pool, Postgres};
use tokio::sync::broadcast::{self, Receiver, Sender};
use uuid::Uuid;

use crate::health::Timeable;

use super::ChangeResult;

#[derive(Debug, Clone, sqlx::Type)]
#[sqlx(type_name = "scheduled_post_status")]
#[sqlx(rename_all = "lowercase")]
pub enum ScheduledPostStatus {
    New,
    Running,
    Errored,
    Finished,
}

#[derive(Debug, Clone)]
pub struct ScheduledPost {
    pub user_id: Uuid,
    pub instance_id: Uuid,
    pub mastodon_id: String,
    pub content: String,
    pub profile_name: String,
    pub profile_display_name: String,
    pub profile_about: String,
    pub profile_picture: String,
    pub profile_nip05: String,
    pub profile_banner: String
}

pub struct JobQueue {
    pool: Pool<Postgres>,
    sender: Sender<ScheduledPost>,
    _receiver: Receiver<ScheduledPost>,
}

async fn poll_job(pool: &Pool<Postgres>) -> Result<Option<ScheduledPost>> {
    Ok(sqlx::query_as!(
        ScheduledPost,
        r#"
             update scheduled_posts set status = 'running'
             where id = (
                select id from scheduled_posts where status = 'new'
                order by id
                for update skip locked
                limit 1
             ) returning 
                user_id, instance_id, mastodon_id, content,
                profile_name, profile_display_name, profile_about,
                profile_picture, profile_nip05, profile_banner
            "#
    )
    .fetch_optional(pool)
    .time_as("postgres.job_queue.poll_job")
    .await?)
}

impl JobQueue {
    pub fn new(pool: Pool<Postgres>) -> Self {
        let (sender, _receiver) = broadcast::channel(128);

        Self {
            pool,
            sender,
            _receiver,
        }
    }

    pub async fn push(&self, post: ScheduledPost) -> Result<()> {
        sqlx::query!(
            r#"
            insert into scheduled_posts
                (user_id, instance_id, mastodon_id, content, status,
                 profile_name, profile_display_name, profile_about,
                profile_picture, profile_nip05, profile_banner)
            values
                ($1, $2, $3, $4, 'new', $5, $6, $7, $8, $9, $10)
            on conflict do nothing"#,
            post.user_id,
            post.instance_id,
            post.mastodon_id,
            post.content,
            post.profile_name,
            post.profile_display_name,
            post.profile_about,
            post.profile_picture,
            post.profile_nip05,
            post.profile_banner,
        )
        .execute(&self.pool)
        .time_as("postgres.job_queue.push")
        .await?;

        Ok(())
    }

    pub async fn finish(&self, mastodon_id: String) -> Result<ChangeResult> {
        sqlx::query_as!(
            super::ResultContainer,
            r#"
            update scheduled_posts set status = 'finished' where status = 'running' and mastodon_id = $1
            returning id::text as result
            "#,
            mastodon_id
        )
        .fetch_one(&self.pool)
            .time_as("postgres.job_queue.finish")
        .await?
        .to_change_result()
    }

    pub async fn error(&self, mastodon_id: String) -> Result<ChangeResult> {
        sqlx::query_as!(
            super::ResultContainer,
            r#"
            update scheduled_posts set status = 'errored' where status = 'running' and mastodon_id = $1
            returning id::text as result
            "#,
            mastodon_id
        )
        .fetch_one(&self.pool)
            .time_as("postgres.job_queue.finish")
        .await?
        .to_change_result()
    }

    pub async fn update_stream(&self) -> Result<Receiver<ScheduledPost>> {
        let sender = self.sender.clone();
        let pool = self.pool.clone();

        tokio::task::spawn(async move {
            let sender = sender.clone();

            let mut listener = PgListener::connect_with(&pool).await?;
            listener.listen("scheduled_posts_status_channel").await?;

            loop {
                while let Ok(_) = listener.recv().time_as("postgres.job_queue.recv").await {
                    if let Some(job) = poll_job(&pool).await? {
                        sender.send(job)?;
                    }
                }
            }

            #[allow(unreachable_code)]
            Ok::<_, ErrReport>(())
        });

        Ok(self.sender.subscribe())
    }
}
