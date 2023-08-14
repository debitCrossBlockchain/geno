use serde::{Deserialize, Serialize};
use std::thread::sleep;
use std::time::{Duration, SystemTime};

/// Gives the duration since the Unix epoch, notice the expect.
pub fn duration_since_epoch() -> Duration {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("System time is before the UNIX_EPOCH")
}

/// Encapsulates calling a function every so often
pub struct Ticker {
    last: Duration,
    timeout: Duration,
}

impl Ticker {
    pub fn new(period: Duration) -> Self {
        Ticker {
            last: duration_since_epoch(),
            timeout: period,
        }
    }

    // Do some work if the timeout has expired
    pub fn tick<T: FnMut()>(&mut self, mut callback: T) {
        let elapsed = duration_since_epoch() - self.last;
        if elapsed >= self.timeout {
            callback();
            self.last = duration_since_epoch();
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
enum TimeoutState {
    Active,
    Inactive,
    Expired,
}

/// A timer that expires after a given duration
/// Check back on this timer every so often to see if it's expired
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Timeout {
    state: TimeoutState,
    duration: Duration,
    #[serde(with = "serde_millis")]
    start: Duration,
}

impl Timeout {
    pub fn new(duration: Duration) -> Self {
        Timeout {
            state: TimeoutState::Inactive,
            duration,
            start: duration_since_epoch(),
        }
    }

    /// Update the timer state, and check if the timer is expired
    pub fn check_expired(&mut self) -> bool {
        if self.state == TimeoutState::Active && duration_since_epoch() - self.start > self.duration
        {
            self.state = TimeoutState::Expired;
        }
        match self.state {
            TimeoutState::Active | TimeoutState::Inactive => false,
            TimeoutState::Expired => true,
        }
    }

    pub fn start(&mut self) {
        self.state = TimeoutState::Active;
        self.start = duration_since_epoch();
    }

    pub fn stop(&mut self) {
        self.state = TimeoutState::Inactive;
        self.start = duration_since_epoch();
    }

    pub fn reset(&mut self) {
        self.stop();
        self.start();
    }

    #[cfg(test)]
    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn is_active(&self) -> bool {
        self.state == TimeoutState::Active
    }
}

/// With exponential backoff, repeatedly try the callback until the result is `Ok`
pub fn retry_until_ok<T, E, F: FnMut() -> Result<T, E>>(
    base: Duration,
    max: Duration,
    mut callback: F,
) -> T {
    let mut delay = base;
    loop {
        match callback() {
            Ok(res) => return res,
            Err(_) => {
                sleep(delay);
                // Only increase delay if it's less than the max
                if delay < max {
                    delay = delay
                        .checked_mul(2)
                        .unwrap_or_else(|| Duration::from_millis(std::u64::MAX));
                    // Make sure the max isn't exceeded
                    if delay > max {
                        delay = max;
                    }
                }
            }
        }
    }
}

pub struct Timestamp {}

impl Timestamp {
    pub fn timestamp_secs() -> i64 {
        duration_since_epoch().as_secs() as i64
    }

    pub fn timestamp_millis() -> i64 {
        duration_since_epoch().as_millis() as i64
    }

    pub fn timestamp_micros() -> i64 {
        duration_since_epoch().as_micros() as i64
    }

    pub fn timestamp_nanos() -> i64 {
        duration_since_epoch().as_nanos() as i64
    }

    pub fn from_secs(secs: i64) -> Duration {
        Duration::from_secs(secs as u64)
    }

    pub fn from_millis(millis: i64) -> Duration {
        Duration::from_millis(millis as u64)
    }

    pub fn from_micros(micros: i64) -> Duration {
        Duration::from_micros(micros as u64)
    }

    pub fn from_nanos(nanos: i64) -> Duration {
        Duration::from_nanos(nanos as u64)
    }

    pub fn check_timeout_millis(dur: Duration, start: i64) -> bool {
        if duration_since_epoch().as_millis() - start as u128 > dur.as_millis() {
            true
        } else {
            false
        }
    }

    pub fn check_timeout_secs(dur: Duration, start: i64) -> bool {
        if duration_since_epoch().as_secs() - start as u64 > dur.as_secs() {
            true
        } else {
            false
        }
    }

    pub fn check_timeout_nanos(dur: Duration, start: i64) -> bool {
        if duration_since_epoch().as_nanos() - start as u128 > dur.as_nanos() {
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOLERANCE_MILLIS: u64 = 10;

    macro_rules! assert_tolerance {
        ($val1:expr, $val2:expr, $tol:expr) => {
            if $val2 > $val1 && $val2 - $val1 > $tol {
                panic!(
                    "Value is not within tolerance ({:?} - {:?} > {:?})",
                    $val2, $val1, $tol
                );
            }
            if $val1 > $val2 && $val1 - $val2 > $tol {
                panic!(
                    "Value is not within tolerance ({:?} - {:?} > {:?})",
                    $val1, $val2, $tol
                );
            }
        };
    }

    /// Tell the ticker to wait for 100ms, then see if it actually waited 100 +/- 5ms
    #[test]
    fn ticker() {
        let time = Duration::from_millis(100);
        let mut t = Ticker::new(time);
        let start_time = duration_since_epoch();
        let mut end_time = duration_since_epoch();
        let mut triggered = false;
        while !triggered {
            t.tick(|| {
                end_time = duration_since_epoch();
                triggered = true;
            })
        }
        assert_tolerance!(
            end_time - start_time,
            time,
            Duration::from_millis(TOLERANCE_MILLIS)
        );
    }

    /// Create a Timeout that lasts for 100ms and check that it expires anytime after 100ms have
    /// passed. Check whether `.start()` and `.stop()` work as expected.
    #[test]
    fn timeout() {
        let start_time = duration_since_epoch();
        let mut t = Timeout::new(Duration::from_millis(100));
        assert_eq!(t.state, TimeoutState::Inactive);
        assert_tolerance!(t.start, start_time, Duration::from_millis(TOLERANCE_MILLIS));

        t.start();
        assert_eq!(t.state, TimeoutState::Active);
        ::std::thread::sleep(Duration::from_millis(110));

        assert!(t.check_expired());
        assert_eq!(t.state, TimeoutState::Expired);

        t.stop();
        assert_eq!(t.state, TimeoutState::Inactive);
    }

    /// Retry a function that fails three times and succeeds on the 4th try with the
    /// `retry_until_ok` method, a 10ms base, and 20ms max; the total time should be 50ms.
    #[test]
    fn retry() {
        let start_time = duration_since_epoch();
        let vec = vec![Err(()), Err(()), Err(()), Ok(())];
        let mut iter = vec.iter().cloned();
        retry_until_ok(Duration::from_millis(10), Duration::from_millis(20), || {
            iter.next().unwrap()
        });
        let end_time = duration_since_epoch();
        assert_tolerance!(
            end_time - start_time,
            Duration::from_millis(50),
            Duration::from_millis(TOLERANCE_MILLIS)
        );
    }
}
