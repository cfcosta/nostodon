use eyre::{ Result, eyre };
use mastodon_async::prelude::Status;
use nostr_sdk::prelude::Keys;
use uuid::Uuid;

use crate::util::extract_instance_url;

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

impl Profile {
    pub fn build(
        instance_id: Uuid,
        user_id: Uuid,
        status: &Status
    ) -> Result<Self> {
        let url = status.url.as_ref().ok_or_else(|| eyre!("failed to extract instance url"))?;
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
