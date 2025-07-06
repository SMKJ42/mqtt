use std::time::Duration;

pub mod sender;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum QoS1Stage {
    Origin,
    Publish,
    Ack,
}

impl Instant for std::time::Instant {
    fn duration_since(&self, instant: &std::time::Instant) -> std::time::Duration {
        return self.duration_since(*instant);
    }
    fn now() -> Self {
        return std::time::Instant::now();
    }
}

#[derive(Debug, Clone, Ord, PartialEq, Eq, PartialOrd)]
pub struct RetryDuration {
    dur: core::time::Duration,
}

impl Default for RetryDuration {
    fn default() -> Self {
        return Self {
            dur: Duration::from_millis(200),
        };
    }
}

impl ExponentialBackoff for RetryDuration {
    fn exponential(&self) -> core::time::Duration {
        return *self.dur.checked_mul(2).get_or_insert(Duration::MAX);
    }

    fn inner(&self) -> core::time::Duration {
        return self.dur;
    }

    fn set_duration(&mut self, dur: Duration) {
        self.dur = dur;
    }
}

pub trait Instant: Ord {
    fn now() -> Self;
    fn duration_since(&self, instant: &Self) -> core::time::Duration;
}

/// This is useful for sessions that utilize a stream where packet delivery is not garenteed.
///
/// For example, TCP guarantees message delivery while UDP does not. UDP is not an inherently supported protocol,
/// however, I do plan on supporting more messaging protocols.
///
/// ## Examples
///
/// ```
/// use mqtt_core::msg_assurance::ExponentialBackoff;
/// use core::time::Duration;
///
/// #[derive(PartialEq, Eq, PartialOrd, Ord)]
/// pub struct MyDuration {
///     dur: Duration,
/// }
///
/// impl Default for MyDuration {
///     fn default() -> Self {
///         return Self {
///             dur: Duration::from_millis(100),
///         };
///     }
/// }
///
/// impl ExponentialBackoff for MyDuration {
///     fn exponential(&self) -> core::time::Duration {
///         return *self.dur.checked_mul(2).get_or_insert(Duration::MAX);
///     }
///
///     fn inner(&self) -> core::time::Duration {
///         return self.dur;
///         }
///
///     fn set_duration(&mut self, dur: Duration) {
///         self.dur = dur;
///     }
/// }
/// ```
pub trait ExponentialBackoff: Ord + Default {
    /// Returns a new duration with the exponential backoff function applied.
    /// Returns None if an overflow occured.
    fn exponential(&self) -> core::time::Duration;

    /// Function that receives the inner Duration value.
    fn inner(&self) -> core::time::Duration;

    fn set_duration(&mut self, dur: Duration);
}
