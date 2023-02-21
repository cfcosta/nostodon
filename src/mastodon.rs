use std::time::Duration;

use eyre::{ErrReport, Result};
use mastodon_async::prelude::{Status, StatusId};
use tokio::{
    sync::broadcast::{self, Receiver, Sender},
    task, time,
};
use tracing::error;

use crate::{health::Timeable, postgres::MastodonServer};

#[async_trait::async_trait]
pub trait MastodonClient {
    type StatusId;

    async fn update_stream(&self) -> Result<Receiver<Status>>;
}

pub struct Mastodon {
    server: MastodonServer,
    sender: Sender<Status>,
    _receiver: Receiver<Status>,
}

impl Mastodon {
    pub fn connect(server: &MastodonServer) -> Result<Self> {
        let server = server.clone();
        let (sender, _receiver) = broadcast::channel(128);

        Ok(Self {
            server,
            sender,
            _receiver,
        })
    }
}

#[async_trait::async_trait]
impl MastodonClient for Mastodon {
    type StatusId = StatusId;

    async fn update_stream(&self) -> Result<Receiver<Status>> {
        let sender = self.sender.clone();
        let server = self.server.clone();

        task::spawn(async move {
            let task = || async {
                let sender = sender.clone();
                let client = mastodon_async::Mastodon::from(server.clone().as_data());
                let events = client
                    .get_public_timeline(false)
                    .time_as("mastodon.get_public_timeline")
                    .await?;

                for event in events {
                    sender
                        .clone()
                        .send(event)
                        .expect("error: mastodon sender has no subscribers");
                }

                Ok::<_, ErrReport>(())
            };

            loop {
                match task().await {
                    Ok(_) => {}
                    Err(e) => {
                        error!(error = %e, server = server.instance_url, "Got an error while getting updates");
                    }
                }

                time::sleep(Duration::from_secs(2)).await;
            }
        });

        Ok(self.sender.subscribe())
    }
}
