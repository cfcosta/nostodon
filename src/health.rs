use std::{
    future::Future,
    time::{Duration, Instant},
};

use eyre::Result;
use metrics::{describe_counter, increment_counter, register_counter, register_histogram};
use tokio::time::timeout_at;
use tracing::trace;

pub const EVENTS_SCHEDULED: &str = "nostodon_mastodon_events_scheduled_count";
pub const EVENTS_SKIPPED: &str = "nostodon_mastodon_events_skipped_count";
pub const TASK_COUNT: &str = "nostodon_task_count";
pub const TASK_TIMEOUT_COUNT: &str = "nostodon_task_timeout_count";
pub const TASK_TIME_ELAPSED: &str = "nostodon_task_time_elapsed_ms";
pub const TASK_TIME_ELAPSED_HISTOGRAM: &str = "nostodon_task_elapsed_histogram";
pub const POSTS_CREATED: &str = "nostodon_posts_created_count";
pub const PROFILES_UPDATED: &str = "nostodon_profiles_updated_count";

pub struct Provider;

impl Provider {
    pub fn setup() {
        describe_counter!(
            EVENTS_SKIPPED,
            "Counter of events that have been skipped because of some rule"
        );

        describe_counter!(
            EVENTS_SCHEDULED,
            "Counter of events that have been scheduled to be posted"
        );

        describe_counter!(TASK_COUNT, "Counter of tasks that have been processed");

        describe_counter!(
            TASK_TIMEOUT_COUNT,
            "Counter of tasks that have been timed out"
        );

        describe_counter!(
            TASK_TIME_ELAPSED,
            "The cumulative amount of time taken to run a task"
        );

        describe_counter!(
            TASK_TIME_ELAPSED_HISTOGRAM,
            "The historigram for the amount of time (and percentiles) of each task"
        );

        describe_counter!(POSTS_CREATED, "Number of posts that have been created");

        describe_counter!(PROFILES_UPDATED, "Number of posts that have been created");
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
        let start = Instant::now();
        let result = self.await;
        let diff = Instant::now() - start;

        increment_counter!(TASK_COUNT, "task" => task_name_str.clone());
        register_counter!(TASK_TIME_ELAPSED, "task" => task_name_str.clone())
            .increment(diff.as_millis() as u64);
        register_histogram!(TASK_TIME_ELAPSED_HISTOGRAM, "task" => task_name_str.clone())
            .record(diff.as_millis() as f64);

        trace!(
            name = &task_name_str,
            time_taken_ms = diff.as_millis(),
            "Task finished",
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
