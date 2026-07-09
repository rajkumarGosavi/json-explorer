use json_index::StructuralIndex;
use memmap2::Mmap;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Arc;

/// One open file. `mmap` and `index` are `Arc`-wrapped so a background
/// indexing/search thread can hold its own handle without borrowing the
/// session lock — dropping the last Arc releases the mapping and index
/// memory (see [[memory_safety_no_leaks]] in the plan).
pub struct Session {
    pub mmap: Arc<Mmap>,
    pub index: Arc<StructuralIndex>,
    /// Kept for future "current file" display / reopen; not read yet.
    #[allow(dead_code)]
    pub path: PathBuf,
    /// Bumped on every new search; a running search checks its own generation
    /// against this to know it's been superseded/cancelled and should stop.
    pub search_generation: Arc<AtomicU64>,
    pub search_cancel: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct AppState {
    pub session: RwLock<Option<Session>>,
}

impl AppState {
    /// Replace the current session (or clear it). Cancels any in-flight
    /// search on the outgoing session so its background thread observes the
    /// cancel flag and exits instead of racing the new session.
    pub fn replace_session(&self, new_session: Option<Session>) {
        let mut guard = self.session.write();
        if let Some(old) = guard.as_ref() {
            old.search_cancel
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        *guard = new_session;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    fn dummy_session() -> Session {
        Session {
            mmap: Arc::new(memmap2::MmapMut::map_anon(4).unwrap().make_read_only().unwrap()),
            index: Arc::new(StructuralIndex::default()),
            path: PathBuf::from("test.json"),
            search_generation: Arc::new(AtomicU64::new(0)),
            search_cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    #[test]
    fn no_session_by_default() {
        let state = AppState::default();
        assert!(state.session.read().is_none());
    }

    #[test]
    fn replace_session_cancels_outgoing_search_and_drops_old_mmap() {
        let state = AppState::default();
        let first = dummy_session();
        let first_cancel = first.search_cancel.clone();
        let first_mmap = Arc::downgrade(&first.mmap);
        state.replace_session(Some(first));
        assert!(!first_cancel.load(Ordering::SeqCst));

        // Reopening a new file (simulating open_file called again while a
        // search might be running on the old session) must flip the old
        // session's cancel flag so its background thread observes it and
        // exits — and must drop the old mmap once nothing else references it.
        let second = dummy_session();
        state.replace_session(Some(second));

        assert!(first_cancel.load(Ordering::SeqCst), "old session must be cancelled");
        assert!(
            first_mmap.upgrade().is_none(),
            "old mmap must be freed once the session is replaced and no thread holds an Arc to it"
        );
    }

    #[test]
    fn close_file_clears_session_and_cancels_search() {
        let state = AppState::default();
        let session = dummy_session();
        let cancel = session.search_cancel.clone();
        state.replace_session(Some(session));
        state.replace_session(None);
        assert!(cancel.load(Ordering::SeqCst));
        assert!(state.session.read().is_none());
    }
}
