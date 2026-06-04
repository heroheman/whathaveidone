use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

/// Poison-tolerant locking for shared UI state.
///
/// A panic in one of the async summary tasks must not take down the whole TUI
/// on the next `lock()`. Recovering the inner guard is safe here because the
/// protected state is plain UI data with no cross-field invariants to uphold.
pub trait LockExt<T> {
    fn lock_safe(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn lock_safe(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Which UI area is currently focused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FocusArea {
    Sidebar,
    CommitList,
    Detail,
}

/// State for the AI quote popup.
#[derive(Debug)]
pub struct PopupQuote {
    pub visible: bool,
    pub text: String,
    pub loading: bool,
    pub scroll: u16, // scroll offset for popup summary
    pub spinner_frame: u8, // frame index for loading spinner
    pub copied: bool, // true once the current summary was copied to the clipboard
    /// The last dispatched request (prompt, lang, model), kept so `r` can
    /// regenerate the summary without rebuilding it from scratch.
    pub last_request: Option<(String, String, String)>,
}

/// State for selected/marked commits.
///
/// Keyed by commit hash and storing the owning repo plus the full commit line
/// captured at mark time, so a marked commit survives timeframe changes even
/// when it is no longer part of the currently loaded commit set. Ordered by
/// hash for deterministic display.
#[derive(Debug)]
pub struct SelectedCommits {
    pub set: BTreeMap<String, (PathBuf, String)>,
}