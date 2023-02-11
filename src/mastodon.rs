use std::time::Duration;

use clap::Parser;
use eyre::{ErrReport, Result};
use futures_util::TryStreamExt;
use mastodon_async::prelude::{Event, StatusId};
use tokio::{
    sync::broadcast::{self, Receiver, Sender},
    task,
    time::sleep,
};

use crate::metrics::Timeable;

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
    sender: Sender<Event>,
    _receiver: Receiver<Event>,
    client: mastodon_async::Mastodon,
}

impl Mastodon {
    pub fn connect(config: MastodonConfig) -> Result<Self> {
        let (sender, _receiver) = broadcast::channel(16);

        Ok(Self {
            sender,
            _receiver,
            client: mastodon_async::Mastodon::from(config.as_data()),
        })
    }
}

#[async_trait::async_trait]
impl MastodonClient for Mastodon {
    type EventId = StatusId;

    async fn update_stream(&self) -> Result<Receiver<Event>> {
        let sender = self.sender.clone();
        let client = self.client.clone();

        task::spawn(async move {
            loop {
                println!("Starting client");

                let stream = match client
                    .stream_public()
                    .time_as("mastodon.client_stream_public_init")
                    .await
                {
                    Ok(v) => v,
                    _ => {
                        println!("Could not connect to public stream, trying again in 0.5s...");
                        sleep(Duration::from_millis(500)).await;

                        continue;
                    }
                };

                stream
                    .try_for_each(|event| async {
                        sender
                            .clone()
                            .send(event)
                            .expect("error: mastodon sender has no subscribers");

                        Ok(())
                    })
                    .await?;

                println!("Mastodon stream crashed, restarting...");
                sleep(Duration::from_millis(500)).await;
            }

            #[allow(unreachable_code)]
            Ok::<_, ErrReport>(())
        });

        Ok(self.sender.subscribe())
    }
}
