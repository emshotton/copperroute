use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fr_router::pipeline::RouterStop;

use crate::timespan::GRACE_PERIOD_SECONDS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline {
    pub stop_at: Instant,
    pub timed_out_at: Instant,
}

impl Deadline {
    pub fn from_base(base: Instant, seconds: i64) -> Deadline {
        let stop_at = offset(base, seconds);
        Deadline {
            stop_at,
            timed_out_at: offset(stop_at, GRACE_PERIOD_SECONDS),
        }
    }

    pub fn in_seconds(seconds: i64) -> Deadline {
        Deadline::from_base(Instant::now(), seconds)
    }

    pub fn is_stop_due_at(&self, now: Instant) -> bool {
        now >= self.stop_at
    }

    pub fn is_timed_out_at(&self, now: Instant) -> bool {
        now >= self.timed_out_at
    }
}

fn offset(base: Instant, seconds: i64) -> Instant {
    if seconds >= 0 {
        base.checked_add(Duration::from_secs(seconds as u64))
            .unwrap_or(base)
    } else {
        base.checked_sub(Duration::from_secs(seconds.unsigned_abs()))
            .unwrap_or(base)
    }
}

#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    cancel_all: Arc<AtomicBool>,
    cancel_auto_router: Arc<AtomicBool>,
    deadline: Option<Deadline>,
}

impl CancelToken {
    pub fn new() -> CancelToken {
        CancelToken::default()
    }

    pub fn with_timeout(total: Duration) -> CancelToken {
        CancelToken::with_deadline(Deadline::in_seconds(
            total.as_secs().min(i64::MAX as u64) as i64
        ))
    }

    pub fn with_deadline(deadline: Deadline) -> CancelToken {
        CancelToken {
            deadline: Some(deadline),
            ..CancelToken::default()
        }
    }

    #[must_use]
    pub fn with_deadline_from(&self, deadline: Deadline) -> CancelToken {
        CancelToken {
            deadline: Some(deadline),
            ..self.clone()
        }
    }

    pub fn deadline(&self) -> Option<Deadline> {
        self.deadline
    }

    pub fn cancel(&self) {
        self.cancel_all.store(true, Ordering::SeqCst);
    }

    pub fn cancel_auto_router(&self) {
        self.cancel_auto_router.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_all.load(Ordering::SeqCst)
    }

    pub fn is_auto_router_cancelled(&self) -> bool {
        self.cancel_auto_router.load(Ordering::SeqCst)
    }

    pub fn is_timed_out(&self) -> bool {
        self.deadline
            .is_some_and(|d| d.is_timed_out_at(Instant::now()))
    }

    pub fn apply_to(&self, stop: &RouterStop) {
        if self.is_cancelled() {
            stop.request_stop();
        }
        if self.is_auto_router_cancelled() {
            stop.request_stop_auto_router();
        }
    }

    pub fn as_router_stop(&self) -> RouterStop {
        let stop = match self.deadline {
            None => RouterStop::new(),
            Some(deadline) => {
                let now = Instant::now();
                let limit_ms = if deadline.stop_at >= now {
                    i64::try_from(deadline.stop_at.duration_since(now).as_millis())
                        .unwrap_or(i64::MAX)
                } else {
                    -1
                };
                RouterStop::with_deadline(limit_ms.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
            }
        };
        let token = self.clone();
        let stop = stop.with_cancel_poll(Box::new(move |stop| token.apply_to(stop)));
        self.apply_to(&stop);
        stop
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_router::pipeline::StopRequestState;

    #[test]
    fn a_fresh_token_is_a_fresh_router_stop() {
        let token = CancelToken::new();
        let stop = token.as_router_stop();
        assert_eq!(stop.state(), StopRequestState::None);
        assert!(!stop.is_timed_out());
        assert!(!stop.poll_deadline());
    }

    #[test]
    fn the_grace_period_lands_on_timed_out_at_and_never_on_stop_at() {
        let base = Instant::now();
        let deadline = Deadline::from_base(base, 60);
        assert_eq!(deadline.stop_at, base + Duration::from_secs(60));
        assert_eq!(deadline.timed_out_at, base + Duration::from_secs(90));
    }
}
