// src/main.rs
mod models;
mod git;
mod utils;
mod network;
mod ui;
mod input;
mod prompts;
mod config;
mod theme;
mod history;

use std::{env, time::Duration};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use crossterm::{execute, terminal::{self, Clear as CrosstermClear, ClearType, enable_raw_mode, disable_raw_mode}, event::{self, Event, KeyCode, read}, style::Stylize};
use ratatui::prelude::*;
use models::{FocusArea, OverviewState, OverviewFocus, LlmConfig, LlmProvider};
use git::{find_git_repos, reload_commits};
use ui::render_commits;
use crate::input::{handle_key, handle_mouse, handle_mouse_scroll, handle_overview_scroll};
use crate::models::SelectedCommits;
use crate::models::LockExt;
use std::collections::BTreeMap;
use utils::CommitData;
use crate::config::Settings;
use std::io::{self, Write};
use crate::theme::Theme;
use clap::Parser;

/// A terminal tool to summarize your Git commit history for daily standups, using AI.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Time frame to load commits from: today (24h), yesterday (48h), 72h, week, month
    #[arg(default_value = "today")]
    timeframe: String,

    /// The language for the AI summary
    #[arg(long)]
    lang: Option<String>,

    /// Path to a custom prompt template file
    #[arg(long)]
    prompt: Option<String>,

    /// The model to use for summaries (e.g., gemini-1.5-flash, or openai/gpt-4o-mini)
    #[arg(long)]
    model: Option<String>,

    /// AI provider: "gemini" (default) or "custom" (any OpenAI-compatible API)
    #[arg(long)]
    provider: Option<String>,

    /// Base URL of a custom OpenAI-compatible endpoint (e.g. https://openrouter.ai/api/v1)
    #[arg(long)]
    base_url: Option<String>,

    /// Start date for the commit history (YYYY-MM-DD)
    #[arg(long, value_name = "YYYY-MM-DD")]
    from: Option<String>,

    /// End date for the commit history (YYYY-MM-DD), defaults to today
    #[arg(long, value_name = "YYYY-MM-DD")]
    to: Option<String>,

    /// Show prompt-construction debug info in the AI summary popup
    #[arg(long)]
    debug: bool,
}

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

/// Top-level screen. The overview view (reachable with `0`) fully replaces the
/// commit browser layout with its own master/detail of stored AI overviews.
#[derive(Copy, Clone, PartialEq, Eq)]
enum AppView {
    Commits,
    Overview,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut settings = Settings::new().map_err(|e| anyhow::anyhow!("Failed to load settings: {e}"))?;

    // Resolve which AI backend to use (CLI flag overrides config; default gemini).
    let provider_str = cli.provider.clone()
        .or_else(|| settings.provider.clone())
        .unwrap_or_else(|| "gemini".to_string());
    let provider = match provider_str.trim().to_lowercase().as_str() {
        "custom" => LlmProvider::Custom,
        _ => LlmProvider::Gemini,
    };

    // Check for API key from config or environment variable
    let api_key_from_env = env::var("GEMINI_API_KEY").ok();
    let mut api_key = settings.gemini_api_key.clone().filter(|k| !k.is_empty()).or(api_key_from_env);

    // If no Gemini key is found, prompt the user — but only when Gemini is the
    // active backend; OpenAI-compatible providers use their own key below.
    if provider == LlmProvider::Gemini
        && api_key.is_none()
        && settings.prompt_for_api_key
        && unsafe { prompt_for_api_key()? }
    {
        // Re-load settings to get the new key
        settings = Settings::new().map_err(|e| anyhow::anyhow!("Failed to reload settings after key entry: {e}"))?;
        api_key = settings.gemini_api_key.clone();
    }

    // If a key is available (from config or prompt), set it as an env var for gemini-rs to pick up
    if let Some(key) = &api_key {
        unsafe {
            env::set_var("GEMINI_API_KEY", key);
        }
    }

    let theme = Theme::default();
    let hours = match cli.timeframe.as_str() {
        "24" | "today" => 24,
        "48" | "yesterday" => 48,
        "72" => 72,
        "week" => 24 * 7,
        "month" => 24 * 30,
        _ => cli.timeframe.parse::<u64>().unwrap_or(24),
    };
    let initial_interval = Duration::from_secs(hours * 3600);
    let lang = cli.lang.or(settings.lang).unwrap_or_else(|| "en".to_string());
    let prompt_path = cli.prompt.or(settings.custom_prompt_path);
    let cli_model = cli.model;
    let from_date = cli.from;
    let to_date = cli.to;
    let debug = cli.debug;

    // Resolve the model for the active provider (CLI --model overrides config).
    let model = cli_model.unwrap_or_else(|| match provider {
        LlmProvider::Gemini => settings.gemini_model.clone(),
        LlmProvider::Custom => settings.custom_model.clone().unwrap_or_default(),
    });

    // Custom OpenAI-compatible endpoint + key (key falls back to CUSTOM_API_KEY).
    let custom_base_url = cli.base_url
        .or_else(|| settings.custom_base_url.clone())
        .unwrap_or_default();
    let custom_api_key = settings.custom_api_key.clone()
        .filter(|k| !k.is_empty())
        .or_else(|| env::var("CUSTOM_API_KEY").ok())
        .unwrap_or_default();

    let llm = LlmConfig {
        provider,
        model,
        base_url: custom_base_url,
        api_key: match provider {
            LlmProvider::Gemini => api_key.clone().unwrap_or_default(),
            LlmProvider::Custom => custom_api_key,
        },
    };

    let repos = find_git_repos(std::path::Path::new("."))?;

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
    let mut commits: CommitData = reload_commits(&repos, current_interval, filter_by_user, detailed_commit_view, from_date.clone(), to_date.clone())?;

    let mut selected_repo_index = usize::MAX;
    let mut selected_commit_index: Option<usize> = None;
    let mut show_details = false;
    let mut focus = FocusArea::Sidebar;

    let mut sidebar_scroll = 0;
    let mut commitlist_scroll = 0;
    let mut detail_scroll = 0;

    let overview_state = Arc::new(Mutex::new(OverviewState {
        items: history::load_overviews(),
        generating: false,
        spinner_frame: 0,
        copied: false,
        pending_delete: false,
        transient: None,
        last_request: None,
    }));
    let selected_commits = Arc::new(Mutex::new(SelectedCommits { set: BTreeMap::new() }));

    // Top-level view + overview-view navigation state (threaded like the rest).
    let mut app_view = AppView::Commits;
    let mut overview_selected: usize = 0;
    let mut overview_focus = OverviewFocus::List;
    let mut overview_detail_scroll: u16 = 0;
    let mut show_help = false;

    let rt = Runtime::new()?;
    terminal::enable_raw_mode()?;
    crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
    execute!(std::io::stdout(), CrosstermClear(ClearType::All))?;
    let poll_timeout = std::time::Duration::from_millis(30);

    let mut last_sidebar_area = None;
    let mut selected_tab = CommitTab::Timeframe;
    // Redraw only when something changed or a summary is loading (spinner), so
    // the loop idles in event::poll instead of repainting ~33×/second.
    let mut needs_redraw = true;
    let mut prev_loading = false;
    loop {
        if needs_redraw {
        terminal.draw(|f| {
            // Remember the sidebar rect for mouse hit-testing (shared layout).
            last_sidebar_area = Some(
                ui::compute_layout(f.area(), show_details, selected_commit_index.is_some()).sidebar,
            );
            render_commits(
                f,
                &theme,
                &repos,
                selected_repo_index,
                &commits,
                intervals[current_index].0,
                &from_date,
                &to_date,
                selected_commit_index,
                show_details,
                focus,
                sidebar_scroll,
                commitlist_scroll,
                detail_scroll,
                filter_by_user,
                &overview_state,
                Some(&selected_commits),
                selected_tab,
                detailed_commit_view,
                app_view == AppView::Overview,
                overview_selected,
                overview_focus,
                overview_detail_scroll,
                show_help,
            );
        })?;
            needs_redraw = false;
        }

        if event::poll(poll_timeout)? {
            needs_redraw = true;
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
                        &overview_state,
                        &selected_commits,
                        &rt,
                        &mut selected_tab,
                        &lang,
                        prompt_path.as_deref(),
                        &llm,
                        &mut detailed_commit_view,
                        from_date.clone(),
                        to_date.clone(),
                        debug,
                        &mut app_view,
                        &mut overview_selected,
                        &mut overview_focus,
                        &mut overview_detail_scroll,
                        &mut show_help,
                    )?;
                    if !handled {
                        break;
                    }
                }
                Event::Mouse(mouse_event) => {
                    use crossterm::event::MouseEventKind;
                    let area = terminal.get_frame().area();
                    match mouse_event.kind {
                        // Wheel works in both views; the hovered region decides what scrolls.
                        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                            let down = matches!(mouse_event.kind, MouseEventKind::ScrollDown);
                            match app_view {
                                AppView::Commits => {
                                    let layout = ui::compute_layout(area, show_details, selected_commit_index.is_some());
                                    handle_mouse_scroll(
                                        mouse_event.column,
                                        mouse_event.row,
                                        down,
                                        &layout,
                                        &commits,
                                        &mut selected_repo_index,
                                        &mut selected_commit_index,
                                        &mut commitlist_scroll,
                                        &mut sidebar_scroll,
                                        &mut detail_scroll,
                                    );
                                }
                                AppView::Overview => {
                                    handle_overview_scroll(
                                        mouse_event.column,
                                        area,
                                        down,
                                        &overview_state,
                                        &mut overview_selected,
                                        &mut overview_detail_scroll,
                                    );
                                }
                            }
                        }
                        // Clicks only drive the commit browser.
                        MouseEventKind::Down(_) => {
                            if app_view == AppView::Commits {
                                if let Some(sidebar_area) = last_sidebar_area {
                                    handle_mouse(
                                        mouse_event,
                                        &commits,
                                        &mut selected_repo_index,
                                        &mut selected_commit_index,
                                        &mut focus,
                                        sidebar_area,
                                        &mut selected_tab,
                                    );
                                    // Mouse support for commit list tabs — same shared layout.
                                    let tabs_area = ui::compute_layout(area, show_details, selected_commit_index.is_some()).tabs;
                                    let x = mouse_event.column;
                                    let y = mouse_event.row;
                                    if y >= tabs_area.y && y < tabs_area.y + tabs_area.height {
                                        // Calculate tab title widths with padding
                                        let tab_titles = ["Timeframe", "Selection"];
                                        let padding = 2; // 1 space left/right
                                        let mut tab_x = tabs_area.x;
                                        for (i, title) in tab_titles.iter().enumerate() {
                                            let tab_width = title.len() as u16 + padding * 2;
                                            if x >= tab_x && x < tab_x + tab_width {
                                                let new_tab = CommitTab::from_index(i);
                                                if new_tab != selected_tab {
                                                    selected_commit_index = None;
                                                }
                                                selected_tab = new_tab;
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
                _ => {}
            }
        }

        // Keep repainting while a summary is loading (spinner) and once more
        // when it finishes, so the final result is shown without another event.
        let is_loading = overview_state.lock_safe().generating;
        if is_loading || is_loading != prev_loading {
            needs_redraw = true;
        }
        prev_loading = is_loading;
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
        
        println!("{}", "No Gemini API key found.".yellow());
        
        let yes_style = if selection == 0 { "[Yes]".green() } else { " Yes ".white() };
        let skip_style = if selection == 1 { "[Skip]".red() } else { " Skip ".white() };
        let never_style = if selection == 2 { "[Never Ask Again]".grey() } else { " Never Ask Again ".white() };
        
        println!("Would you like to add one now? {} {} {}", yes_style, skip_style, never_style);
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
        print!("{}", "Please enter your Gemini API key: ".cyan());
        stdout.flush()?;

        let mut key_input = String::new();
        io::stdin().read_line(&mut key_input)?;
        let key = key_input.trim();
        
        if key.is_empty() {
            return Ok(false);
        }

        config::save_api_key(key)?;
        println!("\n{}", "API key saved to ~/.config/whid/whid.toml. Starting application...".green());
        std::thread::sleep(Duration::from_secs(2));
        Ok(true)
    } else if selection == 1 { // Skip
        execute!(stdout, terminal::Clear(ClearType::All), crossterm::cursor::MoveTo(0,0))?;
        Ok(false)
    } else { // Never Ask Again
        config::disable_api_key_prompt()?;
        execute!(stdout, terminal::Clear(ClearType::All), crossterm::cursor::MoveTo(0,0))?;
        println!("{}", "Understood. The API key prompt has been disabled.".yellow());
        println!("You can add your key manually to your configuration file at:");
        println!("{}", config::get_user_config_path().display().to_string().cyan());
        println!("\nOr, set it as an environment variable: export GEMINI_API_KEY=your-key-here");
        println!("\n{}", "Starting application...".white());
        std::thread::sleep(Duration::from_secs(4));
        Ok(false)
    }
}
