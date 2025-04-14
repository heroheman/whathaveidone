// src/main.rs
use std::{env, fs, path::PathBuf, process::Command, time::{Duration, SystemTime}};
use chrono::{DateTime, Local};
use ratatui::{prelude::*, widgets::*};
use crossterm::{event::{self, Event, KeyCode}, terminal};

fn main() -> anyhow::Result<()> {
    let initial_interval = parse_args();
    let repos = find_git_repos(".")?;

    let intervals = vec![
        ("24h", Duration::from_secs(24 * 3600)),
        ("48h", Duration::from_secs(48 * 3600)),
        ("72h", Duration::from_secs(72 * 3600)),
        ("1 Woche", Duration::from_secs(7 * 24 * 3600)),
        ("1 Monat", Duration::from_secs(30 * 24 * 3600)),
    ];
    let mut current_index = intervals.iter().position(|(_, d)| *d == initial_interval).unwrap_or(0);
    let mut current_interval = intervals[current_index].1;
    let mut commits = reload_commits(&repos, current_interval)?;

    terminal::enable_raw_mode()?;
    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| {
            render_commits(f, &commits, intervals[current_index].0);
        })?;

        if event::poll(std::time::Duration::from_millis(200))? {
            if let Event::Key(key_event) = event::read()? {
                match key_event.code {
                    KeyCode::Char('1') => current_index = 0,
                    KeyCode::Char('2') => current_index = 1,
                    KeyCode::Char('3') => current_index = 2,
                    KeyCode::Char('w') => current_index = 3,
                    KeyCode::Char('m') => current_index = 4,
                    KeyCode::Left => {
                        if current_index > 0 {
                            current_index -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if current_index < intervals.len() - 1 {
                            current_index += 1;
                        }
                    }
                    KeyCode::Char('q') => break,
                    _ => {}
                }
                current_interval = intervals[current_index].1;
                commits = reload_commits(&repos, current_interval)?;
            }
        }
    }

    terminal::disable_raw_mode()?;
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

fn reload_commits(repos: &Vec<PathBuf>, duration: Duration) -> anyhow::Result<Vec<(PathBuf, Vec<String>)>> {
    let mut commits = vec![];
    for repo in repos {
        let repo_commits = get_recent_commits(repo, duration)?;
        if !repo_commits.is_empty() {
            commits.push((repo.clone(), repo_commits));
        }
    }
    Ok(commits)
}

fn render_commits(f: &mut Frame, data: &Vec<(PathBuf, Vec<String>)>, interval_label: &str) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            std::iter::once(Constraint::Length(1))
                .chain(data.iter().map(|_| Constraint::Min(3)))
                .collect::<Vec<_>>()
        )
        .split(area);

    let header = Paragraph::new(format!("Standup Commits – Zeitfenster: {} (←/→ oder 1/2/3/w/m, q=quit)", interval_label))
        .style(Style::default().add_modifier(Modifier::BOLD));
    f.render_widget(header, chunks[0]);

    for (i, (repo, commits)) in data.iter().enumerate() {
        let text = commits.join("\n");
        let block = Block::default().title(repo.display().to_string()).borders(Borders::ALL);
        let paragraph = Paragraph::new(text).block(block);
        f.render_widget(paragraph, chunks[i + 1]);
    }
}
