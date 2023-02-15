use clap::Parser;
use eyre::Result;
use sqlx::{postgres::PgPoolOptions, Pool};

use crate::metrics::Timeable;

use super::{ChangeResult, MastodonPost, Profile, StorageProvider};

#[derive(Debug, Clone, Parser)]
pub struct PostgresConfig {
    #[clap(short = 'd', long = "database-url", env = "NOSTODON_DATABASE_URL")]
    pub url: String,
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

    async fn update_profile(&self, user: Profile) -> Result<ChangeResult> {
        todo!()
    }
    async fn add_post(&self, post: MastodonPost) -> Result<ChangeResult> {
        todo!()
    }
    async fn delete_post(&self, mastodon_id: String) -> Result<ChangeResult> {
        todo!()
    }
}
