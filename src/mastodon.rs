use std::time::Duration;

use eyre::{ErrReport, Result};
use futures_util::TryStreamExt;
use mastodon_async::prelude::{Event, StatusId};
use tokio::{
    sync::broadcast::{self, Receiver, Sender},
    task, time,
};

use crate::{
    health::{Timeable, Timeoutable},
    postgres::MastodonServer,
};

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

                let mut stream = Box::pin(
                    client
                        .stream_public()
                        .time_as("mastodon.client_stream_public_init")
                        .await?
                        .into_stream(),
                );

                while let Ok(Ok(Some(event))) = stream
                    .try_next()
                    .time_as("mastodon.client_stream_get_next")
                    .with_timeout(Duration::from_secs(20))
                    .await
                {
                    sender
                        .clone()
                        .send(event)
                        .expect("error: mastodon sender has no subscribers");
                }

                drop(stream);

                Ok::<_, ErrReport>(())
            };

            loop {
                match task().time_as("mastodon.task_lifecycle").await {
                    Ok(_) => continue,
                    Err(e) => {
                        println!("Got an error: {e}");
                        println!("Stream died, restarting...");

                        time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }
                }
            }
        });

        Ok(self.sender.subscribe())
    }
}
