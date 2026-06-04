use std::{sync::{Arc, Mutex}, time::Duration, path::PathBuf};
use crossterm::event::{KeyCode, MouseEvent, MouseEventKind};
use tokio::runtime::Runtime;
use arboard::Clipboard;
use crate::models::FocusArea;
use crate::models::PopupQuote;
use crate::git::reload_commits;
use crate::utils::{get_active_commits, CommitData};
use anyhow::Result;
use crate::models::SelectedCommits;

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
    popup_quote: &Arc<Mutex<PopupQuote>>,
    selected_commits: &Arc<Mutex<SelectedCommits>>,
    rt: &Runtime,
    selected_tab: &mut crate::CommitTab,
    lang: &str, // <-- add lang argument
    prompt_path: Option<&str>, // <-- add prompt_path argument
    gemini_model: &str, // <-- add gemini_model argument
    detailed_commit_view: &mut bool, // <-- add new argument
    from_date: Option<String>,
    to_date: Option<String>,
) -> Result<bool> {
    let lang = if lang.is_empty() { "english" } else { lang };
    match key {
        KeyCode::Char('1') => {
            *focus = FocusArea::Sidebar;
            if *selected_tab != crate::CommitTab::Timeframe { *selected_commit_index = None; }
            *selected_tab = crate::CommitTab::Timeframe;
        },
        KeyCode::Char('2') => {
            *focus = FocusArea::CommitList;
            if *selected_tab != crate::CommitTab::Timeframe { *selected_commit_index = None; }
            *selected_tab = crate::CommitTab::Timeframe;
        },
        KeyCode::Char('3') => {
            *focus = FocusArea::CommitList;
            if *selected_tab != crate::CommitTab::Selection { *selected_commit_index = None; }
            *selected_tab = crate::CommitTab::Selection;
        },
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
            // Toggle selection of current commit. Store the repo and full line so
            // the mark survives later timeframe changes.
            let mut sel = selected_commits.lock().unwrap();
            if let Some(idx) = *selected_commit_index {
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
                    let hash = commit.split_whitespace().next().unwrap_or("").to_string();
                    // Toggle: remove if already marked, otherwise insert.
                    if sel.set.remove(&hash).is_none() {
                        sel.set.insert(hash, (repo, commit));
                    }
                }
            }
        },
        KeyCode::Char('s') => {
            // Toggle the popup listing all marked commits.
            let mut sel = selected_commits.lock().unwrap();
            sel.popup_visible = !sel.popup_visible;
        },
        KeyCode::Tab => {
            // Tab cycles forward through timeframes
            if *current_index < intervals.len() - 1 {
                *current_index += 1;
            } else {
                *current_index = 0;
            }
            *current_interval = intervals[*current_index].1;
            *commits = reload_commits(repos, *current_interval, *filter_by_user, *detailed_commit_view, from_date.clone(), to_date.clone())?;
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
        KeyCode::BackTab => {
            // Shift+Tab cycles backward through timeframes
            if *current_index > 0 {
                *current_index -= 1;
            } else {
                *current_index = intervals.len() - 1;
            }
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
            // Popup scroll up
            let mut popup = popup_quote.lock().unwrap();
            if popup.visible && popup.scroll > 0 {
                popup.scroll -= 1;
                return Ok(true); // Prevent background navigation
            } else if popup.visible {
                return Ok(true); // Prevent background navigation
            }
            match *focus {
                FocusArea::Sidebar => {
                    // Sidebar: Up navigation
                    if (*selected_repo_index) == usize::MAX {
                        // Already at ALL, do nothing
                    } else if *selected_repo_index == 0 {
                        // Move to ALL
                        *selected_repo_index = usize::MAX;
                    } else {
                        *selected_repo_index -= 1;
                    }
                    *selected_commit_index = None;
                    if *selected_repo_index == usize::MAX { *sidebar_scroll = 0; }
                    else if *selected_repo_index < *sidebar_scroll { *sidebar_scroll = *selected_repo_index; }
                }
                FocusArea::CommitList => {
                    if *selected_repo_index == usize::MAX {
                        if let Some(idx) = *selected_commit_index {
                            if idx>0 { *selected_commit_index = Some(idx-1); }
                        } else if commits.iter().map(|(_,c)|c.len()).sum::<usize>()>0 { *selected_commit_index = Some(0); }
                    } else if let Some(idx)=*selected_commit_index {
                    if idx>0 { *selected_commit_index = Some(idx-1); } }
                    *commitlist_scroll = (*selected_commit_index).unwrap_or(0).min(*commitlist_scroll);
                }
                FocusArea::Detail => {
                    if *detail_scroll>0 { *detail_scroll -=1; }
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            // Popup scroll down
            let mut popup = popup_quote.lock().unwrap();
            if popup.visible {
                let text_lines = popup.text.lines().count() as u16;
                // Estimate popup height (centered_rect(60,80,area)), minus title/footer
                let area = crossterm::terminal::size().unwrap_or((120,40));
                let popup_height = (area.1 as f32 * 0.8) as u16 - 4;
                if popup.scroll + popup_height < text_lines {
                    popup.scroll += 1;
                }
                return Ok(true); // Prevent background navigation
            }
            match *focus {
                FocusArea::Sidebar => {
                    let repo_count = commits.len();
                    if *selected_repo_index == usize::MAX {
                        // Move from ALL to first project if any
                        if repo_count > 0 { *selected_repo_index = 0; }
                    } else if *selected_repo_index + 1 < repo_count {
                        *selected_repo_index += 1;
                    }
                    *selected_commit_index = None;
                    if *selected_repo_index == usize::MAX { *sidebar_scroll = 0; }
                    else if *selected_repo_index > *sidebar_scroll { *sidebar_scroll = *selected_repo_index; }
                }
                FocusArea::CommitList => {
                    let total_commits = if *selected_repo_index == usize::MAX {
                        commits.iter().map(|(_,c)|c.len()).sum::<usize>()
                    } else {
                        commits.get(*selected_repo_index).map(|(_,c)|c.len()).unwrap_or(0)
                    };
                    if let Some(idx) = *selected_commit_index {
                        if idx + 1 < total_commits { *selected_commit_index = Some(idx + 1); }
                    } else if total_commits > 0 {
                        *selected_commit_index = Some(0);
                    }
                    *commitlist_scroll = (*selected_commit_index).unwrap_or(0).max(*commitlist_scroll);
                }
                FocusArea::Detail => {
                    *detail_scroll += 1;
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
        KeyCode::Char('q') => return Ok(false),
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
            let (project_name, commit_str) = match selected_tab {
                crate::CommitTab::Timeframe => {
                    if (*selected_repo_index) == usize::MAX {
                        let all_commits = commits.iter()
                            .flat_map(|(repo, msgs)| {
                                let repo_name = repo.file_name().unwrap_or_default().to_string_lossy();
                                msgs.iter().map(move |msg| format!("[{}] {}", repo_name, msg))
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        ("All projects".to_string(), all_commits)
                    } else {
                        let project = commits.get(*selected_repo_index)
                            .map(|(repo, _)| repo.file_name().unwrap_or_default().to_string_lossy().to_string())
                            .unwrap_or_else(|| "Project".to_string());
                        let commitlist = commits.get(*selected_repo_index)
                            .map(|(_repo, msgs)| msgs.join("\n"))
                            .unwrap_or_default();
                        (project, commitlist)
                    }
                }
                crate::CommitTab::Selection => {
                    // Build from the stored selection (deterministic by hash) so
                    // it covers marks made under other timeframes too.
                    let sel = selected_commits.lock().unwrap();
                    let commit_str = sel.set.values()
                        .map(|(_repo, line)| line.clone())
                        .collect::<Vec<_>>()
                        .join("\n");
                    ("Selection".to_string(), commit_str)
                }
                crate::CommitTab::Stats => {
                    ("Stats".to_string(), String::new())
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
            {
                let mut p = popup_quote.lock().unwrap();
                p.visible = true;
                p.loading = true;
                p.spinner_frame = 0;
                p.text = match (&debug_msg, prompt_path) {
                    (Some(msg), Some(_)) | (Some(msg), None) => format!(
                        "{msg}\n\nPrompt variables:\n----------------\nfrom: {from}\nto: {to}\nproject: {project}\nlang: {lang}\ngemini_model: {gemini_model}\ncommits: [length: {} chars]\n\nLoading commit summary...",
                        commit_str.len(),
                        msg=msg,
                        from=from_date,
                        to=to_date,
                        project=project_name,
                        lang=lang,
                        gemini_model=gemini_model
                    ),
                    (None, _) => format!(
                        "Prompt variables:\n----------------\nfrom: {from}\nto: {to}\nproject: {project}\nlang: {lang}\ngemini_model: {gemini_model}\ncommits: [length: {} chars]\n\nLoading commit summary...",
                        commit_str.len(),
                        from=from_date,
                        to=to_date,
                        project=project_name,
                        lang=lang,
                        gemini_model=gemini_model
                    ),
                };
            }
            // Check for Gemini API key before spawning async task
            if std::env::var("GEMINI_API_KEY").is_err() {
                let config_path = crate::config::get_user_config_path();
                let error_message = format!(
                    "Gemini API key not found.\n\nPlease add it to your configuration file at:\n{}\n\nOr set it as an environment variable: export GEMINI_API_KEY=your-key",
                    config_path.display()
                );
                popup_quote.lock().unwrap().text = error_message;
                return Ok(true);
            }
            let p2 = popup_quote.clone();
            let lang_owned = lang.to_string();
            let gemini_model = gemini_model.to_string();
            rt.spawn(async move {
                // Animate spinner while loading
                let popup_clone = p2.clone();
                let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
                // Start spinner loop and summary fetch in parallel
                let fetch = crate::network::fetch_gemini_commit_summary(&prompt, &lang_owned, &gemini_model);
                tokio::pin!(fetch);
                loop {
                    tokio::select! {
                        _ = interval.tick() => {
                            let mut p = popup_clone.lock().unwrap();
                            if !p.loading { break; }
                            p.spinner_frame = p.spinner_frame.wrapping_add(1);
                        }
                        result = &mut fetch => {
                            let summary = match result {
                                Ok(s) => s,
                                Err(e) => format!("Gemini error: {}", e),
                            };
                            let mut p = popup_clone.lock().unwrap();
                            p.text = summary;
                            p.loading = false;
                            break;
                        }
                    }
                }
            });
        }
        KeyCode::Char('c') => {
            // Kopieren, wenn Popup sichtbar
            let popup = popup_quote.lock().unwrap();
            if popup.visible && !popup.loading {
                let mut clipboard = Clipboard::new().ok();
                if let Some(cb) = clipboard.as_mut() {
                    let _ = cb.set_text(popup.text.clone());
                }
            }
        }
        KeyCode::Esc => { 
            let mut p = popup_quote.lock().unwrap(); p.visible=false; p.scroll=0; 
            let mut sel = selected_commits.lock().unwrap(); sel.popup_visible = false;
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
    popup_quote: &Arc<Mutex<PopupQuote>>,
    sidebar_area: ratatui::prelude::Rect,
    selected_tab: &mut crate::CommitTab,
) {
    if let MouseEventKind::Down(_) = mouse_event.kind {
        let x = mouse_event.column;
        let y = mouse_event.row;
        // Check for popup summary X button
        {
            let popup = popup_quote.lock().unwrap();
            if popup.visible {
                // Popup area is centered_rect(60,80,area)
                // Get area from main window size
                let area = crossterm::terminal::size().unwrap_or((120,40));
                let area = ratatui::prelude::Rect { x: 0, y: 0, width: area.0, height: area.1 };
                let popup_area = crate::ui::centered_rect(60, 80, area);
                // X button is in the title, right side: [X] is 3 chars, with 1 space padding
                let x_button_x = popup_area.x + popup_area.width - 5; // [X] is at width-4, width-3, width-2
                let x_button_y = popup_area.y; // title line
                if y == x_button_y && x >= x_button_x && x < x_button_x + 3 {
                    // Clicked X
                    drop(popup); // unlock
                    let mut popup = popup_quote.lock().unwrap();
                    popup.visible = false;
                    return;
                }
            }
        }
        // Sidebar area: x < sidebar_area.x + sidebar_area.width
        if x >= sidebar_area.x && x < sidebar_area.x + sidebar_area.width && y >= sidebar_area.y && y < sidebar_area.y + sidebar_area.height {
            // Sidebar layout (inside the top border): rows 0-2 = "All Projects",
            // row 3 = divider, then each repo occupies 3 rows (name, count, blank).
            // Mirrors the rendering in ui.rs; assumes the list top is visible.
            let content_y = (y as usize).saturating_sub(sidebar_area.y as usize + 1);
            if content_y < 3 {
                *selected_repo_index = usize::MAX;
            } else if content_y >= 4 {
                let repo_idx = (content_y - 4) / 3;
                if repo_idx < commits.len() {
                    *selected_repo_index = repo_idx;
                    // Clicking a repo name switches to the timeframe tab (focus unchanged).
                    *selected_tab = crate::CommitTab::Timeframe;
                }
            }
            *selected_commit_index = None;
            return;
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
                crate::CommitTab::Selection | crate::CommitTab::Stats => {}
            }
            return;
        }
    }
    if let MouseEventKind::ScrollUp = mouse_event.kind {
        let popup_area = {
            let area = crossterm::terminal::size().unwrap_or((120,40));
            let area = ratatui::prelude::Rect { x: 0, y: 0, width: area.0, height: area.1 };
            crate::ui::centered_rect(60, 80, area)
        };
        let x = mouse_event.column;
        let y = mouse_event.row;
        if let Ok(mut popup) = popup_quote.lock() {
            if popup.visible && x >= popup_area.x && x < popup_area.x + popup_area.width && y >= popup_area.y && y < popup_area.y + popup_area.height {
                if popup.scroll > 0 {
                    popup.scroll -= 1;
                }
                return;
            }
        }
    }
    if let MouseEventKind::ScrollDown = mouse_event.kind {
        let popup_area = {
            let area = crossterm::terminal::size().unwrap_or((120,40));
            let area = ratatui::prelude::Rect { x: 0, y: 0, width: area.0, height: area.1 };
            crate::ui::centered_rect(60, 80, area)
        };
        let x = mouse_event.column;
        let y = mouse_event.row;
        if let Ok(mut popup) = popup_quote.lock() {
            if popup.visible && x >= popup_area.x && x < popup_area.x + popup_area.width && y >= popup_area.y && y < popup_area.y + popup_area.height {
                let text_lines = popup.text.lines().count() as u16;
                let popup_height = popup_area.height.saturating_sub(4); // account for padding/title/footer
                if popup.scroll + popup_height < text_lines {
                    popup.scroll += 1;
                }
            }
        }
    }
}