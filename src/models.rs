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

/// Which AI backend a summary request is sent to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LlmProvider {
    /// Google Gemini via the `gemini-rs` crate.
    Gemini,
    /// Any custom OpenAI-compatible chat-completions endpoint (OpenRouter,
    /// Vercel AI Gateway, a local server, OpenAI itself, …).
    Custom,
}

/// Everything `network` needs to fetch one summary. Resolved once in `main`
/// from config + CLI flags and threaded through the input layer so a single
/// request carries its provider, model, endpoint and key together.
#[derive(Clone, Debug)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub model: String,
    /// Base URL of the custom OpenAI-compatible endpoint (ignored for Gemini).
    pub base_url: String,
    /// API key. For Gemini this mirrors `GEMINI_API_KEY`; for a custom provider
    /// it is sent as the `Authorization: Bearer …` header.
    pub api_key: String,
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
    /// The last dispatched request (prompt, lang, llm config), kept so `r` can
    /// regenerate the summary without rebuilding it from scratch.
    pub last_request: Option<(String, String, LlmConfig)>,
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