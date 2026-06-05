use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use crate::history::OverviewRecord;

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

/// Which area of the overview view is focused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OverviewFocus {
    /// The left list of stored overviews.
    List,
    /// The right detail pane (scrollable text).
    Detail,
}

/// Metadata captured when a summary is dispatched, carried through the async
/// fetch so the finished result can be stored as an `OverviewRecord` and so a
/// regenerate (`r`) reproduces the same request context.
#[derive(Clone, Debug)]
pub struct OverviewMeta {
    pub project: String,
    pub interval: String,
    pub from: String,
    pub to: String,
    pub lang: String,
    pub provider: String,
    pub model: String,
    pub commit_count: usize,
    pub tab: String,
}

/// Shared state for the AI overview view. Replaces the former popup: holds the
/// persisted overviews plus the transient generation state. Lives behind an
/// `Arc<Mutex<…>>` so the async summary task can push results and the renderer
/// can read them.
#[derive(Debug)]
pub struct OverviewState {
    /// Stored overviews, newest first (mirrors `history::load_overviews`).
    pub items: Vec<OverviewRecord>,
    /// True while a summary is being generated (drives the spinner + redraw).
    pub generating: bool,
    /// Frame index for the loading spinner.
    pub spinner_frame: u8,
    /// True once the selected overview was copied to the clipboard.
    pub copied: bool,
    /// True while awaiting inline y/n confirmation for deleting the selected
    /// overview.
    pub pending_delete: bool,
    /// Transient text shown at the top while generating or on error (errors are
    /// not persisted to `items`).
    pub transient: Option<String>,
    /// The last dispatched request (prompt, lang, llm config, metadata), kept so
    /// `r` can regenerate without rebuilding it from the current selection.
    pub last_request: Option<(String, String, LlmConfig, OverviewMeta)>,
    /// Max overviews to retain (from `recent_generations`); older are pruned.
    pub cap: usize,
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