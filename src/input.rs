use std::{sync::{Arc, Mutex}, time::Duration, path::PathBuf, fs};
use crossterm::event::{KeyCode, MouseEvent, MouseEventKind};
use tokio::runtime::Runtime;
use arboard::Clipboard;
use crate::models::FocusArea;
use crate::models::PopupQuote;
use crate::git::reload_commits;
use crate::utils::{get_active_commits, get_sidebar_height, get_commitlist_height, calculate_max_detail_scroll};
use crate::network::fetch_gemini_commit_summary;
use anyhow::Result;
use crate::models::SelectedCommits;

pub fn handle_key(
    key: KeyCode,
    intervals: &[(&str, Duration)],
    current_index: &mut usize,
    current_interval: &mut Duration,
    filter_by_user: &mut bool,
    repos: &Vec<PathBuf>,
    commits: &mut Vec<(PathBuf, Vec<String>)>,
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
) -> Result<bool> {
    match key {
        KeyCode::Char('1') => {
            *focus = FocusArea::Sidebar;
        },
        KeyCode::Char('2') => {
            *focus = FocusArea::CommitList;
        },
        KeyCode::Char('3') => {
            *focus = FocusArea::CommitList;
        },
        KeyCode::Char('w') => {
            *current_index = 3;
            *current_interval = intervals[*current_index].1;
            *commits = reload_commits(repos, *current_interval, *filter_by_user)?;
            *selected_commit_index = None;
        },
        KeyCode::Char('m') => {
            // Toggle selection of current commit
            let mut sel = selected_commits.lock().unwrap();
            if let Some(idx) = *selected_commit_index {
                let commit_str = if *selected_repo_index == usize::MAX {
                    // global index
                    let mut offset = 0;
                    let mut found = None;
                    for (_repo, repo_commits) in commits.iter() {
                        if idx < offset + repo_commits.len() {
                            found = repo_commits.get(idx - offset).cloned();
                            break;
                        }
                        offset += repo_commits.len();
                    }
                    found
                } else {
                    commits.get(*selected_repo_index)
                        .and_then(|(_repo, repo_commits)| repo_commits.get(idx).cloned())
                };
                if let Some(commit) = commit_str {
                    let hash = commit.split_whitespace().next().unwrap_or("").to_string();
                    if sel.set.contains(&hash) {
                        sel.set.remove(&hash);
                    } else {
                        sel.set.insert(hash);
                    }
                }
            }
        },
        KeyCode::Char('s') => {
        },
        KeyCode::Tab => {
            // Tab cycles forward through timeframes
            if *current_index < intervals.len() - 1 {
                *current_index += 1;
            } else {
                *current_index = 0;
            }
            *current_interval = intervals[*current_index].1;
            *commits = reload_commits(repos, *current_interval, *filter_by_user)?;
            *selected_commit_index = None;
        },
        KeyCode::BackTab => {
            // Shift+Tab cycles backward through timeframes
            if *current_index > 0 {
                *current_index -= 1;
            } else {
                *current_index = intervals.len() - 1;
            }
            *current_interval = intervals[*current_index].1;
            *commits = reload_commits(repos, *current_interval, *filter_by_user)?;
            *selected_commit_index = None;
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
                FocusArea::Sidebar => {
                    let repo_count = commits.len();
                    if *selected_repo_index == usize::MAX {
                        if repo_count>0 { *selected_repo_index = 0; }
                    } else if *selected_repo_index > 0 { *selected_repo_index -= 1; }
                    *selected_commit_index = None;
                    if *selected_repo_index == usize::MAX { *sidebar_scroll = 0; }
                    else if *selected_repo_index < *sidebar_scroll { *sidebar_scroll = *selected_repo_index; }
                }
                FocusArea::CommitList => {
                    if *selected_repo_index == usize::MAX {
                        if let Some(idx) = *selected_commit_index {
                            if idx>0 { *selected_commit_index = Some(idx-1); }
                        } else {
                            if commits.iter().map(|(_,c)|c.len()).sum::<usize>()>0 { *selected_commit_index = Some(0); }
                        }
                    } else {
                        if let Some(idx)=*selected_commit_index {
                            if idx>0 { *selected_commit_index = Some(idx-1); } }
                        else { *selected_commit_index = Some(0); }
                    }
                    *commitlist_scroll = (*selected_commit_index).unwrap_or(0).min(*commitlist_scroll);
                }
                FocusArea::Detail => {
                    if *detail_scroll>0 { *detail_scroll -=1; }
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match *focus {
                FocusArea::Sidebar => {
                    let repo_count = commits.len();
                    if *selected_repo_index == usize::MAX {
                        if repo_count > 0 { *selected_repo_index = 0; }
                    } else if *selected_repo_index + 1 < repo_count { *selected_repo_index += 1; }
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
        KeyCode::Char('h') => {
            // vim 'h' as focus backward
            *focus = match *focus {
                FocusArea::Sidebar => {
                    if *show_details { FocusArea::Detail } else { FocusArea::CommitList }
                }
                FocusArea::CommitList => FocusArea::Sidebar,
                FocusArea::Detail => FocusArea::CommitList,
            };
        }
        KeyCode::Char('l') => {
            // vim 'l' as focus forward
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
            *commits = reload_commits(repos, *current_interval, *filter_by_user)?;
            *selected_commit_index=None;
            *detail_scroll=0;
        }
        KeyCode::Char('q') => return Ok(false),
        KeyCode::Char('a') => {
            // Prompt-Template aus Datei laden
            let prompt_template = fs::read_to_string("prompt.txt").unwrap_or_default();
            // Projektname und Zeitfenster bestimmen
            let (project_name, commit_str) = if *selected_repo_index == usize::MAX {
                let all_commits = commits.iter()
                    .flat_map(|(repo, msgs)| {
                        let repo_name = repo.file_name().unwrap_or_default().to_string_lossy();
                        msgs.iter().map(move |msg| format!("[{}] {}", repo_name, msg))
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                ("Alle Projekte".to_string(), all_commits)
            } else {
                let project = commits.get(*selected_repo_index)
                    .map(|(repo, _)| repo.file_name().unwrap_or_default().to_string_lossy().to_string())
                    .unwrap_or_else(|| "Projekt".to_string());
                let commitlist = commits.get(*selected_repo_index)
                    .map(|(_repo, msgs)| msgs.join("\n"))
                    .unwrap_or_default();
                (project, commitlist)
            };
            // Zeitfenster als String
            let interval_str = intervals[*current_index].0;
            // Prompt bauen
            let prompt = format!(
                "{template}\n\nProject: {project}\nTimeframe: {interval}\nCommits:\n{commits}",
                template=prompt_template,
                project=project_name,
                interval=interval_str,
                commits=commit_str
            );
            { let mut p = popup_quote.lock().unwrap(); p.visible=true; p.loading=true; p.text="Loading commit summary...".into(); }
            let p2 = popup_quote.clone();
            rt.spawn(async move {
                let summary = fetch_gemini_commit_summary(&prompt).await.unwrap_or_else(|e| format!("Error: {}", e));
                let mut p = p2.lock().unwrap(); p.text=summary; p.loading=false;
            });
        }
        KeyCode::Char('A') => {
            // Send only marked commits to Gemini
            let sel = selected_commits.lock().unwrap();
            if sel.set.is_empty() {
                let mut p = popup_quote.lock().unwrap();
                p.visible = true;
                p.loading = false;
                p.text = "No commits marked.".to_string();
            } else {
                // Build hash -> commit line map
                let mut hash_to_commit = std::collections::HashMap::new();
                for (_repo, commits) in commits.iter() {
                    for commit in commits {
                        if let Some(hash) = commit.split_whitespace().next() {
                            hash_to_commit.insert(hash, commit);
                        }
                    }
                }
                // Collect full commit lines for selected hashes
                let commit_lines: Vec<String> = sel.set.iter()
                    .filter_map(|hash| hash_to_commit.get(hash.as_str()).map(|s| s.to_string()))
                    .collect();
                let commit_str = commit_lines.join("\n");
                // Use current project name and interval
                let project_name = if *selected_repo_index == usize::MAX {
                    "All projects".to_string()
                } else {
                    commits.get(*selected_repo_index)
                        .map(|(repo, _)| repo.file_name().unwrap_or_default().to_string_lossy().to_string())
                        .unwrap_or_else(|| "Project".to_string())
                };
                let interval_str = intervals[*current_index].0;
                let prompt_template = fs::read_to_string("prompt.txt").unwrap_or_default();
                let prompt = format!(
                    "{template}\n\nProject: {project}\nTimeframe: {interval}\nCommits:\n{commits}",
                    template=prompt_template,
                    project=project_name,
                    interval=interval_str,
                    commits=commit_str
                );
                { let mut p = popup_quote.lock().unwrap(); p.visible=true; p.loading=true; p.text="Loading commit summary...".into(); }
                let p2 = popup_quote.clone();
                rt.spawn(async move {
                    let summary = fetch_gemini_commit_summary(&prompt).await.unwrap_or_else(|e| format!("Error: {}", e));
                    let mut p = p2.lock().unwrap(); p.text=summary; p.loading=false;
                });
            }
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
            let mut p = popup_quote.lock().unwrap(); p.visible=false; 
            let mut sel = selected_commits.lock().unwrap(); sel.popup_visible = false;
        }
        _ => {}
    }
    *current_interval = intervals[*current_index].1;
    Ok(true)
}

pub fn handle_mouse(
    mouse_event: MouseEvent,
    repos: &Vec<PathBuf>,
    commits: &Vec<(PathBuf, Vec<String>)>,
    selected_repo_index: &mut usize,
    selected_commit_index: &mut Option<usize>,
    focus: &mut FocusArea,
    sidebar_scroll: &mut usize,
    commitlist_scroll: &mut usize,
    show_details: &mut bool,
    popup_quote: &Arc<Mutex<PopupQuote>>,
    selected_commits: &Arc<Mutex<SelectedCommits>>,
    sidebar_area: ratatui::prelude::Rect,
) {
    use crate::network::fetch_gemini_commit_summary;
    use std::thread;
    use tokio::runtime::Runtime;
    use std::fs;
    if let MouseEventKind::Down(_) = mouse_event.kind {
        let x = mouse_event.column as u16;
        let y = mouse_event.row as u16;
        // Sidebar area: x < sidebar_area.x + sidebar_area.width
        if x >= sidebar_area.x && x < sidebar_area.x + sidebar_area.width && y >= sidebar_area.y && y < sidebar_area.y + sidebar_area.height {
            // Button box is last 3 lines of sidebar_area
            let button_box_start = sidebar_area.y + sidebar_area.height - 3;
            if y == button_box_start + 1 {
                // Bookmarks button
                let mut sel = selected_commits.lock().unwrap();
                sel.popup_visible = true;
                *focus = FocusArea::Sidebar;
                return;
            } else if y == button_box_start + 2 {
                // AI Summary button
                let prompt_template = fs::read_to_string("prompt.txt").unwrap_or_default();
                let (project_name, commit_str) = if *selected_repo_index == usize::MAX {
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
                };
                let interval_label = "";
                let prompt = format!(
                    "{template}\n\nProject: {project}\nTimeframe: {interval}\nCommits:\n{commits}",
                    template=prompt_template,
                    project=project_name,
                    interval=interval_label,
                    commits=commit_str
                );
                { let mut p = popup_quote.lock().unwrap(); p.visible=true; p.loading=true; p.text="Loading commit summary...".into(); }
                let popup_quote = popup_quote.clone();
                thread::spawn(move || {
                    let rt = Runtime::new().unwrap();
                    rt.block_on(async move {
                        let summary = fetch_gemini_commit_summary(&prompt).await.unwrap_or_else(|e| format!("Error: {}", e));
                        let mut p = popup_quote.lock().unwrap(); p.text=summary; p.loading=false;
                    });
                });
                *focus = FocusArea::Sidebar;
                return;
            }
            // Sidebar repo selection
            let idx = (y as usize).saturating_sub(1 + *sidebar_scroll);
            if idx == 0 {
                *selected_repo_index = usize::MAX;
            } else if idx > 0 && idx <= repos.len() {
                *selected_repo_index = idx - 1;
            }
            *selected_commit_index = None;
            return;
        } else {
            // Commit list area
            *focus = FocusArea::CommitList;
            // Estimate which commit was clicked
            let commit_y = y.saturating_sub(1); // account for border
            let mut idx = commit_y as usize + *commitlist_scroll;
            if *selected_repo_index == usize::MAX {
                // All: need to skip repo headers
                let mut offset = 0;
                for (_repo, repo_commits) in commits {
                    if idx == 0 {
                        // header line, do nothing
                        return;
                    }
                    idx -= 1;
                    if idx < repo_commits.len() {
                        *selected_commit_index = Some(offset + idx);
                        // Mark/unmark on click
                        let mut sel = selected_commits.lock().unwrap();
                        let commit = &repo_commits[idx];
                        let hash = commit.split_whitespace().next().unwrap_or("").to_string();
                        if sel.set.contains(&hash) {
                            sel.set.remove(&hash);
                        } else {
                            sel.set.insert(hash);
                        }
                        return;
                    }
                    offset += repo_commits.len();
                    idx -= repo_commits.len();
                }
            } else if let Some((_repo, repo_commits)) = commits.get(*selected_repo_index) {
                if idx < repo_commits.len() {
                    *selected_commit_index = Some(idx);
                    // Mark/unmark on click
                    let mut sel = selected_commits.lock().unwrap();
                    let commit = &repo_commits[idx];
                    let hash = commit.split_whitespace().next().unwrap_or("").to_string();
                    if sel.set.contains(&hash) {
                        sel.set.remove(&hash);
                    } else {
                        sel.set.insert(hash);
                    }
                }
            }
        }
    }
}