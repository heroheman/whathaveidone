use std::path::PathBuf;
use anyhow::Result;
use ratatui::prelude::Frame;
use crate::git::get_commit_details;

// Type alias for commit data for clarity
pub type CommitData = Vec<(PathBuf, Vec<String>)>;

pub fn get_active_commits<'a>(commits: &'a CommitData, selected_repo_index: usize) -> Option<&'a Vec<String>> {
    if selected_repo_index == usize::MAX {
        None
    } else {
        commits
            .iter()
            .find(|(repo, _)| repo == &commits[selected_repo_index].0)
            .map(|(_, repo_commits)| repo_commits)
    }
}

pub fn get_sidebar_height() -> Result<usize> {
    let (_cols, rows) = crossterm::terminal::size()?;
    Ok(rows.saturating_sub(2) as usize) // 2 lines for border
}

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

pub fn calculate_max_detail_scroll(
    commits: &CommitData,
    selected_repo_index: usize,
    commit_index: usize,
) -> Result<u16> {
    if selected_repo_index == usize::MAX {
        let mut idx = 0;
        for (repo, repo_commits) in commits {
            if commit_index < idx + repo_commits.len() {
                let commit = &repo_commits[commit_index - idx];
                let commit_hash = commit.split_whitespace().next().unwrap_or("");
                if !commit_hash.is_empty() {
                    match get_commit_details(repo, commit_hash) {
                        Ok(details) => return calculate_max_scroll(details, 15),
                        Err(_) => return Ok(0),
                    }
                }
                return Ok(0);
            }
            idx += repo_commits.len();
        }
        return Ok(0);
    } else if let Some((repo, repo_commits)) = commits.get(selected_repo_index) {
        if let Some(commit) = repo_commits.get(commit_index) {
            let commit_hash = commit.split_whitespace().next().unwrap_or("");
            if !commit_hash.is_empty() {
                match get_commit_details(repo, commit_hash) {
                    Ok(details) => return calculate_max_scroll(details, 15),
                    Err(_) => return Ok(0),
                }
            }
        }
    }
    Ok(0)
}

pub fn calculate_max_scroll(content: String, view_height: u16) -> Result<u16> {
    let content_lines = content.lines().count() as u16;
    let visible_lines = view_height.saturating_sub(2);
    if content_lines <= visible_lines {
        return Ok(0);
    }
    Ok(content_lines.saturating_sub(visible_lines))
}