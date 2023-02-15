use std::time::Duration;

use clap::Parser;
use eyre::{ErrReport, Result};
use futures_util::TryStreamExt;
use mastodon_async::prelude::{Event, StatusId};
use tokio::{
    sync::broadcast::{self, Receiver, Sender},
    task, time,
};

use crate::health::{Timeable, Timeoutable};

#[derive(Debug, Clone, Parser)]
pub struct MastodonConfig {
    #[clap(short = 'u', long = "base-url", env = "NOSTODON_MASTODON_BASE_URL")]
    base_url: String,
    #[clap(short = 'i', long = "client-key", env = "NOSTODON_MASTODON_CLIENT_KEY")]
    client_key: String,
    #[clap(
        short = 's',
        long = "client-secret",
        env = "NOSTODON_MASTODON_CLIENT_SECRET"
    )]
    client_secret: String,
    #[clap(short = 't', long = "token", env = "NOSTODON_MASTODON_TOKEN")]
    token: String,
    #[clap(
        short = 'e',
        long = "redirect-url",
        env = "NOSTODON_MASTODON_REDIRECT_URL"
    )]
    redirect: String,
}

impl MastodonConfig {
    pub fn as_data(self) -> mastodon_async::Data {
        mastodon_async::Data {
            base: self.base_url.into(),
            client_id: self.client_key.into(),
            client_secret: self.client_secret.into(),
            redirect: self.redirect.into(),
            token: self.token.into(),
        }
    }
}

#[async_trait::async_trait]
pub trait MastodonClient {
    type EventId;

    async fn update_stream(&self) -> Result<Receiver<Event>>;
}

pub struct Mastodon {
    config: MastodonConfig,
    sender: Sender<Event>,
    _receiver: Receiver<Event>,
}

impl Mastodon {
    pub fn connect(config: MastodonConfig) -> Result<Self> {
        let (sender, _receiver) = broadcast::channel(128);

        Ok(Self {
            config,
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
        let config = self.config.clone();

        task::spawn(async move {
            let task = || async {
                let sender = sender.clone();
                let client = mastodon_async::Mastodon::from(config.clone().as_data());

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
                match task().await {
                    Ok(_) => continue,
                    Err(e) => {
                        println!("Got an error: {}", e);
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
