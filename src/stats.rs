//! Pure aggregation of the already-loaded commit data into per-timeframe stats.
//!
//! No I/O and no git calls: everything is derived from the `CommitData` that the
//! commit browser already loaded for the active timeframe/filter, so the stats
//! view auto-reflects the window (and recomputes for free on every redraw).

use std::collections::{BTreeMap, BTreeSet};
use chrono::{NaiveDateTime, Timelike, Datelike};
use once_cell::sync::Lazy;
use regex::Regex;
use crate::utils::CommitData;

/// Ticket references like `ABC-123` (same shape as the highlighter in `ui.rs`).
static TICKET_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"[A-Z]+-\d+").unwrap());

/// Recognized Conventional-Commit types; anything else with a `type:` shape is
/// bucketed under "other".
const CONVENTIONAL_TYPES: [&str; 11] = [
    "feat", "fix", "chore", "docs", "refactor", "test", "style", "perf", "build", "ci", "revert",
];

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
    /// Conventional-commit type → count, descending. Only commits whose subject
    /// matches the `type: …` shape are counted (unrecognized types → "other").
    pub per_type: Vec<(String, u64)>,
    /// Ticket reference (e.g. ABC-123) → number of commits, descending.
    pub top_tickets: Vec<(String, u64)>,
    /// Distinct tickets referenced and how many commits carry at least one.
    pub unique_tickets: usize,
    pub commits_with_ticket: usize,
    pub busiest_day: Option<(String, u64)>,
    pub avg_per_active_day: f64,
}

impl CommitStats {
    pub fn is_empty(&self) -> bool {
        self.total_commits == 0
    }
}

/// One parsed commit: the fields the stats need, regardless of stored format.
struct Parsed<'a> {
    datetime: Option<&'a str>,
    author: Option<String>,
    subject: Option<&'a str>,
}

/// Extracts datetime / author / subject from one stored commit line, mirroring
/// the split logic in `ui::render_commit_line` so both stay in sync.
fn parse_line(line: &str, filter_by_user: bool, detailed: bool) -> Parsed<'_> {
    if detailed {
        // "<hash> <YYYY-MM-DD> <HH:MM>\n<subject>\n<body…>\n(author)"
        let mut lines = line.lines();
        let first = lines.next().unwrap_or(line);
        let datetime = first.split_once(' ').map(|(_, rest)| rest.trim());
        let subject = lines.next().map(str::trim);
        // Author is the trailing "(name)" line emitted by `%n(%an)`.
        let author = line
            .lines()
            .last()
            .map(str::trim)
            .filter(|l| l.starts_with('(') && l.ends_with(')'))
            .map(|l| l[1..l.len() - 1].trim().to_string());
        return Parsed { datetime, author, subject };
    }

    if filter_by_user {
        // "hash|YYYY-MM-DD HH:MM|subject" — no author field.
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        Parsed { datetime: parts.get(1).map(|s| s.trim()), author: None, subject: parts.get(2).map(|s| s.trim()) }
    } else {
        // "hash|YYYY-MM-DD HH:MM|author|subject"
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        Parsed {
            datetime: parts.get(1).map(|s| s.trim()),
            author: parts.get(2).map(|s| s.trim().to_string()),
            subject: parts.get(3).map(|s| s.trim()),
        }
    }
}

/// Extracts the Conventional-Commit type from a subject (`feat(scope)!: …` →
/// `feat`). Returns `None` for subjects without the `type: …` shape so plain
/// messages don't pollute the distribution; unrecognized types map to "other".
fn commit_type(subject: &str) -> Option<String> {
    let (head, _) = subject.split_once(':')?;
    // The type is the token before an optional "(scope)" and "!".
    let t = head.split('(').next().unwrap_or(head).trim_end_matches('!').trim().to_lowercase();
    if t.is_empty() || t.contains(char::is_whitespace) {
        return None; // e.g. "Merge branch 'x': …"
    }
    if CONVENTIONAL_TYPES.contains(&t.as_str()) {
        Some(t)
    } else {
        Some("other".to_string())
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
    let mut per_type: BTreeMap<String, u64> = BTreeMap::new();
    let mut ticket_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut commits_with_ticket = 0usize;

    for (idx, (repo, commits)) in data.iter().enumerate() {
        if selected_repo_index != usize::MAX && idx != selected_repo_index {
            continue;
        }
        let repo_name = repo.file_name().unwrap_or_default().to_string_lossy().to_string();
        per_repo.push((repo_name, commits.len() as u64));
        for line in commits {
            total_commits += 1;
            let parsed = parse_line(line, filter_by_user, detailed);
            if let Some(a) = parsed.author
                && !a.is_empty()
            {
                *per_author.entry(a).or_insert(0) += 1;
            }
            if let Some(subject) = parsed.subject {
                if let Some(t) = commit_type(subject) {
                    *per_type.entry(t).or_insert(0) += 1;
                }
                // Count each ticket once per commit, even if mentioned twice.
                let tickets: BTreeSet<&str> = TICKET_REGEX.find_iter(subject).map(|m| m.as_str()).collect();
                if !tickets.is_empty() {
                    commits_with_ticket += 1;
                }
                for t in tickets {
                    *ticket_counts.entry(t.to_string()).or_insert(0) += 1;
                }
            }
            if let Some(dt) = parsed.datetime {
                // Day key is robust to a malformed time portion.
                if dt.len() >= 10 {
                    *per_day.entry(dt[..10].to_string()).or_insert(0) += 1;
                }
                if let Ok(parsed_dt) = NaiveDateTime::parse_from_str(dt, "%Y-%m-%d %H:%M") {
                    per_weekday[parsed_dt.weekday().num_days_from_monday() as usize] += 1;
                    per_hour[parsed_dt.hour() as usize] += 1;
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
    let sort_desc = |v: &mut Vec<(String, u64)>| {
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    };
    sort_desc(&mut per_repo);
    let mut per_author: Vec<(String, u64)> = per_author.into_iter().collect();
    sort_desc(&mut per_author);
    let mut per_type: Vec<(String, u64)> = per_type.into_iter().collect();
    sort_desc(&mut per_type);
    let unique_tickets = ticket_counts.len();
    let mut top_tickets: Vec<(String, u64)> = ticket_counts.into_iter().collect();
    sort_desc(&mut top_tickets);

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
        per_type,
        top_tickets,
        unique_tickets,
        commits_with_ticket,
        busiest_day,
        avg_per_active_day,
    }
}
