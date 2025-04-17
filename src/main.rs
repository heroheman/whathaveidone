// src/main.rs
mod models;
mod git;
mod utils;
mod network;
mod ui;
mod input;

use std::{env, time::Duration};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use crossterm::{execute, terminal::{self, Clear as CrosstermClear, ClearType}, event::{self, Event}};
use ratatui::prelude::*;
use models::{FocusArea, PopupQuote};
use git::{find_git_repos, reload_commits};
use ui::render_commits;
use crate::input::handle_key;

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
    let mut filter_by_user = true;
    let mut commits = reload_commits(&repos, current_interval, filter_by_user)?;

    let mut selected_repo_index = usize::MAX;
    let mut selected_commit_index: Option<usize> = None;
    let mut show_details = false;
    let mut focus = FocusArea::Sidebar;

    let mut sidebar_scroll = 0;
    let mut commitlist_scroll = 0;
    let mut detail_scroll = 0;

    let popup_quote = Arc::new(Mutex::new(PopupQuote { visible: false, text: String::new(), loading: false }));

    let rt = Runtime::new()?;
    terminal::enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
    execute!(std::io::stdout(), CrosstermClear(ClearType::All))?;
    let poll_timeout = std::time::Duration::from_millis(30);

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
                filter_by_user,
                Some(&popup_quote),
            );
        })?;

        if event::poll(poll_timeout)? {
            if let Event::Key(key_event) = event::read()? {
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
                    &rt,
                )? {
                    break;
                }
            }
        }
    }

    terminal::disable_raw_mode()?;
    Ok(())
}

fn parse_args() -> Duration {
    let args: Vec<String> = env::args().collect();
    let hours = match args.get(1).map(|s| s.as_str()) {
        Some("24") => 24,
        Some("48") => 48,
        Some("72") => 72,
        Some("week") => 24 * 7,
        Some("month") => 24 * 30,
        _ => 24,
    };
    Duration::from_secs((hours * 3600) as u64)
}
