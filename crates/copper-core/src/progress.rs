use std::sync::{Arc, Mutex};

use copper_router::pipeline::{ProgressSink, RoutingEvent};

#[derive(Clone)]
pub struct SyncProgressSink {
    #[allow(clippy::type_complexity)]
    handler: Option<Arc<Mutex<dyn FnMut(&RoutingEvent) + Send>>>,
}

impl std::fmt::Debug for SyncProgressSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncProgressSink")
            .field(
                "handler",
                &if self.handler.is_some() {
                    "Some(<closure>)"
                } else {
                    "None"
                },
            )
            .finish()
    }
}

impl Default for SyncProgressSink {
    fn default() -> Self {
        SyncProgressSink::noop()
    }
}

impl SyncProgressSink {
    pub fn noop() -> SyncProgressSink {
        SyncProgressSink { handler: None }
    }

    pub fn new(f: impl FnMut(&RoutingEvent) + Send + 'static) -> SyncProgressSink {
        SyncProgressSink {
            handler: Some(Arc::new(Mutex::new(f))),
        }
    }

    pub fn is_noop(&self) -> bool {
        self.handler.is_none()
    }

    pub fn as_pipeline_sink(&self) -> SyncProgressSinkView<'_> {
        SyncProgressSinkView { sink: self }
    }

    pub fn emit(&self, event: &RoutingEvent) {
        let Some(handler) = self.handler.as_ref() else {
            return;
        };
        let mut guard = handler
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (guard)(event);
    }
}

#[derive(Debug)]
pub struct SyncProgressSinkView<'a> {
    sink: &'a SyncProgressSink,
}

impl ProgressSink for SyncProgressSinkView<'_> {
    fn on_event(&mut self, event: &RoutingEvent) {
        self.sink.emit(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copper_router::pipeline::{NamedAlgorithmType, TaskState};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn an_event() -> RoutingEvent {
        RoutingEvent::TaskStateChanged {
            algorithm: NamedAlgorithmType::Router,
            state: TaskState::Started,
        }
    }

    #[test]
    fn a_noop_sink_drops_everything_without_locking() {
        let sink = SyncProgressSink::noop();
        assert!(sink.is_noop());
        let mut view = sink.as_pipeline_sink();
        view.on_event(&an_event());
    }

    #[test]
    fn clones_share_one_closure() {
        let seen = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&seen);
        let sink = SyncProgressSink::new(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let clone = sink.clone();
        sink.as_pipeline_sink().on_event(&an_event());
        clone.as_pipeline_sink().on_event(&an_event());
        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }
}
