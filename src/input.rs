use std::{sync::{Arc, Mutex}, time::Duration, path::PathBuf, process::Command};
use crossterm::event::KeyCode;
use tokio::runtime::Runtime;
use crate::models::FocusArea;
use crate::models::PopupQuote;
use crate::git::{reload_commits, get_current_git_user, get_commit_details};
use crate::utils::{get_active_commits, get_sidebar_height, get_commitlist_height, get_commitlist_visible_and_total, calculate_max_detail_scroll};
use crate::network::{fetch_quote, fetch_gemini_startrek_quote};
use anyhow::Result;

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
    rt: &Runtime,
) -> Result<bool> {
    match key {
        KeyCode::Char('1') => *current_index = 0,
        KeyCode::Char('2') => *current_index = 1,
        KeyCode::Char('3') => *current_index = 2,
        KeyCode::Char('w') => *current_index = 3,
        KeyCode::Char('m') => *current_index = 4,
        KeyCode::Left => {
            if *current_index > 0 { *current_index -= 1; }
        }
        KeyCode::Right => {
            if *current_index < intervals.len() - 1 { *current_index += 1; }
        }
        KeyCode::Tab => {
            *focus = match *focus {
                FocusArea::Sidebar => {
                    if selected_commit_index.is_none() {
                        if *selected_repo_index == usize::MAX {
                            let total: usize = commits.iter().map(|(_,c)|c.len()).sum();
                            if total > 0 { *selected_commit_index = Some(0); }
                        } else if let Some(repo_commits) = get_active_commits(commits, *selected_repo_index) {
                            if !repo_commits.is_empty() { *selected_commit_index = Some(0); }
                        }
                    }
                    FocusArea::CommitList
                }
                FocusArea::CommitList => {
                    if *show_details { FocusArea::Detail } else { FocusArea::Sidebar }
                }
                FocusArea::Detail => FocusArea::Sidebar,
            };
        }
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
        KeyCode::Char('z') => {
            { let mut p = popup_quote.lock().unwrap(); p.visible=true; p.loading=true; p.text="Lade Zitat...".into(); }
            let p2 = popup_quote.clone();
            rt.spawn(async move {
                let quote = fetch_quote().await.unwrap_or_else(|e| format!("Fehler: {}", e));
                let mut p = p2.lock().unwrap(); p.text=quote; p.loading=false;
            });
        }
        KeyCode::Char('a') => {
            { let mut p = popup_quote.lock().unwrap(); p.visible=true; p.loading=true; p.text="Lade Star Trek Zitat...".into(); }
            let p2 = popup_quote.clone();
            rt.spawn(async move {
                let quote = fetch_gemini_startrek_quote().await.unwrap_or_else(|e| format!("Fehler: {}", e));
                let mut p = p2.lock().unwrap(); p.text=quote; p.loading=false;
            });
        }
        KeyCode::Esc => { let mut p = popup_quote.lock().unwrap(); p.visible=false; }
        _ => {}
    }
    *current_interval = intervals[*current_index].1;
    Ok(true)
}