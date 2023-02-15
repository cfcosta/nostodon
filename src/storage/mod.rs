use eyre::Result;
use uuid::Uuid;

pub mod postgres;

pub struct Profile {
    pub instance_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub display_name: String,
    pub about: String,
    pub picture: String,
    pub nip05: String,
}

pub enum MastodonPostStatus {
    Posted,
    Deleted,
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

#[async_trait::async_trait]
pub trait StorageProvider {
    async fn health_check(&self) -> Result<()>;
    async fn update_profile(&self, user: Profile) -> Result<ChangeResult>;
    async fn add_post(&self, post: MastodonPost) -> Result<ChangeResult>;
    async fn delete_post(&self, mastodon_id: String) -> Result<ChangeResult>;
}
