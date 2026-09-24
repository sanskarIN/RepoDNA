//! Cooperative cancellation for long-running analysis.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A cheap, cloneable flag that long-running work checks between units of work.
///
/// Cancelling never interrupts a unit in the middle, so completed results (and cache
/// entries) stay consistent; the analysis stops at the next checkpoint and is marked partial.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Creates a token that is not cancelled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation. All clones observe the request.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Returns `true` once cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Returns `Err(Cancelled)` if cancellation has been requested.
    pub fn check(&self) -> Result<(), Cancelled> {
        if self.is_cancelled() {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Returned by [`CancellationToken::check`] when work should stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the analysis was cancelled")]
pub struct Cancelled;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_state() {
        let token = CancellationToken::new();
        let clone = token.clone();
        assert!(token.check().is_ok());
        clone.cancel();
        assert!(token.is_cancelled());
        assert_eq!(token.check(), Err(Cancelled));
    }
}
