use clap::Parser;
use eyre::{eyre, Result};
use mastodon_async::prelude::Status;
use nostr_sdk::prelude::{FromSkStr, Keys, ToBech32};
use sqlx::{postgres::PgPoolOptions, Pool};
use uuid::Uuid;

pub mod job_queue;

use crate::{health::Timeable, util::extract_instance_url};

use self::job_queue::JobQueue;

#[derive(Debug, Clone, Parser)]
pub struct PostgresConfig {
    #[clap(short = 'd', long = "database-url", env = "NOSTODON_DATABASE_URL")]
    pub url: String,
}

pub struct IdContainer {
    pub result: Uuid,
}

pub struct ResultContainer {
    pub result: Option<String>,
}

pub struct KeysContainer {
    pub nostr_public_key: String,
    pub nostr_private_key: String,
}

impl ResultContainer {
    pub fn to_change_result(&self) -> Result<ChangeResult> {
        let res = match self.result.as_deref() {
            Some("unchanged") | None => ChangeResult::Unchanged,
            Some(id) => ChangeResult::Changed(Uuid::parse_str(id)?),
        };

        Ok(res)
    }
}

#[derive(Debug, Clone)]
pub struct Profile {
    pub instance_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub display_name: String,
    pub about: String,
    pub picture: String,
    pub nip05: String,
    pub banner: String,
}

impl Profile {
    pub fn build(instance_id: Uuid, user_id: Uuid, status: &Status) -> Result<Self> {
        let url = status
            .url
            .as_ref()
            .ok_or_else(|| eyre!("failed to extract instance url"))?;
        let instance_url = extract_instance_url(url)?;

        let nip05 = format!(
            "{}.{}",
            status.account.username.clone(),
            instance_url.host().unwrap()
        );

        Ok(Self {
            instance_id,
            name: status.account.username.clone(),
            display_name: status.account.display_name.clone(),
            about: status.account.note.clone(),
            user_id,
            nip05,
            picture: status.account.avatar.clone(),
            banner: status.account.header.clone(),
        })
    }
}

#[derive(sqlx::Type)]
#[sqlx(type_name = "mastodon_post_status")]
#[sqlx(rename_all = "lowercase")]
pub enum MastodonPostStatus {
    Posted,
    Deleted,
}

#[derive(Debug, Clone)]
pub struct MastodonServer {
    pub instance_url: String,
    pub client_key: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub token: String,
}

impl MastodonServer {
    pub fn as_data(self) -> mastodon_async::Data {
        mastodon_async::Data {
            base: self.instance_url.into(),
            client_id: self.client_key.into(),
            client_secret: self.client_secret.into(),
            redirect: self.redirect_url.into(),
            token: self.token.into(),
        }
    }
}

pub struct MastodonPost {
    pub instance_id: Uuid,
    pub user_id: Uuid,
    pub mastodon_id: String,
    pub nostr_id: String,
    pub status: MastodonPostStatus,
}

pub enum ChangeResult {
    Changed(Uuid),
    Unchanged,
}

impl ChangeResult {
    pub fn changed(&self) -> bool {
        matches!(self, ChangeResult::Changed(_))
    }
}

#[derive(Debug, Clone)]
pub struct Postgres {
    pool: Pool<sqlx::Postgres>,
}

impl Postgres {
    pub async fn init(config: PostgresConfig) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(16)
            .connect(&config.url)
            .time_as("postgres.connect")
            .await?;

        Ok(Self { pool })
    }

    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("select 1")
            .execute(&self.pool)
            .time_as("postgres.health_check")
            .await?;

        Ok(())
    }

    pub async fn fetch_servers(&self) -> Result<Vec<MastodonServer>> {
        Ok(sqlx::query_as!(MastodonServer, "select instance_url, client_key, client_secret, redirect_url, token from mastodon_servers")
            .fetch_all(&self.pool).time_as("postgres.fetch_servers").await?)
    }

    pub async fn update_profile(&self, profile: Profile) -> Result<ChangeResult> {
        sqlx::query_as!(
            ResultContainer,
            "insert into profiles
                (instance_id, user_id, name, display_name, about, picture, nip05)
            values
                ($1, $2, $3, $4, $5, $6, $7)
            on conflict (user_id) do update set
                name = $3, display_name = $4, about = $5, picture = $6, nip05 = $7
            returning case when xmax = 0 then id::text else 'unchanged' end as result",
            profile.instance_id,
            profile.user_id,
            profile.name,
            profile.display_name,
            profile.about,
            profile.picture,
            profile.nip05
        )
        .fetch_one(&self.pool)
        .time_as("postgres.update_profile")
        .await?
        .to_change_result()
    }

    pub async fn add_post(&self, post: MastodonPost) -> Result<ChangeResult> {
        let result = sqlx::query_as!(
            IdContainer,
            r#"insert into mastodon_posts
                (instance_id, user_id, mastodon_id, nostr_id, status)
            values ($1, $2, $3, $4, $5)
            on conflict (mastodon_id) do nothing
            returning id as result"#,
            post.instance_id,
            post.user_id,
            post.mastodon_id,
            post.nostr_id,
            post.status as MastodonPostStatus
        )
        .fetch_optional(&self.pool)
        .time_as("postgres.fetch_or_create_instance")
        .await?;

        match result {
            Some(id) => Ok(ChangeResult::Changed(id.result)),
            None => Ok(ChangeResult::Unchanged),
        }
    }

    pub async fn delete_post(&self, mastodon_id: String) -> Result<Option<(Uuid, String)>> {
        let result = sqlx::query!(
            r#"
            update mastodon_posts
            set status = 'deleted'
            where mastodon_id = $1
            returning user_id, nostr_id
            "#,
            mastodon_id
        )
        .fetch_optional(&self.pool)
        .time_as("postgres.fetch_or_create_instance")
        .await?
        .map(|x| (x.user_id, x.nostr_id));

        Ok(result)
    }

    pub async fn fetch_credentials(&self, user_id: Uuid) -> Result<Keys> {
        let result = sqlx::query_as!(
            KeysContainer,
            "select nostr_public_key, nostr_private_key from users where id = $1 limit 1",
            user_id
        )
        .fetch_one(&self.pool)
        .time_as("postgres.fetch_credentials")
        .await?;

        Ok(Keys::from_sk_str(&result.nostr_private_key)?)
    }

    pub async fn fetch_or_create_instance<T: Into<String> + Send>(
        &self,
        instance_url: T,
    ) -> Result<Uuid> {
        let instance_url: String = instance_url.into();

        let result = sqlx::query_as!(
            IdContainer,
            "insert into mastodon_instances (url, blacklisted)
            values ($1, false)
            on conflict (url) do update set
                url = $1
            returning id as result",
            instance_url
        )
        .fetch_one(&self.pool)
        .time_as("postgres.fetch_or_create_instance")
        .await?;

        Ok(result.result)
    }

    pub async fn fetch_or_create_user<T: Into<String> + Send>(
        &self,
        instance_id: Uuid,
        username: T,
    ) -> Result<Uuid> {
        let new_keypair = Keys::generate();

        let result = sqlx::query_as!(
            IdContainer,
            "insert into users
                (instance_id, nostr_public_key, nostr_private_key, mastodon_user)
            values ($1, $2, $3, $4)
            on conflict (mastodon_user) do update set instance_id = $1
            returning id as result",
            instance_id,
            new_keypair.public_key().to_bech32()?,
            new_keypair.secret_key().unwrap().to_bech32()?,
            username.into()
        )
        .fetch_one(&self.pool)
        .time_as("postgres.fetch_or_create_user")
        .await?;

        Ok(result.result)
    }

    pub fn listener(&self) -> job_queue::JobQueue {
        JobQueue::new(self.pool.clone())
    }
}
