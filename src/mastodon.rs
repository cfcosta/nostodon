use std::time::Duration;

use eyre::{ErrReport, Result};
use mastodon_async::prelude::{Event, StatusId};
use tokio::{
    sync::broadcast::{self, Receiver, Sender},
    task, time,
};
use tracing::debug;

use crate::{health::Timeable, postgres::MastodonServer};

#[async_trait::async_trait]
pub trait MastodonClient {
    type EventId;

    async fn update_stream(&self) -> Result<Receiver<Event>>;
}

pub struct Mastodon {
    server: MastodonServer,
    sender: Sender<Event>,
    _receiver: Receiver<Event>,
}

impl Mastodon {
    pub fn connect(server: MastodonServer) -> Result<Self> {
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
    type EventId = StatusId;

    async fn update_stream(&self) -> Result<Receiver<Event>> {
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
                        .send(Event::Update(event))
                        .expect("error: mastodon sender has no subscribers");
                }

                Ok::<_, ErrReport>(())
            };

            loop {
                match task().await {
                    Ok(_) => {}
                    Err(e) => {
                        debug!("Got an error: {e}");
                    }
                }

                time::sleep(Duration::from_secs(2)).await;
            }
        });

        Ok(self.sender.subscribe())
    }
}
