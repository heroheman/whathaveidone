// src/main.rs
mod models;
mod git;
mod utils;
mod network;
mod ui;
mod input;
mod prompts;
mod config;

use std::{env, time::Duration};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use crossterm::{execute, terminal::{self, Clear as CrosstermClear, ClearType, enable_raw_mode, disable_raw_mode}, event::{self, Event, KeyCode, read}};
use ratatui::prelude::*;
use models::{FocusArea, PopupQuote};
use git::{find_git_repos, reload_commits};
use ui::render_commits;
use crate::input::{handle_key, handle_mouse};
use crate::models::SelectedCommits;
use std::collections::HashSet;
use utils::CommitData;
use crate::config::Settings;
use std::io::{self, Write};

#[derive(Copy, Clone, PartialEq, Eq)]
enum CommitTab {
    Timeframe,
    Selection,
    Stats,
}
impl CommitTab {
    fn as_index(self) -> usize {
        match self {
            CommitTab::Timeframe => 0,
            CommitTab::Selection => 1,
            CommitTab::Stats => 2,
        }
    }
    fn from_index(idx: usize) -> Self {
        match idx {
            1 => CommitTab::Selection,
            2 => CommitTab::Stats,
            _ => CommitTab::Timeframe,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let mut settings = Settings::new().expect("Failed to load settings");

    // Check for API key from config or environment variable
    let api_key_from_env = env::var("GEMINI_API_KEY").ok();
    let mut api_key = settings.gemini_api_key.clone().filter(|k| !k.is_empty()).or(api_key_from_env);

    // If no key is found, prompt the user
    if api_key.is_none() && settings.prompt_for_api_key {
        if unsafe { prompt_for_api_key()? } {
            // Re-load settings to get the new key
            settings = Settings::new().expect("Failed to reload settings after key entry");
            api_key = settings.gemini_api_key.clone();
        }
    }
    
    // If a key is available (from config or prompt), set it as an env var for gemini-rs to pick up
    if let Some(key) = &api_key {
        unsafe {
            env::set_var("GEMINI_API_KEY", key);
        }
    }

    let (initial_interval, lang, prompt_path, cli_gemini_model) = parse_args();
    let mut gemini_model = settings.gemini_model;
    if let Some(model) = cli_gemini_model {
        gemini_model = model;
    }

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
    let mut detailed_commit_view = false;
    let mut commits: CommitData = reload_commits(&repos, current_interval, filter_by_user, detailed_commit_view)?;

    let mut selected_repo_index = usize::MAX;
    let mut selected_commit_index: Option<usize> = None;
    let mut show_details = false;
    let mut focus = FocusArea::Sidebar;

    let mut sidebar_scroll = 0;
    let mut commitlist_scroll = 0;
    let mut detail_scroll = 0;

    let popup_quote = Arc::new(Mutex::new(PopupQuote { visible: false, text: String::new(), loading: false, scroll: 0, spinner_frame: 0 }));
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
                detailed_commit_view, // <-- pass new state
            );
        })?;

        if event::poll(poll_timeout)? {
            match event::read()? {
                Event::Key(key_event) => {
                    let handled = handle_key(
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
                        &mut selected_tab, // <-- pass as mutable reference
                        &lang,
                        prompt_path.as_deref(),
                        &gemini_model,
                        &mut detailed_commit_view, // <-- pass mutable reference
                    )?;
                    if !handled {
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
                            &lang,
                            prompt_path.as_deref(),
                            &gemini_model, // <-- add this argument
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

unsafe fn prompt_for_api_key() -> anyhow::Result<bool> {
    let mut stdout = io::stdout();
    let mut selection = 0; // 0 for Yes, 1 for Skip, 2 for Never

    loop {
        // Clear the line and print the prompt
        execute!(stdout, terminal::Clear(ClearType::All), crossterm::cursor::MoveTo(0,0))?;
        println!("No Gemini API key found.");
        
        let yes_style = if selection == 0 { "[Yes]" } else { " Yes " };
        let skip_style = if selection == 1 { "[Skip]" } else { " Skip " };
        let never_style = if selection == 2 { "[Never Ask Again]" } else { " Never Ask Again " };
        
        print!("Would you like to add one now? {} {} {}", yes_style, skip_style, never_style);
        stdout.flush()?;

        enable_raw_mode()?;
        let key_event = read()?;
        disable_raw_mode()?;

        if let Event::Key(key) = key_event {
            match key.code {
                KeyCode::Left => selection = (selection + 2) % 3,
                KeyCode::Right => selection = (selection + 1) % 3,
                KeyCode::Enter => break,
                KeyCode::Char('q') | KeyCode::Esc => return Ok(false),
                _ => {}
            }
        }
    }

    if selection == 0 { // Yes
        execute!(stdout, terminal::Clear(ClearType::All), crossterm::cursor::MoveTo(0,0))?;
        print!("Please enter your Gemini API key: ");
        stdout.flush()?;

        let mut key_input = String::new();
        io::stdin().read_line(&mut key_input)?;
        let key = key_input.trim();
        
        if key.is_empty() {
            return Ok(false);
        }

        config::save_api_key(key)?;
        println!("\nAPI key saved to ~/.config/whid/whid.toml. Starting application...");
        std::thread::sleep(Duration::from_secs(2));
        Ok(true)
    } else if selection == 1 { // Skip
        execute!(stdout, terminal::Clear(ClearType::All), crossterm::cursor::MoveTo(0,0))?;
        Ok(false)
    } else { // Never Ask Again
        config::disable_api_key_prompt()?;
        execute!(stdout, terminal::Clear(ClearType::All), crossterm::cursor::MoveTo(0,0))?;
        println!("Understood. The API key prompt has been disabled.");
        println!("You can add your key manually to your configuration file at:");
        println!("{}", config::get_user_config_path().display());
        println!("\nOr, set it as an environment variable: export GEMINI_API_KEY=your-key-here");
        println!("\nStarting application...");
        std::thread::sleep(Duration::from_secs(4));
        Ok(false)
    }
}

fn parse_args() -> (Duration, String, Option<String>, Option<String>) {
    let args: Vec<String> = env::args().collect();
    let mut hours = 24;
    let mut lang = "en".to_string();
    let mut prompt_path: Option<String> = None;
    let mut gemini_model: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "24" | "today" => hours = 24,
            "48" | "yesterday" => hours = 48,
            "72" => hours = 72,
            "week" => hours = 24 * 7,
            "month" => hours = 24 * 30,
            "--lang" => {
                if i + 1 < args.len() {
                    lang = args[i + 1].clone();
                    i += 1;
                }
            },
            "--prompt" => {
                if i + 1 < args.len() {
                    prompt_path = Some(args[i + 1].clone());
                    i += 1;
                }
            },
            "--model" => {
                if i + 1 < args.len() {
                    gemini_model = Some(args[i + 1].clone());
                    i += 1;
                }
            },
            _ => {}
        }
        i += 1;
    }
    (Duration::from_secs(hours * 3600), lang, prompt_path, gemini_model)
}
