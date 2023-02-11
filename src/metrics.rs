use std::{future::Future, time::Instant};

use metrics::{describe_counter, register_counter, register_histogram};

pub const TASK_TIME_ELAPSED: &'static str = "nostodon_task_time_elapsed_ms";
pub const TASK_TIME_ELAPSED_HISTOGRAM: &'static str = "nostodon_task_elapsed_histogram";

pub struct Provider;

impl Provider {
    pub fn setup() {
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
        let start = Instant::now();
        let result = self.await;
        let diff = Instant::now() - start;

        let task_name_str = task_name.into();
        register_counter!(TASK_TIME_ELAPSED, "task" => task_name_str.clone())
            .increment(diff.as_millis() as u64);
        register_histogram!(TASK_TIME_ELAPSED_HISTOGRAM, "task" => task_name_str)
            .record(diff.as_millis() as f64);
        result
    }
}
