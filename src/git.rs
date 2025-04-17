use std::{fs, path::PathBuf, process::Command, time::{Duration, SystemTime}};
use chrono::{DateTime, Local};
use anyhow::Result;
use std::sync::OnceLock;

pub fn find_git_repos(start_dir: &str) -> Result<Vec<PathBuf>> {
    let mut repos = vec![];
    for entry in fs::read_dir(start_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if path.join(".git").exists() {
                repos.push(path);
            } else {
                let mut sub = find_git_repos(path.to_str().unwrap())?;
                repos.append(&mut sub);
            }
        }
    }
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

pub fn get_recent_commits(repo: &PathBuf, interval: Duration, filter_by_user: bool) -> Result<Vec<String>> {
    let since = SystemTime::now() - interval;
    let since_datetime: DateTime<Local> = since.into();
    let since_str = since_datetime.format("%Y-%m-%d %H:%M:%S").to_string();

    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo)
        .arg("log")
        .arg("--since").arg(&since_str);

    if filter_by_user {
        cmd.arg("--pretty=format:%h %ar %s");
        static USER_EMAIL: OnceLock<Option<String>> = OnceLock::new();
        let user = USER_EMAIL.get_or_init(|| get_current_git_user().ok());
        if let Some(user) = user {
            cmd.arg("--author").arg(user);
        }
    } else {
        cmd.arg("--pretty=format:%h %an %ar %s");
    }

    let output = cmd.output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().map(|s| s.to_string()).collect())
}

pub fn reload_commits(repos: &Vec<PathBuf>, duration: Duration, filter_by_user: bool) -> Result<Vec<(PathBuf, Vec<String>)>> {
    let mut commits = vec![];
    for repo in repos {
        let repo_commits = get_recent_commits(repo, duration, filter_by_user)?;
        if !repo_commits.is_empty() {
            commits.push((repo.clone(), repo_commits));
        }
    }
    Ok(commits)
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