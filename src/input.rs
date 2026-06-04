use std::{sync::{Arc, Mutex}, time::Duration, path::PathBuf};
use crossterm::event::{KeyCode, MouseEvent, MouseEventKind};
use tokio::runtime::Runtime;
use arboard::Clipboard;
use crate::models::FocusArea;
use crate::models::{OverviewState, OverviewFocus, OverviewMeta};
use crate::git::reload_commits;
use crate::history::{self, OverviewRecord};
use crate::utils::{get_active_commits, CommitData};
use anyhow::Result;
use crate::models::SelectedCommits;
use crate::models::LockExt;
use crate::models::{LlmConfig, LlmProvider};
use ratatui::prelude::Rect;

/// Rows moved per Page key or mouse-wheel notch — responsive without feeling jumpy.
const PAGE_STEP: usize = 10;

/// Total commit rows for the current sidebar selection (sum across all repos
/// when "All" is selected, otherwise just the selected repo).
fn commit_total(commits: &CommitData, selected_repo_index: usize) -> usize {
    if selected_repo_index == usize::MAX {
        commits.iter().map(|(_, c)| c.len()).sum()
    } else {
        commits.get(selected_repo_index).map(|(_, c)| c.len()).unwrap_or(0)
    }
}

/// Moves the commit cursor by `delta` rows, clamped to `[0, total)`. A `None`
/// selection enters the list at the first row. Shared by keyboard nav, Page
/// keys and the mouse wheel so the clamping lives in one place.
fn step_commit(selected_commit_index: &mut Option<usize>, commitlist_scroll: &mut usize, total: usize, delta: isize) {
    if total == 0 {
        *selected_commit_index = None;
        return;
    }
    let next = match *selected_commit_index {
        None => 0,
        Some(cur) => (cur as isize + delta).clamp(0, total as isize - 1) as usize,
    };
    *selected_commit_index = Some(next);
    *commitlist_scroll = next;
}

/// Moves the sidebar selection by `delta`, where `usize::MAX` is the "All"
/// pseudo-row sitting just above repo index 0.
fn step_repo(
    selected_repo_index: &mut usize,
    selected_commit_index: &mut Option<usize>,
    sidebar_scroll: &mut usize,
    repo_count: usize,
    delta: isize,
) {
    let cur: isize = if *selected_repo_index == usize::MAX { -1 } else { *selected_repo_index as isize };
    let next = (cur + delta).clamp(-1, repo_count as isize - 1);
    *selected_repo_index = if next < 0 { usize::MAX } else { next as usize };
    *selected_commit_index = None;
    *sidebar_scroll = if *selected_repo_index == usize::MAX { 0 } else { *selected_repo_index };
}

/// Scrolls a Paragraph-style pane offset by `delta`, clamped at the top (0).
/// The bottom is left unclamped (matching the existing arrow-key behavior);
/// over-scroll is corrected on demand by End via `calculate_max_detail_scroll`.
fn step_scroll(scroll: &mut u16, delta: i32) {
    *scroll = (*scroll as i32 + delta).max(0) as u16;
}

// State is threaded by reference through the input layer (see CLAUDE.md), so
// these handlers necessarily take many parameters.
#[allow(clippy::too_many_arguments)]
pub fn handle_key(
    key: KeyCode,
    intervals: &[(&str, Duration)],
    current_index: &mut usize,
    current_interval: &mut Duration,
    filter_by_user: &mut bool,
    repos: &[PathBuf],
    commits: &mut CommitData,
    selected_repo_index: &mut usize,
    selected_commit_index: &mut Option<usize>,
    show_details: &mut bool,
    focus: &mut FocusArea,
    sidebar_scroll: &mut usize,
    commitlist_scroll: &mut usize,
    detail_scroll: &mut u16,
    overview_state: &Arc<Mutex<OverviewState>>,
    selected_commits: &Arc<Mutex<SelectedCommits>>,
    rt: &Runtime,
    selected_tab: &mut crate::CommitTab,
    lang: &str, // <-- add lang argument
    prompt_path: Option<&str>, // <-- add prompt_path argument
    llm: &LlmConfig, // <-- AI provider/model/endpoint/key for summaries
    detailed_commit_view: &mut bool, // <-- add new argument
    from_date: Option<String>,
    to_date: Option<String>,
    debug: bool, // <-- show prompt-construction debug info in the overview
    app_view: &mut crate::AppView,
    overview_selected: &mut usize,
    overview_focus: &mut OverviewFocus,
    overview_detail_scroll: &mut u16,
    show_help: &mut bool,
) -> Result<bool> {
    let lang = if lang.is_empty() { "english" } else { lang };

    // The help overlay swallows input: any key closes it (q still quits).
    if *show_help {
        match key {
            KeyCode::Char('q') => return Ok(false),
            _ => *show_help = false,
        }
        return Ok(true);
    }

    // Global keys, identical in every view: help, quit, and the two top-level
    // view switches. Handled before view-specific routing so they always work.
    match key {
        KeyCode::Char('?') => { *show_help = true; return Ok(true); }
        KeyCode::Char('q') => return Ok(false),
        KeyCode::Char('1') => {
            *app_view = crate::AppView::Commits;
            *focus = FocusArea::Sidebar;
            return Ok(true);
        }
        KeyCode::Char('3') => {
            // Full-screen stats dashboard, computed live from the loaded commits.
            *app_view = crate::AppView::Stats;
            return Ok(true);
        }
        KeyCode::Char('2') | KeyCode::Char('0') => {
            // Open the overview view if one exists (or is being generated);
            // otherwise it stays disabled and nothing happens.
            let available = {
                let s = overview_state.lock_safe();
                !s.items.is_empty() || s.generating
            };
            if available {
                *app_view = crate::AppView::Overview;
                *overview_selected = 0;
                *overview_focus = OverviewFocus::List;
                *overview_detail_scroll = 0;
            }
            return Ok(true);
        }
        _ => {}
    }

    // The overview view owns the rest of the keyboard while active; only `a`/`A`
    // (generate a fresh overview, which needs the commit state below) falls
    // through to the commit handler.
    if *app_view == crate::AppView::Overview && !matches!(key, KeyCode::Char('a') | KeyCode::Char('A')) {
        return handle_overview_key(
            key, overview_state, rt, app_view, overview_selected, overview_focus, overview_detail_scroll,
        );
    }

    // The stats dashboard is read-only: only timeframe (`[`/`]`/`w`) and filter
    // (`u`/`d`) keys are live — they reload `commits`, and the charts recompute
    // from it on the next redraw. Everything else is ignored so browser-only
    // keys (selection, focus, summary) can't act on a hidden commit list.
    if *app_view == crate::AppView::Stats
        && !matches!(
            key,
            KeyCode::Char('[') | KeyCode::Char(']') | KeyCode::Char('w')
                | KeyCode::Char('u') | KeyCode::Char('d')
        )
    {
        return Ok(true);
    }

    match key {
        KeyCode::Char('w') => {
            *current_index = 3;
            *current_interval = intervals[*current_index].1;
            *commits = reload_commits(repos, *current_interval, *filter_by_user, *detailed_commit_view, from_date, to_date)?;
            *selected_commit_index = None;
            // After reloading commits (timeframe/filter change), ensure selected_repo_index is valid
            if *selected_repo_index != usize::MAX {
                // If the selected repo is not present in the new commit list, reset to ALL
                if *selected_repo_index >= commits.len() {
                    *selected_repo_index = usize::MAX;
                    *selected_commit_index = None;
                }
            }
        },
        KeyCode::Char('m') => {
            // Toggle selection. Store the repo and full line so the mark survives
            // later timeframe changes.
            let mut sel = selected_commits.lock_safe();
            if *focus == FocusArea::Sidebar {
                // Bulk toggle: all current-timeframe commits of the selected repo,
                // or every repo when "All Projects" is selected.
                let scope: Vec<&(PathBuf, Vec<String>)> = if *selected_repo_index == usize::MAX {
                    commits.iter().collect()
                } else {
                    commits.get(*selected_repo_index).into_iter().collect()
                };
                // "Fill, then clear": only remove when EVERY commit in scope is
                // already marked (and the scope is not empty).
                let any = scope.iter().any(|(_, cs)| !cs.is_empty());
                let all_marked = any && scope.iter().all(|(_, cs)|
                    cs.iter().all(|c| sel.set.contains_key(crate::utils::commit_hash(c))));
                for (repo, cs) in scope {
                    for c in cs {
                        let hash = crate::utils::commit_hash(c).to_string();
                        if all_marked {
                            sel.set.remove(&hash);
                        } else {
                            sel.set.entry(hash).or_insert_with(|| (repo.clone(), c.clone()));
                        }
                    }
                }
            } else if let Some(idx) = *selected_commit_index {
                let found = if *selected_repo_index == usize::MAX {
                    // global index across all repos
                    let mut offset = 0;
                    let mut hit = None;
                    for (repo, repo_commits) in commits.iter() {
                        if idx < offset + repo_commits.len() {
                            hit = repo_commits.get(idx - offset).map(|c| (repo.clone(), c.clone()));
                            break;
                        }
                        offset += repo_commits.len();
                    }
                    hit
                } else {
                    commits.get(*selected_repo_index)
                        .and_then(|(repo, repo_commits)| repo_commits.get(idx).map(|c| (repo.clone(), c.clone())))
                };
                if let Some((repo, commit)) = found {
                    let hash = crate::utils::commit_hash(&commit).to_string();
                    // Toggle: remove if already marked, otherwise insert.
                    if sel.set.remove(&hash).is_none() {
                        sel.set.insert(hash, (repo, commit));
                    }
                }
            }
        },
        KeyCode::Char('s') => {
            // Toggle the commit-list mode between Timeframe and Selection.
            *focus = FocusArea::CommitList;
            *selected_tab = match *selected_tab {
                crate::CommitTab::Selection => crate::CommitTab::Timeframe,
                _ => crate::CommitTab::Selection,
            };
            *selected_commit_index = None;
        },
        KeyCode::Tab => {
            // Tab cycles focus forward through the panes.
            *focus = match *focus {
                FocusArea::Sidebar => FocusArea::CommitList,
                FocusArea::CommitList => if *show_details { FocusArea::Detail } else { FocusArea::Sidebar },
                FocusArea::Detail => FocusArea::Sidebar,
            };
        },
        KeyCode::BackTab => {
            // Shift+Tab cycles focus backward through the panes.
            *focus = match *focus {
                FocusArea::Sidebar => if *show_details { FocusArea::Detail } else { FocusArea::CommitList },
                FocusArea::CommitList => FocusArea::Sidebar,
                FocusArea::Detail => FocusArea::CommitList,
            };
        },
        KeyCode::Char('[') | KeyCode::Char(']') => {
            // Cycle the timeframe backward ('[') / forward (']').
            let forward = matches!(key, KeyCode::Char(']'));
            if forward {
                *current_index = (*current_index + 1) % intervals.len();
            } else {
                *current_index = (*current_index + intervals.len() - 1) % intervals.len();
            }
            *current_interval = intervals[*current_index].1;
            *commits = reload_commits(repos, *current_interval, *filter_by_user, *detailed_commit_view, from_date, to_date)?;
            *selected_commit_index = None;
            // Ensure selected_repo_index is still valid after the reload.
            if *selected_repo_index != usize::MAX && *selected_repo_index >= commits.len() {
                *selected_repo_index = usize::MAX;
                *selected_commit_index = None;
            }
        },
        KeyCode::Char(' ') => {
            if *focus == FocusArea::CommitList {
                // Ensure a commit is selected
                if selected_commit_index.is_none() {
                    if *selected_repo_index == usize::MAX {
                        // global first commit
                        if let Some((_, commits_list)) = commits.first() {
                            if !commits_list.is_empty() {
                                *selected_commit_index = Some(0);
                            }
                        }
                    } else if let Some(repo_commits) = get_active_commits(commits, *selected_repo_index) {
                        if !repo_commits.is_empty() {
                            *selected_commit_index = Some(0);
                        }
                    }
                }
                // Toggle detail view
                *show_details = !*show_details;
                if !*show_details { *focus = FocusArea::CommitList; }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            match *focus {
                FocusArea::Sidebar => step_repo(selected_repo_index, selected_commit_index, sidebar_scroll, commits.len(), -1),
                FocusArea::CommitList => {
                    let total = commit_total(commits, *selected_repo_index);
                    step_commit(selected_commit_index, commitlist_scroll, total, -1);
                }
                FocusArea::Detail => step_scroll(detail_scroll, -1),
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match *focus {
                FocusArea::Sidebar => step_repo(selected_repo_index, selected_commit_index, sidebar_scroll, commits.len(), 1),
                FocusArea::CommitList => {
                    let total = commit_total(commits, *selected_repo_index);
                    step_commit(selected_commit_index, commitlist_scroll, total, 1);
                }
                FocusArea::Detail => step_scroll(detail_scroll, 1),
            }
        }
        KeyCode::PageUp | KeyCode::PageDown => {
            let delta = if matches!(key, KeyCode::PageDown) { PAGE_STEP as isize } else { -(PAGE_STEP as isize) };
            match *focus {
                FocusArea::Sidebar => step_repo(selected_repo_index, selected_commit_index, sidebar_scroll, commits.len(), delta),
                FocusArea::CommitList => {
                    let total = commit_total(commits, *selected_repo_index);
                    step_commit(selected_commit_index, commitlist_scroll, total, delta);
                }
                FocusArea::Detail => step_scroll(detail_scroll, delta as i32),
            }
        }
        KeyCode::Home => {
            match *focus {
                FocusArea::Sidebar => {
                    *selected_repo_index = usize::MAX;
                    *selected_commit_index = None;
                    *sidebar_scroll = 0;
                }
                FocusArea::CommitList => {
                    let total = commit_total(commits, *selected_repo_index);
                    *selected_commit_index = if total > 0 { Some(0) } else { None };
                    *commitlist_scroll = 0;
                }
                FocusArea::Detail => *detail_scroll = 0,
            }
        }
        KeyCode::End => {
            match *focus {
                FocusArea::Sidebar => {
                    let repo_count = commits.len();
                    *selected_repo_index = if repo_count > 0 { repo_count - 1 } else { usize::MAX };
                    *selected_commit_index = None;
                    *sidebar_scroll = if *selected_repo_index == usize::MAX { 0 } else { *selected_repo_index };
                }
                FocusArea::CommitList => {
                    let total = commit_total(commits, *selected_repo_index);
                    *selected_commit_index = if total > 0 { Some(total - 1) } else { None };
                    *commitlist_scroll = selected_commit_index.unwrap_or(0);
                }
                FocusArea::Detail => {
                    if let Some(idx) = *selected_commit_index {
                        // The detail pane spans the content row, so its inner text
                        // height is the terminal rows minus the top bar, footer and
                        // borders. Computed on demand (End shells out to git).
                        let view_height = crossterm::terminal::size().map(|(_, r)| r.saturating_sub(6)).unwrap_or(20);
                        *detail_scroll = crate::utils::calculate_max_detail_scroll(commits, *selected_repo_index, idx, view_height);
                    }
                }
            }
        }
        KeyCode::Char('h') | KeyCode::Left => {
            // vim 'h' or Left Arrow as focus backward
            *focus = match *focus {
                FocusArea::Sidebar => {
                    if *show_details { FocusArea::Detail } else { FocusArea::CommitList }
                }
                FocusArea::CommitList => FocusArea::Sidebar,
                FocusArea::Detail => FocusArea::CommitList,
            };
        }
        KeyCode::Char('l') | KeyCode::Right => {
            // vim 'l' or Right Arrow as focus forward
            *focus = match *focus {
                FocusArea::Sidebar => FocusArea::CommitList,
                FocusArea::CommitList => {
                    if *show_details { FocusArea::Detail } else { FocusArea::Sidebar }
                }
                FocusArea::Detail => FocusArea::Sidebar,
            };
        }
        KeyCode::Char('u') => {
            *filter_by_user = !*filter_by_user;
            *commits = reload_commits(repos, *current_interval, *filter_by_user, *detailed_commit_view, from_date, to_date)?;
            *selected_commit_index=None;
            *detail_scroll=0;
            // After reloading commits (timeframe/filter change), ensure selected_repo_index is valid
            if *selected_repo_index != usize::MAX {
                // If the selected repo is not present in the new commit list, reset to ALL
                if *selected_repo_index >= commits.len() {
                    *selected_repo_index = usize::MAX;
                    *selected_commit_index = None;
                }
            }
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            // Load the custom prompt template once (if configured) and reuse it below.
            let loaded_template = prompt_path.map(|path| (path, std::fs::read_to_string(path)));
            let debug_msg = match &loaded_template {
                Some((path, Ok(_))) => Some(format!("Prompt loaded from {}", path)),
                Some((path, Err(e))) => Some(format!("Error loading {}: {}. Falling back to default prompt.", path, e)),
                None => Some(String::new()),
            };
            // --- Gemini prompt construction update ---
            use chrono::Local;
            let now = Local::now();
            let to_date = now.format("%Y-%m-%d").to_string();
            let from_date = (now - *current_interval).format("%Y-%m-%d").to_string();
            let interval_str = intervals[*current_index].0;
            let (project_name, commit_str, commit_count) = match selected_tab {
                crate::CommitTab::Timeframe => {
                    if (*selected_repo_index) == usize::MAX {
                        let all_commits = commits.iter()
                            .flat_map(|(repo, msgs)| {
                                let repo_name = repo.file_name().unwrap_or_default().to_string_lossy();
                                msgs.iter().map(move |msg| format!("[{}] {}", repo_name, msg))
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        let count = commits.iter().map(|(_, c)| c.len()).sum();
                        ("All projects".to_string(), all_commits, count)
                    } else {
                        let project = commits.get(*selected_repo_index)
                            .map(|(repo, _)| repo.file_name().unwrap_or_default().to_string_lossy().to_string())
                            .unwrap_or_else(|| "Project".to_string());
                        let commitlist = commits.get(*selected_repo_index)
                            .map(|(_repo, msgs)| msgs.join("\n"))
                            .unwrap_or_default();
                        let count = commits.get(*selected_repo_index).map(|(_, c)| c.len()).unwrap_or(0);
                        (project, commitlist, count)
                    }
                }
                crate::CommitTab::Selection => {
                    // Build from the stored selection (deterministic by hash) so
                    // it covers marks made under other timeframes too.
                    let sel = selected_commits.lock_safe();
                    let commit_str = sel.set.values()
                        .map(|(_repo, line)| line.clone())
                        .collect::<Vec<_>>()
                        .join("\n");
                    ("Selection".to_string(), commit_str, sel.set.len())
                }
            };
            let prompt = match &loaded_template {
                Some((_path, Ok(template))) => template
                    .replace("{from}", &from_date)
                    .replace("{to}", &to_date)
                    .replace("{project}", &project_name)
                    .replace("{projectname}", &project_name)
                    .replace("{interval}", interval_str)
                    .replace("{lang}", lang)
                    .replace("{commits}", &commit_str),
                Some((path, Err(e))) => {
                    eprintln!("Error loading custom prompt '{}': {}. Falling back to default prompt.", path, e);
                    crate::prompts::prompt_en(&from_date, &to_date, &project_name, lang, &commit_str)
                }
                None => crate::prompts::prompt_en(&from_date, &to_date, &project_name, lang, &commit_str),
            };
            let provider_str = match llm.provider {
                LlmProvider::Gemini => "gemini",
                LlmProvider::Custom => "custom",
            };
            let meta = OverviewMeta {
                project: project_name.clone(),
                interval: interval_str.to_string(),
                from: from_date.clone(),
                to: to_date.clone(),
                lang: lang.to_string(),
                provider: provider_str.to_string(),
                model: llm.model.clone(),
                commit_count,
                tab: match selected_tab {
                    crate::CommitTab::Timeframe => "Timeframe",
                    crate::CommitTab::Selection => "Selection",
                }.to_string(),
            };
            // Under `--debug` the transient line carries prompt-construction
            // detail; otherwise a concise loading message.
            let transient = if debug {
                let msg = debug_msg.as_deref().unwrap_or("");
                format!(
                    "{msg}\n\nPrompt variables:\n----------------\nfrom: {from}\nto: {to}\nproject: {project}\nlang: {lang}\nprovider: {provider}\nmodel: {model}\ncommits: [{count} commits, {} chars]\n\nLoading commit summary...",
                    commit_str.len(),
                    msg=msg,
                    from=from_date,
                    to=to_date,
                    project=project_name,
                    lang=lang,
                    provider=provider_str,
                    model=llm.model,
                    count=commit_count,
                )
            } else {
                format!(
                    "Summarizing {} commit{} from {}…",
                    commit_count,
                    if commit_count == 1 { "" } else { "s" },
                    project_name,
                )
            };
            // Switch to the overview view so the spinner + result appear there.
            *app_view = crate::AppView::Overview;
            *overview_selected = 0;
            *overview_focus = OverviewFocus::List;
            *overview_detail_scroll = 0;
            {
                let mut s = overview_state.lock_safe();
                s.generating = true;
                s.spinner_frame = 0;
                s.copied = false;
                s.transient = Some(transient);
            }
            // For Gemini, surface a missing key immediately (no spinner). For
            // custom OpenAI-compatible providers the network layer returns a
            // helpful message for missing url/model/key, so we let it flow.
            if llm.provider == LlmProvider::Gemini && std::env::var("GEMINI_API_KEY").is_err() {
                let config_path = crate::config::get_user_config_path();
                let error_message = format!(
                    "Gemini API key not found.\n\nPlease add it to your configuration file at:\n{}\n\nOr set it as an environment variable: export GEMINI_API_KEY=your-key",
                    config_path.display()
                );
                let mut s = overview_state.lock_safe();
                s.generating = false;
                s.transient = Some(error_message);
                return Ok(true);
            }
            spawn_summary(rt, overview_state, prompt, lang.to_string(), llm.clone(), meta);
        }
        KeyCode::Esc => {
            // Esc closes the detail pane if open; otherwise it cancels any
            // in-flight generation (clearing `generating` makes the spinner loop
            // break on its next tick, dropping the pending fetch future).
            if *show_details {
                *show_details = false;
                if *focus == FocusArea::Detail { *focus = FocusArea::CommitList; }
            } else {
                overview_state.lock_safe().generating = false;
            }
        }
        KeyCode::Char('d') => {
            *detailed_commit_view = !*detailed_commit_view;
            *commits = reload_commits(repos, *current_interval, *filter_by_user, *detailed_commit_view, from_date, to_date)?;
        },
        _ => {}
    }
    *current_interval = intervals[*current_index].1;
    Ok(true)
}

pub fn handle_mouse(
    mouse_event: MouseEvent,
    commits: &CommitData,
    selected_repo_index: &mut usize,
    selected_commit_index: &mut Option<usize>,
    focus: &mut FocusArea,
    sidebar_area: ratatui::prelude::Rect,
    selected_tab: &mut crate::CommitTab,
) {
    if let MouseEventKind::Down(_) = mouse_event.kind {
        let x = mouse_event.column;
        let y = mouse_event.row;
        // Sidebar area: x < sidebar_area.x + sidebar_area.width
        if x >= sidebar_area.x && x < sidebar_area.x + sidebar_area.width && y >= sidebar_area.y && y < sidebar_area.y + sidebar_area.height {
            // Sidebar layout (inside the top border): row 0 = "All Projects",
            // row 1 = divider, then one row per repo. Mirrors the rendering in
            // ui.rs; assumes the list top is visible.
            let content_y = (y as usize).saturating_sub(sidebar_area.y as usize + 1);
            if content_y == 0 {
                *selected_repo_index = usize::MAX;
            } else if content_y >= 2 {
                let repo_idx = content_y - 2;
                if repo_idx < commits.len() {
                    *selected_repo_index = repo_idx;
                    // Clicking a repo name switches to the timeframe tab (focus unchanged).
                    *selected_tab = crate::CommitTab::Timeframe;
                }
            }
            *selected_commit_index = None;
        } else {
            // Commit list / selection area (everything right of the sidebar).
            // The list begins below the 3-row tab bar and inside the list block
            // border, so the first commit row is at sidebar_area.y + 4. The
            // sidebar and commit list share the same vertical chunk, hence the
            // shared height. Mapping assumes the list top is visible: the List
            // widget owns its auto-scroll offset and does not expose it, so
            // clicks on a scrolled list are best-effort (see todo V2).
            *focus = FocusArea::CommitList;
            let list_top = sidebar_area.y + 4;
            let list_bottom = sidebar_area.y + sidebar_area.height;
            if y < list_top || y >= list_bottom {
                return;
            }
            let mut row = (y - list_top) as usize;
            match *selected_tab {
                crate::CommitTab::Timeframe => {
                    if *selected_repo_index == usize::MAX {
                        // "All" view interleaves one repo-header row before each
                        // repo's commits — skip those when mapping the click.
                        let mut global = 0;
                        for (_repo, repo_commits) in commits.iter() {
                            if row == 0 {
                                return; // clicked a repo header
                            }
                            row -= 1;
                            if row < repo_commits.len() {
                                *selected_commit_index = Some(global + row);
                                return;
                            }
                            row -= repo_commits.len();
                            global += repo_commits.len();
                        }
                    } else if let Some((_repo, repo_commits)) = commits.get(*selected_repo_index) {
                        if row < repo_commits.len() {
                            *selected_commit_index = Some(row);
                        }
                    }
                }
                // Selection list uses a header-interleaved, path-sorted layout
                // whose index space differs from the timeframe view; just focus
                // it rather than guessing a wrong commit index.
                crate::CommitTab::Selection => {}
            }
        }
    }
}
/// Mouse-wheel scrolling for the commit browser. The hovered region (not the
/// focused pane) decides what moves, matching how wheels behave elsewhere.
#[allow(clippy::too_many_arguments)]
pub fn handle_mouse_scroll(
    col: u16,
    row: u16,
    down: bool,
    layout: &crate::ui::AppLayout,
    commits: &CommitData,
    selected_repo_index: &mut usize,
    selected_commit_index: &mut Option<usize>,
    commitlist_scroll: &mut usize,
    sidebar_scroll: &mut usize,
    detail_scroll: &mut u16,
) {
    let delta: isize = if down { 1 } else { -1 };
    let inside = |r: Rect| col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height;
    if let Some(detail) = layout.detail {
        if inside(detail) {
            step_scroll(detail_scroll, delta as i32);
            return;
        }
    }
    if inside(layout.sidebar) {
        step_repo(selected_repo_index, selected_commit_index, sidebar_scroll, commits.len(), delta);
    } else if inside(layout.commit) {
        let total = commit_total(commits, *selected_repo_index);
        step_commit(selected_commit_index, commitlist_scroll, total, delta);
    }
}

/// Mouse-wheel scrolling for the overview view: the left third is the overview
/// list, the rest is the detail pane. Mirrors the master/detail split in
/// `render_overview`, so the boundary is derived the same way.
pub fn handle_overview_scroll(
    col: u16,
    area: Rect,
    down: bool,
    overview_state: &Arc<Mutex<OverviewState>>,
    overview_selected: &mut usize,
    overview_detail_scroll: &mut u16,
) {
    let list_w = (area.width / 3).clamp(28, 44);
    let in_list = col < area.x + list_w;
    let len = overview_state.lock_safe().items.len();
    if in_list {
        if down {
            if len > 0 && *overview_selected + 1 < len {
                *overview_selected += 1;
                *overview_detail_scroll = 0;
            }
        } else if *overview_selected > 0 {
            *overview_selected -= 1;
            *overview_detail_scroll = 0;
        }
    } else if down {
        *overview_detail_scroll = overview_detail_scroll.saturating_add(1);
    } else {
        *overview_detail_scroll = overview_detail_scroll.saturating_sub(1);
    }
}

/// Copies `text` to the system clipboard, returning whether it succeeded.
fn copy_to_clipboard(text: &str) -> bool {
    match Clipboard::new() {
        Ok(mut cb) => cb.set_text(text.to_owned()).is_ok(),
        Err(_) => false,
    }
}

/// Returns true if `text` looks like an error message rather than a real
/// summary, so it is shown transiently but never persisted to history.
fn looks_like_error(text: &str) -> bool {
    const PREFIXES: [&str; 6] = [
        "AI error",
        "Gemini API error",
        "Gemini API key not found",
        "Custom API",
        "No OpenAI-compatible",
        "No summary received",
    ];
    let t = text.trim_start();
    PREFIXES.iter().any(|p| t.starts_with(p))
        || t.contains("API key not found")
        || t.contains("API key not configured")
}

/// Handles all keyboard input while the overview view is active. Returns
/// `Ok(false)` to quit the app, `Ok(true)` otherwise.
fn handle_overview_key(
    key: KeyCode,
    overview_state: &Arc<Mutex<OverviewState>>,
    rt: &Runtime,
    app_view: &mut crate::AppView,
    overview_selected: &mut usize,
    overview_focus: &mut OverviewFocus,
    overview_detail_scroll: &mut u16,
) -> Result<bool> {
    let len = overview_state.lock_safe().items.len();

    // While a delete is pending, only y/n (or Esc) are live — everything else
    // is ignored so a stray key can't act on the wrong overview.
    if overview_state.lock_safe().pending_delete {
        match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let items = {
                    let mut s = overview_state.lock_safe();
                    s.pending_delete = false;
                    if *overview_selected < s.items.len() {
                        s.items.remove(*overview_selected);
                        s.copied = false;
                    }
                    if *overview_selected >= s.items.len() {
                        *overview_selected = s.items.len().saturating_sub(1);
                    }
                    s.items.clone()
                };
                let _ = history::save_overviews(&items);
                *overview_detail_scroll = 0;
                // Nothing left → the view becomes disabled again, go back.
                if items.is_empty() {
                    *app_view = crate::AppView::Commits;
                }
            }
            _ => {
                overview_state.lock_safe().pending_delete = false;
            }
        }
        return Ok(true);
    }

    match key {
        // Leave the overview view back to the commit browser; if a summary is
        // still generating, cancel it instead.
        KeyCode::Esc => {
            let mut s = overview_state.lock_safe();
            if s.generating {
                s.generating = false;
            } else {
                drop(s);
                *app_view = crate::AppView::Commits;
            }
        }
        // Toggle focus between the list and the detail pane.
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab | KeyCode::BackTab => {
            *overview_focus = match *overview_focus {
                OverviewFocus::List => OverviewFocus::Detail,
                OverviewFocus::Detail => OverviewFocus::List,
            };
        }
        KeyCode::Up | KeyCode::Char('k') => match *overview_focus {
            OverviewFocus::List => {
                if *overview_selected > 0 {
                    *overview_selected -= 1;
                    *overview_detail_scroll = 0;
                }
            }
            OverviewFocus::Detail => {
                *overview_detail_scroll = overview_detail_scroll.saturating_sub(1);
            }
        },
        KeyCode::Down | KeyCode::Char('j') => match *overview_focus {
            OverviewFocus::List => {
                if len > 0 && *overview_selected + 1 < len {
                    *overview_selected += 1;
                    *overview_detail_scroll = 0;
                }
            }
            OverviewFocus::Detail => {
                *overview_detail_scroll = overview_detail_scroll.saturating_add(1);
            }
        },
        KeyCode::PageUp | KeyCode::PageDown => {
            let down = matches!(key, KeyCode::PageDown);
            match *overview_focus {
                OverviewFocus::List => {
                    if len > 0 {
                        let delta = if down { PAGE_STEP as isize } else { -(PAGE_STEP as isize) };
                        let next = (*overview_selected as isize + delta).clamp(0, len as isize - 1) as usize;
                        if next != *overview_selected {
                            *overview_selected = next;
                            *overview_detail_scroll = 0;
                        }
                    }
                }
                OverviewFocus::Detail => {
                    let step = PAGE_STEP as u16;
                    *overview_detail_scroll = if down {
                        overview_detail_scroll.saturating_add(step)
                    } else {
                        overview_detail_scroll.saturating_sub(step)
                    };
                }
            }
        }
        KeyCode::Home => match *overview_focus {
            OverviewFocus::List => {
                if *overview_selected != 0 {
                    *overview_selected = 0;
                    *overview_detail_scroll = 0;
                }
            }
            OverviewFocus::Detail => *overview_detail_scroll = 0,
        },
        KeyCode::End => match *overview_focus {
            OverviewFocus::List => {
                if len > 0 {
                    let last = len - 1;
                    if *overview_selected != last {
                        *overview_selected = last;
                        *overview_detail_scroll = 0;
                    }
                }
            }
            OverviewFocus::Detail => {
                let lines = overview_state
                    .lock_safe()
                    .items
                    .get(*overview_selected)
                    .map(|r| r.text.lines().count())
                    .unwrap_or(0);
                let view_height = crossterm::terminal::size().map(|(_, r)| r.saturating_sub(10)).unwrap_or(20);
                *overview_detail_scroll = (lines as u16).saturating_sub(view_height);
            }
        },
        // Copy the selected overview's text to the clipboard.
        KeyCode::Char('c') | KeyCode::Enter => {
            let mut s = overview_state.lock_safe();
            if let Some(rec) = s.items.get(*overview_selected) {
                if copy_to_clipboard(&rec.text) {
                    s.copied = true;
                }
            }
        }
        // Arm an inline delete confirmation (resolved by y/n in the guard above).
        KeyCode::Char('x') | KeyCode::Delete => {
            let mut s = overview_state.lock_safe();
            if !s.items.is_empty() {
                s.pending_delete = true;
                s.copied = false;
            }
        }
        // Regenerate from the last dispatched request.
        KeyCode::Char('r') => {
            let request = {
                let s = overview_state.lock_safe();
                if s.generating { None } else { s.last_request.clone() }
            };
            if let Some((prompt, req_lang, req_llm, meta)) = request {
                {
                    let mut s = overview_state.lock_safe();
                    s.generating = true;
                    s.spinner_frame = 0;
                    s.copied = false;
                    s.transient = Some("Regenerating summary…".to_string());
                }
                *overview_selected = 0;
                *overview_detail_scroll = 0;
                spawn_summary(rt, overview_state, prompt, req_lang, req_llm, meta);
            }
        }
        _ => {}
    }
    Ok(true)
}

/// Spawns the AI summary fetch on the shared runtime and animates the spinner
/// until it resolves. On success the result is stored as an `OverviewRecord`
/// (newest first, capped) and persisted; errors are shown transiently only.
/// The single place that dispatches a summary.
fn spawn_summary(
    rt: &Runtime,
    overview_state: &Arc<Mutex<OverviewState>>,
    prompt: String,
    lang: String,
    llm: LlmConfig,
    meta: OverviewMeta,
) {
    // Remember the request so `r` can regenerate it later.
    overview_state.lock_safe().last_request =
        Some((prompt.clone(), lang.clone(), llm.clone(), meta.clone()));
    let state = overview_state.clone();
    rt.spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
        // Run the spinner loop and the fetch concurrently.
        let fetch = crate::network::fetch_commit_summary(&prompt, &lang, &llm);
        tokio::pin!(fetch);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let mut s = state.lock_safe();
                    if !s.generating { break; }
                    s.spinner_frame = s.spinner_frame.wrapping_add(1);
                }
                result = &mut fetch => {
                    let summary = match result {
                        Ok(s) => s,
                        Err(e) => format!("AI error: {}", e),
                    };
                    let mut s = state.lock_safe();
                    s.generating = false;
                    if looks_like_error(&summary) {
                        // Surface the error transiently; do not persist it.
                        s.transient = Some(summary);
                    } else {
                        let now = chrono::Local::now();
                        let record = OverviewRecord {
                            created_at: now.format("%Y-%m-%d %H:%M").to_string(),
                            created_unix: now.timestamp(),
                            text: summary,
                            project: meta.project,
                            interval: meta.interval,
                            from: meta.from,
                            to: meta.to,
                            lang: meta.lang,
                            provider: meta.provider,
                            model: meta.model,
                            commit_count: meta.commit_count,
                            tab: meta.tab,
                        };
                        s.items.insert(0, record);
                        s.items.truncate(history::MAX_OVERVIEWS);
                        s.transient = None;
                        let _ = history::save_overviews(&s.items);
                    }
                    break;
                }
            }
        }
    });
}
