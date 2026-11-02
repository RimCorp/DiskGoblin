//! Shared cancellation state. A completed scan may only clear its own registration.
use diskgoblin_core::CancelToken;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};

#[derive(Debug, Default)]
pub struct ActiveScanState {
    active: Mutex<Option<CancelToken>>,
    generation: AtomicU64,
}

impl ActiveScanState {
    /// Replacing a scan cooperatively cancels its predecessor.
    pub fn register(&self, token: CancelToken) -> u64 {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(previous) = active.replace(token) {
            previous.cancel();
        }
        self.generation
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1)
    }

    pub fn cancel(&self) {
        let active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(token) = active.as_ref() {
            token.cancel();
        }
    }

    pub(crate) fn clear(&self, generation: u64) {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if self.generation.load(Ordering::Relaxed) == generation {
            *active = None;
        }
    }
}

/// Clears state on success, error, or worker panic.
pub(crate) struct ScanRegistration<'a>(pub &'a ActiveScanState, pub u64);

impl Drop for ScanRegistration<'_> {
    fn drop(&mut self) {
        self.0.clear(self.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_cancels_previous_and_stale_completion_preserves_current() {
        let state = ActiveScanState::default();
        let first = CancelToken::new();
        let first_id = state.register(first.clone());
        let second = CancelToken::new();
        let second_id = state.register(second.clone());
        assert!(first.is_cancelled());
        assert!(!second.is_cancelled());
        state.clear(first_id);
        state.cancel();
        assert!(second.is_cancelled());
        state.clear(second_id);
        assert!(state.active.lock().unwrap().is_none());
    }

    #[test]
    fn clearing_completion_leaves_no_active_token() {
        let state = ActiveScanState::default();
        state.cancel();
        let token = CancelToken::new();
        let generation = state.register(token.clone());
        drop(ScanRegistration(&state, generation));
        state.cancel();
        assert!(!token.is_cancelled());
        assert!(state.active.lock().unwrap().is_none());
    }
}
