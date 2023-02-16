use std::{
    future::Future,
    time::{Duration, Instant},
};

use eyre::Result;
use metrics::{describe_counter, register_counter, register_histogram, increment_counter};
use tokio::time::timeout_at;

pub const TASK_COUNT: &'static str = "nostodon_task_count";
pub const TASK_TIMEOUT_COUNT: &'static str = "nostodon_task_timeout_count";
pub const TASK_TIME_ELAPSED: &'static str = "nostodon_task_time_elapsed_ms";
pub const TASK_TIME_ELAPSED_HISTOGRAM: &'static str = "nostodon_task_elapsed_histogram";
pub const EVENTS_PROCESSED: &'static str = "nostodon_mastodon_events_processed_count";

pub struct Provider;

impl Provider {
    pub fn setup() {
        describe_counter!(
            EVENTS_PROCESSED,
            "Counter of events that have been processed since start"
        );

        describe_counter!(
            TASK_TIME_ELAPSED,
            "The cumulative amount of time taken to run a task"
        );

        describe_counter!(
            TASK_TIME_ELAPSED_HISTOGRAM,
            "The historigram for the amount of time (and percentiles) of each task"
        );
    }
}

#[async_trait::async_trait]
pub trait Timeable<T> {
    async fn time_as<S: Into<String> + Send>(self, task_name: S) -> T;
}

#[async_trait::async_trait]
impl<A, B> Timeable<A> for B
where
    B: Future<Output = A> + Send,
{
    async fn time_as<S: Into<String> + Send>(self, task_name: S) -> A {
        let task_name_str = task_name.into();

        println!("Running task `{}`...", &task_name_str);

        let start = Instant::now();
        let result = self.await;
        let diff = Instant::now() - start;

        increment_counter!(TASK_COUNT, "task" => task_name_str.clone());
        register_counter!(TASK_TIME_ELAPSED, "task" => task_name_str.clone())
            .increment(diff.as_millis() as u64);
        register_histogram!(TASK_TIME_ELAPSED_HISTOGRAM, "task" => task_name_str.clone())
            .record(diff.as_millis() as f64);

        println!(
            "Finished {} [OK] (took {}ms)",
            &task_name_str,
            diff.as_millis()
        );
        result
    }
}

#[async_trait::async_trait]
pub trait Timeoutable<T> {
    async fn with_timeout(self, deadline: Duration) -> Result<T>;
}

#[async_trait::async_trait]
impl<A, B> Timeoutable<A> for B
where
    B: Future<Output = A> + Send,
{
    async fn with_timeout(self, deadline: Duration) -> Result<A> {
        let result = timeout_at((Instant::now() + deadline).into(), self).await;

        if result.is_err() {
            increment_counter!(TASK_TIMEOUT_COUNT);
        }

        Ok(result?)
    }
}