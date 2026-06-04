use std::path::PathBuf;
use anyhow::Result;
use ratatui::prelude::Frame;
use crate::git::get_commit_details;

// Type alias for commit data for clarity
pub type CommitData = Vec<(PathBuf, Vec<String>)>;

/// Extracts the short hash from a stored commit line.
///
/// The hash is the first token, terminated by `|` in the compact format
/// (`"h|date|subject"`) or by whitespace in the detailed format (`"h date\n…"`).
/// Splitting on whitespace alone is wrong because the date contains a space,
/// which previously produced hashes like `086e74e|2026-06-04`.
pub fn commit_hash(line: &str) -> &str {
    line.split(|c: char| c == '|' || c.is_whitespace()).next().unwrap_or("")
}

pub fn get_active_commits(commits: &CommitData, selected_repo_index: usize) -> Option<&Vec<String>> {
    if selected_repo_index == usize::MAX {
        None
    } else {
        // Bounds-checked lookup; usize::MAX means "All projects" (handled above).
        commits.get(selected_repo_index).map(|(_, repo_commits)| repo_commits)
    }
}

#[allow(dead_code)]
pub fn get_sidebar_height() -> Result<usize> {
    let (_cols, rows) = crossterm::terminal::size()?;
    Ok(rows.saturating_sub(2) as usize) // 2 lines for border
}

#[allow(dead_code)]
pub fn get_commitlist_height() -> Result<usize> {
    let (_cols, rows) = crossterm::terminal::size()?;
    Ok(rows.saturating_sub(2 + 1 + 15) as usize) // borders + footer + detail
}

#[allow(dead_code)]
pub fn get_commitlist_visible_and_total(commits: &CommitData, selected_repo_index: usize) -> (usize, usize) {
    if selected_repo_index == usize::MAX {
        let total: usize = commits.iter().map(|(_, c)| c.len()).sum();
        (0, total)
    } else {
        let total = commits.get(selected_repo_index).map(|(_, c)| c.len()).unwrap_or(0);
        (0, total)
    }
}

#[allow(dead_code)]
pub fn calculate_visible_height(f: &Frame, has_details: bool) -> u16 {
    const FOOTER_HEIGHT: u16 = 1;
    const DETAIL_HEIGHT: u16 = 15;
    let total_height = f.area().height;
    if has_details {
        total_height.saturating_sub(2 + FOOTER_HEIGHT + DETAIL_HEIGHT)
    } else {
        total_height.saturating_sub(2 + FOOTER_HEIGHT)
    }
}

/// Maximum scroll offset for the detail pane showing `commit_index`, given the
/// pane's inner (border-excluded) `view_height`, so End and clamping land on the
/// last line instead of scrolling into empty space. Shells out to git for the
/// commit body, so call it on demand (e.g. the End key), not every frame.
pub fn calculate_max_detail_scroll(
    commits: &CommitData,
    selected_repo_index: usize,
    commit_index: usize,
    view_height: u16,
) -> u16 {
    let detail_for = |repo: &PathBuf, commit: &str| {
        let hash = commit_hash(commit);
        if hash.is_empty() {
            return 0;
        }
        get_commit_details(repo, hash)
            .map(|details| calculate_max_scroll(&details, view_height))
            .unwrap_or(0)
    };
    if selected_repo_index == usize::MAX {
        let mut idx = 0;
        for (repo, repo_commits) in commits {
            if commit_index < idx + repo_commits.len() {
                return detail_for(repo, &repo_commits[commit_index - idx]);
            }
            idx += repo_commits.len();
        }
        0
    } else if let Some((repo, repo_commits)) = commits.get(selected_repo_index) {
        repo_commits.get(commit_index).map(|c| detail_for(repo, c)).unwrap_or(0)
    } else {
        0
    }
}

/// Maximum vertical scroll offset so the last content line still fits in a
/// `view_height`-tall viewport (0 when everything already fits).
pub fn calculate_max_scroll(content: &str, view_height: u16) -> u16 {
    let content_lines = content.lines().count() as u16;
    content_lines.saturating_sub(view_height)
}