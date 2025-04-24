// src/main.rs
mod models;
mod git;
mod utils;
mod network;
mod ui;
mod input;
mod prompts;

use std::{env, time::Duration};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use crossterm::{execute, terminal::{self, Clear as CrosstermClear, ClearType}, event::{self, Event, KeyCode}};
use ratatui::prelude::*;
use models::{FocusArea, PopupQuote};
use git::{find_git_repos, reload_commits};
use ui::render_commits;
use crate::input::{handle_key, handle_mouse};
use crate::models::SelectedCommits;
use std::collections::HashSet;
use utils::CommitData;

#[derive(Copy, Clone, PartialEq, Eq)]
enum CommitTab {
    Timeframe,
    Selection,
}
impl CommitTab {
    fn as_index(self) -> usize {
        match self {
            CommitTab::Timeframe => 0,
            CommitTab::Selection => 1,
        }
    }
    fn from_index(idx: usize) -> Self {
        match idx {
            1 => CommitTab::Selection,
            _ => CommitTab::Timeframe,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let (initial_interval, lang) = parse_args();
    let repos = find_git_repos(".")?;

    let intervals = vec![
        ("24h", Duration::from_secs(24 * 3600)),
        ("48h", Duration::from_secs(48 * 3600)),
        ("72h", Duration::from_secs(72 * 3600)),
        ("1 week", Duration::from_secs(7 * 24 * 3600)),
        ("1 month", Duration::from_secs(30 * 24 * 3600)),
    ];
    let mut current_index = intervals.iter().position(|(_, d)| *d == initial_interval).unwrap_or(0);
    let mut current_interval = intervals[current_index].1;
    let mut filter_by_user = true;
    let mut commits: CommitData = reload_commits(&repos, current_interval, filter_by_user)?;

    let mut selected_repo_index = usize::MAX;
    let mut selected_commit_index: Option<usize> = None;
    let mut show_details = false;
    let mut focus = FocusArea::Sidebar;

    let mut sidebar_scroll = 0;
    let mut commitlist_scroll = 0;
    let mut detail_scroll = 0;

    let popup_quote = Arc::new(Mutex::new(PopupQuote { visible: false, text: String::new(), loading: false }));
    let selected_commits = Arc::new(Mutex::new(SelectedCommits { set: HashSet::new(), popup_visible: false }));

    let rt = Runtime::new()?;
    terminal::enable_raw_mode()?;
    crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
    execute!(std::io::stdout(), CrosstermClear(ClearType::All))?;
    let poll_timeout = std::time::Duration::from_millis(30);

    let mut last_sidebar_area = None;
    let mut selected_tab = CommitTab::Timeframe;
    loop {
        terminal.draw(|f| {
            // Compute layout to get sidebar_area
            let area = f.area();
            let vertical_chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([ratatui::layout::Constraint::Min(1), ratatui::layout::Constraint::Length(3)]).split(area);
            let columns = if show_details && selected_commit_index.is_some() {
                ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Horizontal)
                    .constraints([
                        ratatui::layout::Constraint::Length(30),
                        ratatui::layout::Constraint::Percentage(60),
                        ratatui::layout::Constraint::Percentage(40),
                    ])
                    .split(vertical_chunks[0])
            } else {
                ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Horizontal)
                    .constraints([
                        ratatui::layout::Constraint::Length(30),
                        ratatui::layout::Constraint::Min(1),
                    ])
                    .split(vertical_chunks[0])
            };
            let sidebar_area = columns[0];
            last_sidebar_area = Some(sidebar_area);
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
                filter_by_user,
                Some(&popup_quote),
                Some(&selected_commits),
                selected_tab,
            );
        })?;

        if event::poll(poll_timeout)? {
            match event::read()? {
                Event::Key(key_event) => {
                    match key_event.code {
                        KeyCode::Char('2') => selected_tab = CommitTab::Timeframe,
                        KeyCode::Char('3') | KeyCode::Char('s') => selected_tab = CommitTab::Selection,
                        _ => {}
                    }
                    if !handle_key(
                        key_event.code,
                        &intervals,
                        &mut current_index,
                        &mut current_interval,
                        &mut filter_by_user,
                        &repos,
                        &mut commits,
                        &mut selected_repo_index,
                        &mut selected_commit_index,
                        &mut show_details,
                        &mut focus,
                        &mut sidebar_scroll,
                        &mut commitlist_scroll,
                        &mut detail_scroll,
                        &popup_quote,
                        &selected_commits,
                        &rt,
                        selected_tab,
                    )? {
                        break;
                    }
                }
                Event::Mouse(mouse_event) => {
                    if let Some(sidebar_area) = last_sidebar_area {
                        handle_mouse(
                            mouse_event,
                            &repos,
                            &commits,
                            &mut selected_repo_index,
                            &mut selected_commit_index,
                            &mut focus,
                            &mut sidebar_scroll,
                            &mut commitlist_scroll,
                            &popup_quote,
                            &selected_commits,
                            sidebar_area,
                            &mut selected_tab,
                        );
                        // Mouse support for commit list tabs
                        // Calculate tab area (same as in ui.rs)
                        let area = terminal.get_frame().area();
                        let vertical_chunks = ratatui::layout::Layout::default()
                            .direction(ratatui::layout::Direction::Vertical)
                            .constraints([ratatui::layout::Constraint::Min(1), ratatui::layout::Constraint::Length(3)]).split(area);
                        let columns = if show_details && selected_commit_index.is_some() {
                            ratatui::layout::Layout::default()
                                .direction(ratatui::layout::Direction::Horizontal)
                                .constraints([
                                    ratatui::layout::Constraint::Length(30),
                                    ratatui::layout::Constraint::Percentage(60),
                                    ratatui::layout::Constraint::Percentage(40),
                                ])
                                .split(vertical_chunks[0])
                        } else {
                            ratatui::layout::Layout::default()
                                .direction(ratatui::layout::Direction::Horizontal)
                                .constraints([
                                    ratatui::layout::Constraint::Length(30),
                                    ratatui::layout::Constraint::Min(1),
                                ])
                                .split(vertical_chunks[0])
                        };
                        let commit_area = columns[1];
                        let tabs_area = ratatui::prelude::Rect {
                            x: commit_area.x,
                            y: commit_area.y,
                            width: commit_area.width,
                            height: 3,
                        };
                        use crossterm::event::MouseEventKind;
                        if let MouseEventKind::Down(_) = mouse_event.kind {
                            let x = mouse_event.column as u16;
                            let y = mouse_event.row as u16;
                            if y >= tabs_area.y && y < tabs_area.y + tabs_area.height {
                                // Calculate tab title widths with padding
                                let tab_titles = ["Timeframe", "Selection"];
                                let padding = 2; // 1 space left/right
                                let mut tab_x = tabs_area.x;
                                for (i, title) in tab_titles.iter().enumerate() {
                                    let tab_width = title.len() as u16 + padding * 2;
                                    if x >= tab_x && x < tab_x + tab_width {
                                        selected_tab = CommitTab::from_index(i);
                                        break;
                                    }
                                    tab_x += tab_width + 1; // +1 for divider
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    terminal::disable_raw_mode()?;
    crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture)?;
    Ok(())
}

fn parse_args() -> (Duration, String) {
    let args: Vec<String> = env::args().collect();
    let mut hours = 24;
    let mut lang = "en".to_string();
    for i in 1..args.len() {
        match args[i].as_str() {
            "24" => hours = 24,
            "48" => hours = 48,
            "72" => hours = 72,
            "week" => hours = 24 * 7,
            "month" => hours = 24 * 30,
            "--lang" => {
                if i + 1 < args.len() {
                    lang = args[i + 1].clone();
                }
            },
            _ => {}
        }
    }
    (Duration::from_secs((hours * 3600) as u64), lang)
}
