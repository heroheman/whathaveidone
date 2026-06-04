use std::{fs, path::{Path, PathBuf}, process::Command, time::{Duration, SystemTime}};
use chrono::{DateTime, Local};
use anyhow::Result;
use std::sync::OnceLock;

pub fn find_git_repos(start_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut repos = vec![];
    if start_dir.join(".git").exists() {
        repos.push(start_dir.to_path_buf());
        // Do not recurse into subdirs if the start_dir is a git repo itself (common convention)
        return Ok(repos);
    }
    // Skipping an unreadable directory should not abort the whole scan.
    let entries = match fs::read_dir(start_dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(repos),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Use the entry's file type to skip symlinks (avoids traversal cycles)
        // and ignore hidden directories such as caches and dot-folders.
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let is_hidden = entry
            .file_name()
            .to_str()
            .map(|n| n.starts_with('.'))
            .unwrap_or(false);
        if is_dir && !is_hidden {
            if path.join(".git").exists() {
                repos.push(path);
            } else if let Ok(mut sub) = find_git_repos(&path) {
                repos.append(&mut sub);
            }
        }
    }
    repos.sort();
    Ok(repos)
}

pub fn get_current_git_user() -> Result<String> {
    let output = Command::new("git")
        .arg("config")
        .arg("user.email")
        .output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(anyhow::anyhow!(
            "Failed to get git user email: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

pub fn get_recent_commits(
    repo: &PathBuf,
    interval: Duration,
    filter_by_user: bool,
    detailed: bool,
    from: Option<String>,
    to: Option<String>,
) -> Result<Vec<String>> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(repo)
        .arg("log");

    if let Some(from_date) = from {
        cmd.arg("--since").arg(from_date);
        if let Some(to_date) = to {
            cmd.arg("--until").arg(to_date);
        }
    } else {
        let since = SystemTime::now() - interval;
        let since_datetime: DateTime<Local> = since.into();
        let since_str = since_datetime.format("%Y-%m-%d %H:%M:%S").to_string();
        cmd.arg("--since").arg(&since_str);
    }

    cmd.arg("--date=format:%Y-%m-%d %H:%M");

    // Apply the "only mine" author filter in every view, detailed included.
    if filter_by_user {
        static USER_EMAIL: OnceLock<Option<String>> = OnceLock::new();
        let user = USER_EMAIL.get_or_init(|| get_current_git_user().ok());
        if let Some(user) = user {
            cmd.arg("--author").arg(user);
        }
    }

    if detailed {
        // Use a unique separator for robust splitting, and show date+time (hh:mm)
        cmd.arg("--format=%h %ad%n%B (%an)%n---GITBLOCK---");
    } else if filter_by_user {
        cmd.arg("--pretty=format:%h|%ad|%s");
    } else {
        cmd.arg("--pretty=format:%h|%ad|%an|%s");
    }

    let output = cmd.output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // For detailed view, split by the unique separator
    if detailed {
        Ok(stdout.split("---GITBLOCK---").map(|s| s.trim_matches(['\n', '\r', ' '].as_ref()).to_string()).filter(|s| !s.is_empty()).collect())
    } else {
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    }
}

pub fn get_commit_details(repo: &PathBuf, commit_hash: &str) -> Result<String> {
    let output = Command::new("git")
        .arg("-C").arg(repo)
        .arg("show")
        .arg("--pretty=fuller")
        .arg("--name-status")
        .arg(commit_hash)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(anyhow::anyhow!(
            "git show failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

pub type CommitData = Vec<(PathBuf, Vec<String>)>;

pub fn reload_commits(
    repos: &[PathBuf],
    duration: Duration,
    filter_by_user: bool,
    detailed: bool,
    from: Option<String>,
    to: Option<String>,
) -> Result<CommitData> {
    let mut commits = vec![];
    for repo in repos {
        let repo_commits =
            get_recent_commits(repo, duration, filter_by_user, detailed, from.clone(), to.clone())?;
        if !repo_commits.is_empty() {
            commits.push((repo.clone(), repo_commits));
        }
    }
    Ok(commits)
}