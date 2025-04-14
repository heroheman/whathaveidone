// src/main.rs
use std::{env, fs, path::PathBuf, process::Command, time::{Duration, SystemTime}};
use chrono::{DateTime, Local};
use ratatui::{prelude::*, widgets::*};
use crossterm::{event::{self, Event, KeyCode}, execute, terminal::{self, Clear, ClearType}};

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
    let mut selected_repo_index = usize::MAX; // Default to showing all repositories
    let mut selected_commit_index: Option<usize> = None; // Track the selected commit index
    let mut show_details = false; // Whether to show the detailed view

    terminal::enable_raw_mode()?;
    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Clear the terminal screen to avoid artifacts
    execute!(std::io::stdout(), Clear(ClearType::All))?;

    loop {
        terminal.draw(|f| {
            render_commits(
                f,
                &repos,
                selected_repo_index,
                &commits,
                intervals[current_index].0,
                selected_commit_index,
                show_details,
            );
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
                    KeyCode::Tab => {
                        let filtered_repos: Vec<&PathBuf> = commits.iter().map(|(repo, _)| repo).collect();
                        if selected_repo_index == usize::MAX {
                            selected_repo_index = 0;
                        } else {
                            selected_repo_index = (selected_repo_index + 1) % filtered_repos.len();
                            if selected_repo_index == 0 {
                                selected_repo_index = usize::MAX; // Return to showing all
                            }
                        }
                        selected_commit_index = None; // Reset commit selection
                    }
                    KeyCode::Up => {
                        if let Some(index) = selected_commit_index {
                            if index > 0 {
                                selected_commit_index = Some(index - 1);
                            }
                        } else {
                            selected_commit_index = Some(0);
                        }
                    }
                    KeyCode::Down => {
                        if let Some(index) = selected_commit_index {
                            if let Some(repo_commits) = get_active_commits(&commits, selected_repo_index) {
                                if index < repo_commits.len() - 1 {
                                    selected_commit_index = Some(index + 1);
                                }
                            }
                        } else {
                            selected_commit_index = Some(0);
                        }
                    }
                    KeyCode::Char(' ') => {
                        show_details = !show_details; // Toggle detailed view
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

fn get_active_commits<'a>(
    commits: &'a Vec<(PathBuf, Vec<String>)>,
    selected_repo_index: usize,
) -> Option<&'a Vec<String>> {
    if selected_repo_index == usize::MAX {
        None
    } else {
        commits
            .iter()
            .find(|(repo, _)| repo == &commits[selected_repo_index].0)
            .map(|(_, repo_commits)| repo_commits)
    }
}

fn render_commits(
    f: &mut Frame,
    repos: &Vec<PathBuf>,
    selected_repo_index: usize,
    data: &Vec<(PathBuf, Vec<String>)>,
    interval_label: &str,
    selected_commit_index: Option<usize>,
    show_details: bool,
) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(1)].as_ref())
        .split(area);

    // Sidebar for repositories
    let filtered_repos: Vec<&PathBuf> = data.iter().map(|(repo, _)| repo).collect();
    let mut repo_list: Vec<ListItem> = vec![ListItem::new(format!(
        "{} Alle",
        if selected_repo_index == usize::MAX { "→" } else { " " }
    ))
    .style(if selected_repo_index == usize::MAX {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    })];
    repo_list.extend(
        filtered_repos
            .iter()
            .enumerate()
            .map(|(i, repo)| {
                let repo_name = repo.file_name().unwrap_or_default().to_string_lossy();
                let style = if selected_repo_index == i {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(format!(
                    "{} {}",
                    if selected_repo_index == i { "→" } else { " " },
                    repo_name
                ))
                .style(style)
            }),
    );
    let sidebar = List::new(repo_list)
        .block(Block::default().title("Repositories").borders(Borders::ALL));
    f.render_widget(sidebar, chunks[0]);

    // Main area for commits
    let main_area = chunks[1];
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            if show_details {
                vec![Constraint::Min(3), Constraint::Length(7), Constraint::Length(1)]
            } else {
                vec![Constraint::Min(3), Constraint::Length(1)]
            },
        )
        .split(main_area);

    let header = Paragraph::new(format!(
        "Standup Commits – Zeitfenster: {}",
        interval_label
    ))
    .style(Style::default().add_modifier(Modifier::BOLD));
    f.render_widget(header, vertical_chunks[0]);

    if selected_repo_index == usize::MAX {
        // Show all commits with project names as headings
        let mut all_commits = vec![];
        for (repo, commits) in data {
            let repo_name = repo.file_name().unwrap_or_default().to_string_lossy();
            all_commits.push(format!("### {}\n", repo_name));
            all_commits.extend(commits.iter().cloned());
        }
        let text = all_commits.join("\n");
        let block = Block::default().borders(Borders::ALL);
        let paragraph = Paragraph::new(text).block(block);
        f.render_widget(paragraph, vertical_chunks[0]);
    } else {
        // Show commits for the selected repository
        if let Some((repo, commits)) = data.iter().find(|(r, _)| *r == *filtered_repos[selected_repo_index]) {
            let commit_list: Vec<ListItem> = commits
                .iter()
                .enumerate()
                .map(|(i, commit)| {
                    let style = if Some(i) == selected_commit_index {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(format!(
                        "{} {}",
                        if Some(i) == selected_commit_index { "→" } else { " " },
                        commit
                    ))
                    .style(style)
                })
                .collect();
            let commit_widget = List::new(commit_list)
                .block(Block::default().title(repo.display().to_string()).borders(Borders::ALL));
            f.render_widget(commit_widget, vertical_chunks[0]);

            // Show detailed view if enabled
            if show_details {
                if let Some(index) = selected_commit_index {
                    if let Some(commit) = commits.get(index) {
                        let details = format!("Details for commit:\n{}", commit);
                        let details_widget = Paragraph::new(details)
                            .block(Block::default().title("Details").borders(Borders::ALL));
                        f.render_widget(details_widget, vertical_chunks[1]);
                    }
                }
            }
        }
    }

    // Footer for keybindings
    let footer = Paragraph::new(
        "Tasten: ←/→ Zeitfenster | ↑/↓ Commits | Tab Projekte | Space Details | q Beenden",
    )
    .style(Style::default().add_modifier(Modifier::DIM));
    f.render_widget(footer, vertical_chunks.last().unwrap().clone());
}
