// src/main.rs
use std::{env, fs, path::PathBuf, process::Command, time::{Duration, SystemTime}};
use chrono::{DateTime, Local};
use ratatui::{prelude::*, widgets::*};
use crossterm::{event::{self, Event, KeyCode}, execute, terminal::{self, Clear, ClearType}};

// Fokusbereiche
#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusArea {
    Sidebar,
    CommitList,
    Detail,
}

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
    let mut focus = FocusArea::Sidebar; // Initialer Fokus auf Sidebar

    // Scroll-Offsets für die drei Bereiche
    let mut sidebar_scroll: usize = 0;
    let mut commitlist_scroll: usize = 0;
    let mut detail_scroll: u16 = 0;

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
                focus,
                sidebar_scroll,
                commitlist_scroll,
                detail_scroll,
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
                        // Fokus zyklisch wechseln
                        focus = match focus {
                            FocusArea::Sidebar => {
                                // Wenn in CommitList noch nichts selektiert ist, selektiere ersten Commit
                                if selected_commit_index.is_none() {
                                    if selected_repo_index == usize::MAX {
                                        let total_commits: usize = commits.iter().map(|(_, c)| c.len()).sum();
                                        if total_commits > 0 {
                                            selected_commit_index = Some(0);
                                        }
                                    } else if let Some(repo_commits) = get_active_commits(&commits, selected_repo_index) {
                                        if !repo_commits.is_empty() {
                                            selected_commit_index = Some(0);
                                        }
                                    }
                                }
                                FocusArea::CommitList
                            }
                            FocusArea::CommitList => {
                                if show_details { FocusArea::Detail } else { FocusArea::Sidebar }
                            }
                            FocusArea::Detail => FocusArea::Sidebar,
                        };
                    }
                    KeyCode::Char(' ') => {
                        // Toggle Detailview nur im CommitList-Fokus
                        if focus == FocusArea::CommitList {
                            show_details = !show_details;
                            // Wenn Detail geschlossen wird und Fokus darauf war, zurück zu CommitList
                            if !show_details && focus == FocusArea::Detail {
                                focus = FocusArea::CommitList;
                            }
                        }
                    }
                    KeyCode::Up => {
                        match focus {
                            FocusArea::Sidebar => {
                                // Sidebar: Repos hoch navigieren
                                let filtered_repos: Vec<&PathBuf> = commits.iter().map(|(repo, _)| repo).collect();
                                let repo_count = filtered_repos.len();
                                if selected_repo_index == usize::MAX {
                                    if repo_count > 0 {
                                        selected_repo_index = repo_count - 1;
                                    }
                                } else if selected_repo_index > 0 {
                                    selected_repo_index -= 1;
                                } else {
                                    selected_repo_index = usize::MAX;
                                }
                                selected_commit_index = None;
                                // Scrollbar für Sidebar
                                if selected_repo_index == usize::MAX {
                                    sidebar_scroll = 0;
                                } else if selected_repo_index < sidebar_scroll {
                                    sidebar_scroll = selected_repo_index;
                                }
                            }
                            FocusArea::CommitList => {
                                // Commitlist: Commits hoch navigieren
                                if selected_repo_index == usize::MAX {
                                    // "Alle"-View: globaler Index
                                    if let Some(index) = selected_commit_index {
                                        if index > 0 {
                                            selected_commit_index = Some(index - 1);
                                        }
                                    } else {
                                        // Wenn noch nichts selektiert, auf ersten Commit gehen
                                        // Finde ersten Commit (global index 0, falls es Commits gibt)
                                        let total_commits: usize = commits.iter().map(|(_, c)| c.len()).sum();
                                        if total_commits > 0 {
                                            selected_commit_index = Some(0);
                                        }
                                    }
                                } else {
                                    // Einzel-Repo-View wie gehabt
                                    if let Some(index) = selected_commit_index {
                                        if index > 0 {
                                            selected_commit_index = Some(index - 1);
                                        }
                                    } else {
                                        selected_commit_index = Some(0);
                                    }
                                }
                                // Scrollbar für CommitList
                                let (_visible, _total) = get_commitlist_visible_and_total(&commits, selected_repo_index);
                                if let Some(idx) = selected_commit_index {
                                    if idx < commitlist_scroll {
                                        commitlist_scroll = idx;
                                    }
                                } else {
                                    commitlist_scroll = 0;
                                }
                            }
                            FocusArea::Detail => {
                                // Scroll im Detailview nach oben
                                if detail_scroll > 0 {
                                    detail_scroll -= 1;
                                }
                            }
                        }
                    }
                    KeyCode::Down => {
                        match focus {
                            FocusArea::Sidebar => {
                                // Sidebar: Repos runter navigieren
                                let filtered_repos: Vec<&PathBuf> = commits.iter().map(|(repo, _)| repo).collect();
                                let repo_count = filtered_repos.len();
                                if selected_repo_index == usize::MAX {
                                    selected_repo_index = 0;
                                } else if selected_repo_index < repo_count - 1 {
                                    selected_repo_index += 1;
                                } else {
                                    selected_repo_index = usize::MAX;
                                }
                                selected_commit_index = None;
                                // Scrollbar für Sidebar
                                let filtered_repos: Vec<&PathBuf> = commits.iter().map(|(repo, _)| repo).collect();
                                let repo_count = filtered_repos.len();
                                let sidebar_len = repo_count + 1;
                                if selected_repo_index == usize::MAX {
                                    sidebar_scroll = 0;
                                } else if selected_repo_index >= sidebar_scroll + get_sidebar_height()? {
                                    sidebar_scroll = selected_repo_index + 1 - get_sidebar_height()?;
                                }
                                if sidebar_scroll > sidebar_len.saturating_sub(get_sidebar_height()?) {
                                    sidebar_scroll = sidebar_len.saturating_sub(get_sidebar_height()?);
                                }
                            }
                            FocusArea::CommitList => {
                                if selected_repo_index == usize::MAX {
                                    // "Alle"-View: globaler Index
                                    let total_commits: usize = commits.iter().map(|(_, c)| c.len()).sum();
                                    if total_commits == 0 {
                                        selected_commit_index = None;
                                    } else if let Some(index) = selected_commit_index {
                                        if index < total_commits - 1 {
                                            selected_commit_index = Some(index + 1);
                                        }
                                    } else {
                                        selected_commit_index = Some(0);
                                    }
                                } else {
                                    // Einzel-Repo-View wie gehabt
                                    if let Some(repo_commits) = get_active_commits(&commits, selected_repo_index) {
                                        if let Some(index) = selected_commit_index {
                                            if index < repo_commits.len() - 1 {
                                                selected_commit_index = Some(index + 1);
                                            }
                                        } else {
                                            selected_commit_index = Some(0);
                                        }
                                    }
                                }
                                // Scrollbar für CommitList
                                let (_visible, _total) = get_commitlist_visible_and_total(&commits, selected_repo_index);
                                if let Some(idx) = selected_commit_index {
                                    let height = get_commitlist_height()?;
                                    if idx >= commitlist_scroll + height {
                                        commitlist_scroll = idx + 1 - height;
                                    }
                                    if commitlist_scroll > _total.saturating_sub(height) {
                                        commitlist_scroll = _total.saturating_sub(height);
                                    }
                                }
                            }
                            FocusArea::Detail => {
                                // Scroll im Detailview nach unten
                                detail_scroll = detail_scroll.saturating_add(1);
                            }
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

// Hilfsfunktionen für Scrollhöhe (abhängig von Terminalgröße)
fn get_sidebar_height() -> anyhow::Result<usize> {
    let (_cols, rows) = crossterm::terminal::size()?;
    Ok(rows.saturating_sub(2) as usize) // 2 Zeilen für Rahmen
}
fn get_commitlist_height() -> anyhow::Result<usize> {
    let (_cols, rows) = crossterm::terminal::size()?;
    // 2 Zeilen für Rahmen, 1 für Footer, ggf. 15 für Details
    Ok(rows.saturating_sub(2 + 1 + 15) as usize)
}

// Liefert (sichtbare Höhe, Gesamtzahl) für Commitlist
fn get_commitlist_visible_and_total(commits: &Vec<(PathBuf, Vec<String>)>, selected_repo_index: usize) -> (usize, usize) {
    if selected_repo_index == usize::MAX {
        let total: usize = commits.iter().map(|(_, c)| c.len()).sum();
        (0, total)
    } else {
        let total = commits.get(selected_repo_index).map(|(_, c)| c.len()).unwrap_or(0);
        (0, total)
    }
}

fn render_commits(
    f: &mut Frame,
    _repos: &Vec<PathBuf>, // Prefix unused parameter
    selected_repo_index: usize,
    data: &Vec<(PathBuf, Vec<String>)>,
    interval_label: &str,
    selected_commit_index: Option<usize>,
    show_details: bool,
    focus: FocusArea, // <-- Fokus-Parameter
    _sidebar_scroll: usize,      // unused, prefix with _
    _commitlist_scroll: usize,   // unused, prefix with _
    detail_scroll: u16,
) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(1)].as_ref())
        .split(area);

    // Sidebar für Repos
    let filtered_repos: Vec<&PathBuf> = data.iter().map(|(repo, _)| repo).collect();
    let mut repo_list: Vec<ListItem> = vec![ListItem::new(format!(
        "{} Alle",
        if selected_repo_index == usize::MAX { "→" } else { " " }
    ))
    .style(if selected_repo_index == usize::MAX {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    })];
    repo_list.extend(
        filtered_repos
            .iter()
            .enumerate()
            .map(|(i, repo)| {
                let repo_name = repo.file_name().unwrap_or_default().to_string_lossy();
                let style = if selected_repo_index == i {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(format!(
                    "{} {}",
                    if selected_repo_index == i { "→" } else { " " },
                    repo_name
                ))
                .style(style)
            }),
    );
    let sidebar_block = Block::default()
        .title("Repositories")
        .borders(Borders::ALL)
        .style(
            if focus == FocusArea::Sidebar {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            }
        );
    let sidebar = List::new(repo_list)
        .block(sidebar_block);
    f.render_widget(sidebar, chunks[0]);

    // Main area für Commits
    let main_area = chunks[1];
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            if show_details && selected_commit_index.is_some() {
                vec![Constraint::Min(0), Constraint::Length(15), Constraint::Length(1)]
            } else {
                vec![Constraint::Min(0), Constraint::Length(1)]
            },
        )
        .split(main_area);

    let header_text = if selected_repo_index == usize::MAX {
        format!("Standup Commits – Zeitfenster: {}", interval_label)
    } else if let Some((repo, _)) = data.iter().find(|(r, _)| *r == *filtered_repos[selected_repo_index]) {
        format!("{} – Zeitfenster: {}", repo.file_name().unwrap_or_default().to_string_lossy(), interval_label)
    } else {
        format!("Standup Commits – Zeitfenster: {}", interval_label)
    };

    if selected_repo_index == usize::MAX {
        // Alle Commits mit Projektnamen als Überschrift
        let mut items: Vec<ListItem> = vec![];
        let mut current_commit_offset = 0;
        for (_repo_idx, (repo, commits)) in data.iter().enumerate() {
            let repo_name = repo.file_name().unwrap_or_default().to_string_lossy();
            items.push(ListItem::new(format!("### {}", repo_name)).style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)));
            items.extend(commits.iter().enumerate().map(|(commit_idx_in_repo, commit)| {
                let global_commit_idx = current_commit_offset + commit_idx_in_repo;
                let style = if Some(global_commit_idx) == selected_commit_index {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                 ListItem::new(format!(
                    "{} {}",
                    if Some(global_commit_idx) == selected_commit_index { "→" } else { " " },
                    commit
                )).style(style)
            }));
            current_commit_offset += commits.len();
        }

        let commitlist_block = Block::default()
            .title(header_text)
            .borders(Borders::ALL)
            .style(
                if focus == FocusArea::CommitList {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Cyan)
                }
            );
        let list = List::new(items)
            .block(commitlist_block);
        f.render_widget(list, vertical_chunks[0]);

        // Navigierbarkeit für "Alle"-View: Detailview für selektierten Commit anzeigen
        if show_details {
            if let Some(global_index) = selected_commit_index {
                // Finde das Repo und den Commit anhand des globalen Index
                let mut idx = 0;
                for (repo, commits) in data {
                    if global_index < idx + commits.len() {
                        let commit = &commits[global_index - idx];
                        let commit_hash = commit.split_whitespace().next().unwrap_or("");
                        let details = if !commit_hash.is_empty() {
                            match get_commit_details(repo, commit_hash) {
                                Ok(d) => d,
                                Err(e) => format!("Error fetching details: {}", e),
                            }
                        } else {
                            "Could not extract commit hash.".to_string()
                        };

                        let detail_block = Block::default()
                            .title("Details")
                            .borders(Borders::ALL)
                            .style(
                                if focus == FocusArea::Detail {
                                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default().fg(Color::Magenta)
                                }
                            );
                        let details_widget = Paragraph::new(details)
                            .block(detail_block)
                            .wrap(Wrap { trim: true })
                            .scroll((detail_scroll, 0))
                            .style(Style::default().fg(Color::White));
                        f.render_widget(details_widget, vertical_chunks[1]);
                        break;
                    }
                    idx += commits.len();
                }
            }
        }
    } else {
        // Commits für ausgewähltes Repo
        if let Some((repo, commits)) = data.iter().find(|(r, _)| *r == *filtered_repos[selected_repo_index]) {
            let commit_list: Vec<ListItem> = commits
                .iter()
                .enumerate()
                .map(|(i, commit)| {
                    let style = if Some(i) == selected_commit_index {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    ListItem::new(format!(
                        "{} {}",
                        if Some(i) == selected_commit_index { "→" } else { " " },
                        commit
                    ))
                    .style(style)
                })
                .collect();
            let commitlist_block = Block::default()
                .title(header_text)
                .borders(Borders::ALL)
                .style(
                    if focus == FocusArea::CommitList {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Cyan)
                    }
                );
            let commit_widget = List::new(commit_list)
                .block(commitlist_block);
            f.render_widget(commit_widget, vertical_chunks[0]);

            // Detailview
            if show_details {
                if let Some(index) = selected_commit_index {
                    if let Some(commit) = commits.get(index) {
                        let commit_hash = commit.split_whitespace().next().unwrap_or("");
                        let details = if !commit_hash.is_empty() {
                            match get_commit_details(repo, commit_hash) {
                                Ok(d) => d,
                                Err(e) => format!("Error fetching details: {}", e),
                            }
                        } else {
                            "Could not extract commit hash.".to_string()
                        };

                        let detail_block = Block::default()
                            .title("Details")
                            .borders(Borders::ALL)
                            .style(
                                if focus == FocusArea::Detail {
                                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default().fg(Color::Magenta)
                                }
                            );
                        let details_widget = Paragraph::new(details)
                            .block(detail_block)
                            .wrap(Wrap { trim: true })
                            .scroll((detail_scroll, 0))
                            .style(Style::default().fg(Color::White));
                        f.render_widget(details_widget, vertical_chunks[1]);
                    }
                }
            }
        }
    }

    // Footer für Keybindings
    let footer = Paragraph::new(
        "Tasten: ←/→ Zeitfenster | ↑/↓ Navigation/Scroll | Tab Fokus | Space Details | q Beenden",
    )
    .style(Style::default().fg(Color::Gray).add_modifier(Modifier::DIM));
    f.render_widget(footer, vertical_chunks.last().unwrap().clone());
}

// Add this new function to fetch commit details
fn get_commit_details(repo: &PathBuf, commit_hash: &str) -> anyhow::Result<String> {
    // Show meta info and file list, omit full diff
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("show")
        .arg("--pretty=fuller")   // all meta info
        .arg("--name-status")     // file list with status (M, A, D, etc.)
        // .arg("--no-patch")     // remove: conflicts with --name-status
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
