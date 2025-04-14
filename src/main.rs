// src/main.rs
use std::{env, fs, path::PathBuf, process::Command, time::{Duration, SystemTime}};
use chrono::{DateTime, Local, NaiveDateTime};
use ratatui::{prelude::*, widgets::*};
use crossterm::{event, terminal};

fn main() -> anyhow::Result<()> {
    let interval = parse_args();
    let repos = find_git_repos(".")?;
    let mut commits = vec![];

    for repo in repos {
        let repo_commits = get_recent_commits(&repo, interval)?;
        if !repo_commits.is_empty() {
            commits.push((repo, repo_commits));
        }
    }

    display_in_tui(commits)?;
    Ok(())
}

fn parse_args() -> Duration {
    let args: Vec<String> = env::args().collect();
    let hours = if args.len() > 1 {
        match args[1].as_str() {
            "24" => 24,
            "48" => 48,
            "72" => 72,
            "week" => 24 * 7,
            "month" => 24 * 30,
            _ => 24,
        }
    } else {
        24
    };
    Duration::from_secs((hours * 3600) as u64)
}

fn find_git_repos(start_dir: &str) -> anyhow::Result<Vec<PathBuf>> {
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

fn get_recent_commits(repo: &PathBuf, interval: Duration) -> anyhow::Result<Vec<String>> {
    let since = SystemTime::now() - interval;
    let since_datetime: DateTime<Local> = since.into();
    let since_str = since_datetime.format("%Y-%m-%d %H:%M:%S").to_string();

    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("log")
        .arg("--since")
        .arg(&since_str)
        .arg("--pretty=format:%h %an %ar %s")
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines = stdout.lines().map(|s| s.to_string()).collect();
    Ok(lines)
}

fn display_in_tui(data: Vec<(PathBuf, Vec<String>)>) -> anyhow::Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    terminal::enable_raw_mode()?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|f| {
        let size = f.size();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                data.iter().map(|_| Constraint::Min(3)).collect::<Vec<_>>()
            )
            .split(size);

        for (i, (repo, commits)) in data.iter().enumerate() {
            let text = commits.join("\n");
            let block = Block::default().title(repo.display().to_string()).borders(Borders::ALL);
            let paragraph = Paragraph::new(text).block(block);
            f.render_widget(paragraph, chunks[i]);
        }
    })?;

    // wait for key press
    loop {
        if event::poll(std::time::Duration::from_millis(500))? {
            if let event::Event::Key(_) = event::read()? {
                break;
            }
        }
    }

    terminal::disable_raw_mode()?;
    Ok(())
}

// fn main() {
//     println!("Hello, world!");
// }