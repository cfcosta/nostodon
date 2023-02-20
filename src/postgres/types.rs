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
