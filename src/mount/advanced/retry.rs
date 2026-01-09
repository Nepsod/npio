//! Retry logic with configurable policies and backoff strategies.

use std::time::{Duration, Instant};
use crate::error::NpioError;
use crate::mount::advanced::{error::{ErrorClass, ErrorClassifier}, config::RetryConfig};

/// Configurable retry logic with multiple strategies.
#[derive(Debug)]
pub struct RetryPolicy {
    config: RetryConfig,
    error_classifier: ErrorClassifier,
}

impl RetryPolicy {
    /// Creates a new retry policy.
    pub fn new(config: RetryConfig) -> Self {
        Self {
            config,
            error_classifier: ErrorClassifier,
        }
    }

    /// Determines if an error should be retried.
    pub fn should_retry(&self, error: &NpioError, attempt: u32) -> bool {
        if attempt >= self.config.max_attempts {
            return false;
        }

        let class = ErrorClassifier::classify(error);
        matches!(class, ErrorClass::Transient)
    }

    /// Calculates the delay before the next retry attempt.
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        match self.config.backoff_strategy {
            BackoffStrategy::Fixed => self.config.base_delay,
            BackoffStrategy::Linear => {
                let delay = self.config.base_delay * attempt;
                std::cmp::min(delay, self.config.max_delay)
            }
            BackoffStrategy::Exponential { multiplier } => {
                let delay = Duration::from_millis(
                    (self.config.base_delay.as_millis() as f64 * multiplier.powi(attempt as i32)) as u64
                );
                std::cmp::min(delay, self.config.max_delay)
            }
            BackoffStrategy::ExponentialWithJitter { multiplier, jitter } => {
                let base_delay = Duration::from_millis(
                    (self.config.base_delay.as_millis() as f64 * multiplier.powi(attempt as i32)) as u64
                );
                // Simple jitter implementation without external rand dependency
                let jitter_amount = Duration::from_millis(
                    (base_delay.as_millis() as f64 * jitter * 0.5) as u64
                );
                let delay = base_delay + jitter_amount;
                std::cmp::min(delay, self.config.max_delay)
            }
        }
    }

    /// Gets the maximum number of attempts.
    pub fn max_attempts(&self) -> u32 {
        self.config.max_attempts
    }
}

/// Backoff strategy for retry delays.
#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    /// Fixed delay between retries.
    Fixed,
    /// Linear increase in delay.
    Linear,
    /// Exponential backoff.
    Exponential { multiplier: f64 },
    /// Exponential backoff with jitter.
    ExponentialWithJitter { multiplier: f64, jitter: f64 },
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        BackoffStrategy::ExponentialWithJitter {
            multiplier: 2.0,
            jitter: 0.1,
        }
    }
}

/// State tracking for retry operations.
#[derive(Debug)]
pub struct RetryState {
    pub attempt: u32,
    pub last_error: Option<NpioError>,
    pub next_retry: Option<Instant>,
    pub total_delay: Duration,
}

impl RetryState {
    /// Creates a new retry state.
    pub fn new() -> Self {
        Self {
            attempt: 0,
            last_error: None,
            next_retry: None,
            total_delay: Duration::ZERO,
        }
    }

    /// Records a failed attempt.
    pub fn record_failure(&mut self, error: NpioError, delay: Duration) {
        self.attempt += 1;
        self.last_error = Some(error);
        self.next_retry = Some(Instant::now() + delay);
        self.total_delay += delay;
    }

    /// Checks if it's time for the next retry.
    pub fn is_ready_for_retry(&self) -> bool {
        match self.next_retry {
            Some(next_retry) => Instant::now() >= next_retry,
            None => true,
        }
    }
}

impl Default for RetryState {
    fn default() -> Self {
        Self::new()
    }
}