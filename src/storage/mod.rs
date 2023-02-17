use eyre::Result;
use nostr_sdk::prelude::Keys;
use uuid::Uuid;

pub mod postgres;

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

#[async_trait::async_trait]
pub trait StorageProvider {
    async fn health_check(&self) -> Result<()>;
    async fn fetch_servers(&self) -> Result<Vec<MastodonServer>>;
    async fn update_profile(&self, user: Profile) -> Result<ChangeResult>;
    async fn add_post(&self, post: MastodonPost) -> Result<ChangeResult>;
    async fn delete_post(&self, mastodon_id: String) -> Result<Option<(Uuid, String)>>;
    async fn fetch_credentials(&self, user_id: Uuid) -> Result<Keys>;
    async fn fetch_or_create_instance<T: Into<String> + Send>(
        &self,
        instance_url: T,
    ) -> Result<Uuid>;
    async fn fetch_or_create_user<T: Into<String> + Send>(
        &self,
        instance_id: Uuid,
        username: T,
    ) -> Result<Uuid>;
}
