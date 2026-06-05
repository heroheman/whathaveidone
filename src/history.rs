// Persistence for generated AI overviews.
//
// Overviews are stored as a JSON array at `~/.config/whid/overviews.json`,
// next to the user config (`config::get_user_config_path`). Timestamps are
// kept as a preformatted display string plus Unix seconds so we never need a
// chrono `serde` feature. The newest entry lives at index 0.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// One stored AI overview together with the metadata of the request that
/// produced it. Shown in the overview view's list (summary fields) and detail
/// pane (full `text` + metadata header).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OverviewRecord {
    /// Human-readable creation time, e.g. "2026-06-04 13:42".
    pub created_at: String,
    /// Creation time in Unix seconds, for stable ordering.
    pub created_unix: i64,
    /// The generated summary text.
    pub text: String,
    /// "All projects" / a repo name / "Selection".
    pub project: String,
    /// Interval label, e.g. "24h" or "2026-06-01 to 2026-06-04".
    pub interval: String,
    pub from: String,
    pub to: String,
    pub lang: String,
    /// "gemini" or "openai".
    pub provider: String,
    pub model: String,
    pub commit_count: usize,
    /// Source tab: "Timeframe" or "Selection".
    pub tab: String,
}

/// Path to the overviews store, alongside the user config.
pub fn get_overviews_path() -> PathBuf {
    let mut path = dirs::home_dir().expect("Failed to get home directory");
    path.push(".config");
    path.push("whid");
    path.push("overviews.json");
    path
}

/// Loads stored overviews, newest first. Returns an empty list if the file is
/// missing or unreadable/invalid — a corrupt history must never block the TUI.
pub fn load_overviews() -> Vec<OverviewRecord> {
    let path = get_overviews_path();
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Append a freshly generated overview to the on-disk store, keeping only the
/// `cap` most recent (newest first) and pruning anything older. Used by the
/// non-interactive direct mode, which has no in-memory `OverviewState`.
pub fn push_overview(record: OverviewRecord, cap: usize) -> std::io::Result<()> {
    let mut items = load_overviews();
    items.insert(0, record);
    items.truncate(cap.max(1));
    save_overviews(&items)
}

/// Persists overviews as pretty JSON. Best-effort: errors are returned so the
/// caller can decide, but the app continues regardless.
pub fn save_overviews(items: &[OverviewRecord]) -> std::io::Result<()> {
    let path = get_overviews_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(items)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    fs::write(&path, json)
}
