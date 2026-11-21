//! What every core operation needs: the platform, a cancel flag and an optional progress sink.
//! Cheap to clone, safe to share across threads.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::model::Progress;
use crate::platform::Platform;

#[derive(Clone)]
pub struct Ctx {
    pub platform: Arc<dyn Platform>,
    cancel: Arc<AtomicBool>,
    progress: Option<Arc<dyn Fn(Progress) + Send + Sync>>,
}

impl Ctx {
    pub fn new(platform: Arc<dyn Platform>) -> Self {
        Self { platform, cancel: Arc::new(AtomicBool::new(false)), progress: None }
    }

    pub fn with_progress(mut self, sink: impl Fn(Progress) + Send + Sync + 'static) -> Self {
        self.progress = Some(Arc::new(sink));
        self
    }

    /// Ask every operation sharing this context to stop. Operations return what they have so far, except
    /// those the contract says return nothing on cancel (duplicates).
    pub fn cancel(&self) { self.cancel.store(true, Ordering::Relaxed); }

    pub fn cancelled(&self) -> bool { self.cancel.load(Ordering::Relaxed) }

    pub fn report(&self, p: Progress) {
        if let Some(sink) = &self.progress { sink(p); }
    }

    pub fn now(&self) -> f64 { self.platform.now() }
}
