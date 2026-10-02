//! Cooperative cancellation (Ctrl-C). Long operations check the token between files or chunks and
//! stop with [`Cancelled`], so staged downloads are cleaned up instead of left half-installed.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    pub fn check(&self) -> Result<(), Cancelled> {
        if self.is_cancelled() { Err(Cancelled) } else { Ok(()) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("Cancelled.")]
pub struct Cancelled;

/// True if `error` is (or wraps) a cancellation.
pub fn is_cancelled(error: &anyhow::Error) -> bool {
    error.chain().any(|e| e.is::<Cancelled>())
}
