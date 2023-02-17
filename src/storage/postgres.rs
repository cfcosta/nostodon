use clap::Parser;
use eyre::Result;
use nostr_sdk::prelude::{FromSkStr, Keys, ToBech32};
use sqlx::{postgres::PgPoolOptions, Pool};
use uuid::Uuid;

use crate::{health::Timeable, storage::MastodonPostStatus};

use super::{ChangeResult, MastodonPost, MastodonServer, Profile, StorageProvider};

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
pub struct Postgres {
    pool: Pool<sqlx::Postgres>,
}

impl Postgres {
    pub async fn init(config: PostgresConfig) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(16)
            .connect(&config.url)
            .time_as("storage.connect")
            .await?;

        Ok(Self { pool })
    }
}

#[async_trait::async_trait]
impl StorageProvider for Postgres {
    async fn health_check(&self) -> Result<()> {
        sqlx::query("select 1")
            .execute(&self.pool)
            .time_as("storage.health_check")
            .await?;

        Ok(())
    }

    async fn fetch_servers(&self) -> Result<Vec<MastodonServer>> {
        Ok(sqlx::query_as!(MastodonServer, "select instance_url, client_key, client_secret, redirect_url, token from mastodon_servers")
            .fetch_all(&self.pool).time_as("storage.fetch_servers").await?)
    }

    async fn update_profile(&self, profile: Profile) -> Result<ChangeResult> {
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
        .time_as("storage.update_profile")
        .await?
        .to_change_result()
    }

    async fn add_post(&self, post: MastodonPost) -> Result<ChangeResult> {
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
        .time_as("storage.fetch_or_create_instance")
        .await?;

        match result {
            Some(id) => Ok(ChangeResult::Changed(id.result)),
            None => Ok(ChangeResult::Unchanged),
        }
    }

    async fn delete_post(&self, mastodon_id: String) -> Result<Option<(Uuid, String)>> {
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
        .time_as("storage.fetch_or_create_instance")
        .await?
        .map(|x| (x.user_id, x.nostr_id));

        Ok(result)
    }

    async fn fetch_credentials(&self, user_id: Uuid) -> Result<Keys> {
        let result = sqlx::query_as!(
            KeysContainer,
            "select nostr_public_key, nostr_private_key from users where id = $1 limit 1",
            user_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(Keys::from_sk_str(&result.nostr_private_key)?)
    }

    async fn fetch_or_create_instance<T: Into<String> + Send>(
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
        .time_as("storage.fetch_or_create_instance")
        .await?;

        Ok(result.result)
    }

    async fn fetch_or_create_user<T: Into<String> + Send>(
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
        .time_as("storage.fetch_or_create_user")
        .await?;

        Ok(result.result)
    }
}
