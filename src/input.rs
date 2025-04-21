use std::{sync::{Arc, Mutex}, time::Duration, path::PathBuf, fs};
use crossterm::event::KeyCode;
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
            *current_index = 0;
            *current_interval = intervals[*current_index].1;
            *commits = reload_commits(repos, *current_interval, *filter_by_user)?;
            *selected_commit_index = None;
        },
        KeyCode::Char('2') => {
            *current_index = 1;
            *current_interval = intervals[*current_index].1;
            *commits = reload_commits(repos, *current_interval, *filter_by_user)?;
            *selected_commit_index = None;
        },
        KeyCode::Char('3') => {
            *current_index = 2;
            *current_interval = intervals[*current_index].1;
            *commits = reload_commits(repos, *current_interval, *filter_by_user)?;
            *selected_commit_index = None;
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
            let mut sel = selected_commits.lock().unwrap();
            sel.popup_visible = true;
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
        KeyCode::Up => {
            match *focus {
                FocusArea::Sidebar => {
                    let repo_count = commits.len();
                    if *selected_repo_index == usize::MAX {
                        if repo_count>0 { *selected_repo_index = repo_count-1; }
                    } else if *selected_repo_index >0 { *selected_repo_index -=1; } else { *selected_repo_index = usize::MAX; }
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
        KeyCode::Down => {
            match *focus {
                FocusArea::Sidebar => {
                    let count = commits.len();
                    if *selected_repo_index==usize::MAX { *selected_repo_index=0; }
                    else if *selected_repo_index < count-1 { *selected_repo_index+=1; }
                    else { *selected_repo_index=usize::MAX; }
                    *selected_commit_index=None;
                    let height = get_sidebar_height()?;
                    if *selected_repo_index==usize::MAX { *sidebar_scroll=0; }
                    else if *selected_repo_index>= *sidebar_scroll+height { *sidebar_scroll = *selected_repo_index+1-height; }
                }
                FocusArea::CommitList => {
                    if *selected_repo_index==usize::MAX {
                        let total: usize = commits.iter().map(|(_,c)|c.len()).sum();
                        if total==0 { *selected_commit_index=None; }
                        else if let Some(idx)=*selected_commit_index {
                            if idx<total-1 { *selected_commit_index = Some(idx+1);} }
                        else {*selected_commit_index=Some(0);}    
                    } else if let Some(repo_commits)= get_active_commits(commits,*selected_repo_index) {
                        if let Some(idx)=*selected_commit_index {
                            if idx<repo_commits.len()-1 {*selected_commit_index=Some(idx+1);} }
                        else {*selected_commit_index=Some(0);}    
                    }
                    let height = get_commitlist_height()?;
                    if let Some(idx)=*selected_commit_index {
                        if idx>= *commitlist_scroll + height { *commitlist_scroll = idx+1-height; }
                    }
                }
                FocusArea::Detail => {
                    if let Some(idx)=*selected_commit_index {
                        let max = calculate_max_detail_scroll(commits,*selected_repo_index,idx)?;
                        if *detail_scroll<max { *detail_scroll+=1; }
                    }
                }
            }
        }
        KeyCode::Char('k') => {
            // vim 'k' as Up arrow
            match *focus {
                FocusArea::Sidebar => {
                    let repo_count = commits.len();
                    if *selected_repo_index == usize::MAX {
                        if repo_count>0 { *selected_repo_index = repo_count-1; }
                    } else if *selected_repo_index >0 { *selected_repo_index -=1; } else { *selected_repo_index = usize::MAX; }
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
        KeyCode::Char('j') => {
            // vim 'j' as Down arrow
            match *focus {
                FocusArea::Sidebar => {
                    let count = commits.len();
                    if *selected_repo_index==usize::MAX { *selected_repo_index=0; }
                    else if *selected_repo_index < count-1 { *selected_repo_index+=1; }
                    else { *selected_repo_index=usize::MAX; }
                    *selected_commit_index=None;
                    let height = get_sidebar_height()?;
                    if *selected_repo_index==usize::MAX { *sidebar_scroll=0; }
                    else if *selected_repo_index>= *sidebar_scroll+height { *sidebar_scroll = *selected_repo_index+1-height; }
                }
                FocusArea::CommitList => {
                    if *selected_repo_index==usize::MAX {
                        let total: usize = commits.iter().map(|(_,c)|c.len()).sum();
                        if total==0 { *selected_commit_index=None; }
                        else if let Some(idx)=*selected_commit_index {
                            if idx<total-1 { *selected_commit_index = Some(idx+1);} }
                        else {*selected_commit_index=Some(0);}    
                    } else if let Some(repo_commits)= get_active_commits(commits,*selected_repo_index) {
                        if let Some(idx)=*selected_commit_index {
                            if idx<repo_commits.len()-1 {*selected_commit_index=Some(idx+1);} }
                        else {*selected_commit_index=Some(0);}    
                    }
                    let height = get_commitlist_height()?;
                    if let Some(idx)=*selected_commit_index {
                        if idx>= *commitlist_scroll + height { *commitlist_scroll = idx+1-height; }
                    }
                }
                FocusArea::Detail => {
                    if let Some(idx)=*selected_commit_index {
                        let max = calculate_max_detail_scroll(commits,*selected_repo_index,idx)?;
                        if *detail_scroll<max { *detail_scroll+=1; }
                    }
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