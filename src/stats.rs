//! Pure aggregation of the already-loaded commit data into per-timeframe stats.
//!
//! No I/O and no git calls: everything is derived from the `CommitData` that the
//! commit browser already loaded for the active timeframe/filter, so the stats
//! view auto-reflects the window (and recomputes for free on every redraw).

use std::collections::BTreeMap;
use chrono::{NaiveDateTime, Timelike, Datelike};
use crate::utils::CommitData;

/// All statistics derived from the currently loaded commits.
pub struct CommitStats {
    pub total_commits: usize,
    pub active_days: usize,
    pub repo_count: usize,
    /// Chronologically ordered (YYYY-MM-DD sorts lexically).
    pub per_day: Vec<(String, u64)>,
    /// Descending by count, then name.
    pub per_repo: Vec<(String, u64)>,
    /// Index 0 = Monday .. 6 = Sunday.
    pub per_weekday: [u64; 7],
    /// Index = hour 0..23.
    pub per_hour: [u64; 24],
    /// Descending by count; empty when the author is not knowable from the
    /// stored format (compact + mine-only).
    pub per_author: Vec<(String, u64)>,
    pub busiest_day: Option<(String, u64)>,
    pub avg_per_active_day: f64,
}

impl CommitStats {
    pub fn is_empty(&self) -> bool {
        self.total_commits == 0
    }
}

/// Extracts `(datetime "YYYY-MM-DD HH:MM", author)` from one stored commit line,
/// mirroring the split logic in `ui::render_commit_line` so both stay in sync.
fn parse_line(line: &str, filter_by_user: bool, detailed: bool) -> (Option<&str>, Option<String>) {
    if detailed {
        // First line: "<hash> <YYYY-MM-DD> <HH:MM>" (hash split off the rest).
        let first = line.lines().next().unwrap_or(line);
        let datetime = first.split_once(' ').map(|(_, rest)| rest.trim());
        // Author is the trailing "(name)" line emitted by `%n(%an)`.
        let author = line
            .lines()
            .last()
            .map(str::trim)
            .filter(|l| l.starts_with('(') && l.ends_with(')'))
            .map(|l| l[1..l.len() - 1].trim().to_string());
        return (datetime, author);
    }

    if filter_by_user {
        // "hash|YYYY-MM-DD HH:MM|subject" — no author field.
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        (parts.get(1).map(|s| s.trim()), None)
    } else {
        // "hash|YYYY-MM-DD HH:MM|author|subject"
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        let author = parts.get(2).map(|s| s.trim().to_string());
        (parts.get(1).map(|s| s.trim()), author)
    }
}

/// Computes aggregates from already-loaded commit data.
///
/// `selected_repo_index == usize::MAX` (the "All projects" sentinel) aggregates
/// every repo; otherwise only that repo is considered.
pub fn compute_stats(
    data: &CommitData,
    filter_by_user: bool,
    detailed: bool,
    selected_repo_index: usize,
) -> CommitStats {
    let mut total_commits = 0usize;
    let mut per_day: BTreeMap<String, u64> = BTreeMap::new();
    let mut per_repo: Vec<(String, u64)> = Vec::new();
    let mut per_weekday = [0u64; 7];
    let mut per_hour = [0u64; 24];
    let mut per_author: BTreeMap<String, u64> = BTreeMap::new();

    for (idx, (repo, commits)) in data.iter().enumerate() {
        if selected_repo_index != usize::MAX && idx != selected_repo_index {
            continue;
        }
        let repo_name = repo.file_name().unwrap_or_default().to_string_lossy().to_string();
        per_repo.push((repo_name, commits.len() as u64));
        for line in commits {
            total_commits += 1;
            let (datetime, author) = parse_line(line, filter_by_user, detailed);
            if let Some(a) = author {
                if !a.is_empty() {
                    *per_author.entry(a).or_insert(0) += 1;
                }
            }
            if let Some(dt) = datetime {
                // Day key is robust to a malformed time portion.
                if dt.len() >= 10 {
                    *per_day.entry(dt[..10].to_string()).or_insert(0) += 1;
                }
                if let Ok(parsed) = NaiveDateTime::parse_from_str(dt, "%Y-%m-%d %H:%M") {
                    per_weekday[parsed.weekday().num_days_from_monday() as usize] += 1;
                    per_hour[parsed.hour() as usize] += 1;
                }
            }
        }
    }

    let per_day: Vec<(String, u64)> = per_day.into_iter().collect();
    let active_days = per_day.len();
    let busiest_day = per_day.iter().max_by_key(|(_, c)| *c).cloned();
    let avg_per_active_day = if active_days > 0 {
        total_commits as f64 / active_days as f64
    } else {
        0.0
    };

    // Sort distributions descending by count (stable on name for ties).
    per_repo.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut per_author: Vec<(String, u64)> = per_author.into_iter().collect();
    per_author.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let repo_count = per_repo.iter().filter(|(_, c)| *c > 0).count();

    CommitStats {
        total_commits,
        active_days,
        repo_count,
        per_day,
        per_repo,
        per_weekday,
        per_hour,
        per_author,
        busiest_day,
        avg_per_active_day,
    }
}
