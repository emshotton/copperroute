use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use web_time::{Duration, Instant};

use copper_router::pipeline::RouterStop;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline {
    pub stop_at: Instant,
}

impl Deadline {
    pub fn from_base(base: Instant, seconds: i64) -> Deadline {
        Deadline {
            stop_at: offset(base, seconds),
        }
    }

    pub fn in_seconds(seconds: i64) -> Deadline {
        Deadline::from_base(Instant::now(), seconds)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStopReason {
    Deadline,
    Cancelled,
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
        let now = Instant::now();
        CancelToken::with_deadline(Deadline {
            stop_at: now.checked_add(total).unwrap_or(now),
        })
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
    use copper_router::pipeline::StopRequestState;

    #[test]
    fn search_poll_observes_new_cancellation() {
        let token = CancelToken::new();
        let stop = token.as_router_stop();
        token.cancel();
        assert!(stop.is_stopped_or_expired());
    }

    #[test]
    fn timeout_preserves_fractional_seconds() {
        let before = Instant::now();
        let token = CancelToken::with_timeout(Duration::from_millis(900));
        assert!(token.deadline().unwrap().stop_at >= before + Duration::from_millis(900));
    }

    #[test]
    fn a_fresh_token_is_a_fresh_router_stop() {
        let token = CancelToken::new();
        let stop = token.as_router_stop();
        assert_eq!(stop.state(), StopRequestState::None);
        assert!(!stop.is_timed_out());
        assert!(!stop.poll_deadline());
    }
}
