use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap, ListState, Clear},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Line},
    symbols,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::models::{FocusArea, PopupQuote};
use crate::git::get_commit_details;
use crate::models::SelectedCommits;
use crate::CommitTab;
use once_cell::sync::Lazy;
use regex::Regex;

// Type alias for commit data for clarity
pub type CommitData = Vec<(PathBuf, Vec<String>)>;

// Compile the ticket regex once for all uses
static TICKET_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"[A-Z]+-\d+").unwrap());

/// Renders a commit line with syntax highlighting and ticket detection.
fn render_commit_line(commit: &str, indicator: String, _selected: bool) -> Line<'static> {
    let mut spans = vec![];
    let mut parts = commit.split_whitespace();
    if let Some(hash) = parts.next() {
        spans.push(Span::styled(hash.to_owned(), Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)));
    }
    if let Some(second) = parts.next() {
        spans.push(Span::raw(format!(" {}", second)));
    }
    let rest: String = parts.collect::<Vec<_>>().join(" ");
    let mut last = 0;
    for m in TICKET_REGEX.find_iter(&rest) {
        if m.start() > last {
            spans.push(Span::raw(rest[last..m.start()].to_owned()));
        }
        spans.push(Span::styled(rest[m.start()..m.end()].to_owned(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        last = m.end();
    }
    if last < rest.len() {
        spans.push(Span::raw(rest[last..].to_owned()));
    }
    let mut content = vec![Span::raw(indicator), Span::raw(" ")];
    content.extend(spans);
    Line::from(content)
}

/// Renders the commits view.
pub fn render_commits(
    f: &mut Frame,
    _repos: &Vec<PathBuf>,
    selected_repo_index: usize,
    data: &CommitData,
    interval_label: &str,
    selected_commit_index: Option<usize>,
    show_details: bool,
    focus: FocusArea,
    _sidebar_scroll: usize,
    _commitlist_scroll: usize,
    detail_scroll: u16,
    filter_by_user: bool,
    popup_quote: Option<&Arc<Mutex<PopupQuote>>>,
    selected_commits: Option<&Arc<Mutex<SelectedCommits>>>,
    selected_tab: CommitTab,
) {
    let selected_set = selected_commits.map(|arc| arc.lock().unwrap().set.clone()).unwrap_or_default();
    let area = f.area();
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)]).split(area);

    // Main layout: sidebar, commits, optional detail
    let columns = if show_details && selected_commit_index.is_some() {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(30),     // sidebar
                Constraint::Percentage(60),   // commit list
                Constraint::Percentage(40),   // detail view
            ])
            .split(vertical_chunks[0])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(30),     // sidebar
                Constraint::Min(1),         // commit list only
            ])
            .split(vertical_chunks[0])
    };
    // Assign areas
    let sidebar_area = columns[0];
    let commit_area = columns[1];
    let detail_area = if columns.len() > 2 { Some(columns[2]) } else { None };

    // Split sidebar area into sidebar and button box
    let sidebar_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(2), // sidebar list
            // Removed button box area
        ])
        .split(sidebar_area);

    // Sidebar list (only repos with commits in the current timeframe)
    let filtered_repos: Vec<&PathBuf> = data.iter().map(|(repo,_)| repo).collect();
    let mut repo_list = Vec::new();
    // 'All' entry
    let all_selected = selected_repo_index == usize::MAX;
    let all_style = if all_selected {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    repo_list.push(ListItem::new(vec![
        Line::from(vec![Span::styled("All", all_style)]),
        Line::from(vec![Span::styled(format!("{} projects", filtered_repos.len()), Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC))]),
    ]));
    // Visual divider
    repo_list.push(ListItem::new(Line::from(vec![Span::styled("────────────", Style::default().fg(Color::DarkGray))])));
    // Per-repo entries (only those with commits)
    if filtered_repos.is_empty() {
        repo_list.push(ListItem::new(Line::from(vec![Span::styled(
            "No projects found. Try another timeframe with <Tab>",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        )])));
    } else {
        for (i, repo) in filtered_repos.iter().enumerate() {
            let name = repo.file_name().unwrap_or_default().to_string_lossy();
            let selected = selected_repo_index == i;
            let style = if selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            // Find commit count for this repo
            let count = data.iter().find(|(r,_)| r == *repo).map(|(_,c)| c.len()).unwrap_or(0);
            let count_style = if count > 0 {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)
            };
            repo_list.push(ListItem::new(vec![
                Line::from(vec![Span::styled(name, style)]),
                Line::from(vec![Span::styled(format!("{} commit{}", count, if count == 1 { "" } else { "s" }), count_style)]),
            ]));
        }
    }
    let sidebar = List::new(repo_list).highlight_symbol("→");
    let mut sidebar_state = ListState::default();
    sidebar_state.select(Some(if selected_repo_index==usize::MAX {0} else {selected_repo_index+2}));
    let sidebar_block = Block::default().title("Repositories [1]").borders(Borders::ALL)
        .style(if focus==FocusArea::Sidebar {Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)} else {Style::default().fg(Color::Cyan)});
    f.render_stateful_widget(sidebar.block(sidebar_block), sidebar_chunks[0], &mut sidebar_state);

    // Commit list layout with scrollbar
    let commit_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
        .split(commit_area);

    // Tabs for commit list (refactored)
    let tab_titles = ["Timeframe [2]", "Selection [3]"];
    let tabs = ratatui::widgets::Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::ALL).title("Select View"))
        .style(Style::default().white())
        .highlight_style(Style::default().yellow().bold().underlined())
        .select(selected_tab.as_index())
        .divider(symbols::DOT)
        .padding(" ", " ");
    let tabs_area = Rect {
        x: commit_area.x,
        y: commit_area.y,
        width: commit_area.width,
        height: 3,
    };
    f.render_widget(tabs, tabs_area);

    let list_area = Rect {
        x: commit_area.x,
        y: commit_area.y + 3,
        width: commit_area.width,
        height: commit_area.height.saturating_sub(3),
    };

    // Header
    let header = if selected_repo_index==usize::MAX {
        if filter_by_user { format!("Standup Commits (only mine) – {}", interval_label) }
        else { format!("Standup Commits – {}", interval_label) }
    } else if let Some((repo,_)) = data.get(selected_repo_index) {
        let name = repo.file_name().unwrap().to_string_lossy();
        if filter_by_user { format!("{} (only mine) – {}", name, interval_label)} else {format!("{} – {}", name, interval_label)}
    } else { format!("Standup Commits – {}", interval_label) };

    // Render commit list depending on active tab
    match selected_tab {
        CommitTab::Timeframe => {
            // Flatten commits for 'All'
            if selected_repo_index==usize::MAX {
                let mut items = Vec::new();
                let mut offset=0;
                for (repo, commits) in data {
                    items.push(ListItem::new(format!("### {}", repo.file_name().unwrap().to_string_lossy()))
                        .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)));
                    for (i,commit) in commits.iter().enumerate() {
                        let idx=offset+i;
                        let sel = Some(idx)==selected_commit_index;
                        let star = if let Some(hash) = commit.split_whitespace().next() { if selected_set.contains(hash) {"*"} else {" "} } else {" "};
                        let indicator = format!("{}{}", star, if sel {"→"} else {"  " });
                        let style = if sel {Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)} else {Style::default()};
                        let line = render_commit_line(commit, indicator, sel);
                        items.push(ListItem::new(line).style(style));
                    }
                    offset+=commits.len();
                }
                let mut state = ListState::default(); state.select(selected_commit_index);
                let list = List::new(items).block(Block::default().title(header).borders(Borders::ALL)
                    .style(if focus==FocusArea::CommitList {Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)} else {Style::default().fg(Color::Cyan)}));
                f.render_stateful_widget(list, list_area, &mut state);
                // scrollbar
                let total: usize = data.iter().map(|(_,c)|c.len()).sum();
                let visible = commit_area.height.saturating_sub(2) as usize;
                let pos = selected_commit_index.unwrap_or(0).saturating_sub(visible.saturating_sub(visible));
                let mut sb = ScrollbarState::default().position(pos).content_length(total);
                f.render_stateful_widget(Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight), commit_layout[1], &mut sb);
            } else {
                if let Some((_, commits)) = data.get(selected_repo_index) {
                    let items: Vec<ListItem> = commits.iter().enumerate().map(|(i,commit)|{
                        let sel=Some(i)==selected_commit_index;
                        let star = if let Some(hash) = commit.split_whitespace().next() { if selected_set.contains(hash) {"*"} else {" "} } else {" "};
                        let indicator = format!("{}{}", star, if sel {"→"} else {"  " });
                        let style=if sel {Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)} else {Style::default()};
                        let line = render_commit_line(commit, indicator, sel);
                        ListItem::new(line).style(style)
                    }).collect();
                    let mut state=ListState::default(); state.select(selected_commit_index);
                    let list = List::new(items).block(Block::default().title(header).borders(Borders::ALL)
                        .style(if focus==FocusArea::CommitList {Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)} else {Style::default().fg(Color::Cyan)}));
                    f.render_stateful_widget(list, list_area, &mut state);
                    // scrollbar
                    let total=commits.len();
                    let visible=commit_area.height.saturating_sub(2) as usize;
                    let pos=selected_commit_index.unwrap_or(0).saturating_sub(visible.saturating_sub(visible));
                    let mut sb=ScrollbarState::default().position(pos).content_length(total);
                    f.render_stateful_widget(Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight), commit_layout[1], &mut sb);
                }
            }
        },
        CommitTab::Selection => {
            if let Some(selected_commits) = selected_commits {
                let sel = selected_commits.lock().unwrap();
                if sel.set.is_empty() {
                    let placeholder = Paragraph::new("No commits selected. Press 'm' to add commits to your selection.")
                        .block(Block::default().title("Selected Commits").borders(Borders::ALL))
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(Color::DarkGray));
                    f.render_widget(placeholder, list_area);
                } else {
                    // Map: repo_path -> Vec<commit>
                    let mut repo_to_commits: std::collections::BTreeMap<&PathBuf, Vec<&String>> = std::collections::BTreeMap::new();
                    let mut hash_to_repo: std::collections::HashMap<&str, &PathBuf> = std::collections::HashMap::new();
                    for (repo, commits) in data {
                        for commit in commits {
                            if let Some(hash) = commit.split_whitespace().next() {
                                hash_to_repo.insert(hash, repo);
                                if sel.set.contains(hash) {
                                    repo_to_commits.entry(repo).or_default().push(commit);
                                }
                            }
                        }
                    }
                    let mut items = Vec::new();
                    for (repo, commits) in repo_to_commits.iter() {
                        items.push(ListItem::new(format!("### {}", repo.file_name().unwrap().to_string_lossy()))
                            .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)));
                        for commit in commits.iter() {
                            let star = if let Some(hash) = commit.split_whitespace().next() { if sel.set.contains(hash) {"*"} else {" "} } else {" "};
                            let indicator = format!("{}  ", star);
                            let style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
                            let line = render_commit_line(commit, indicator, true);
                            items.push(ListItem::new(line).style(style));
                        }
                    }
                    let mut state = ListState::default(); state.select(selected_commit_index);
                    let list = List::new(items).block(Block::default().title("Selected Commits").borders(Borders::ALL)
                        .style(if focus==FocusArea::CommitList {Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)} else {Style::default().fg(Color::Cyan)}));
                    f.render_stateful_widget(list, list_area, &mut state);
                }
            }
        }
    }

    // Unified detail view rendering on the right when toggled
    if let Some(detail_chunk) = detail_area {
        if show_details {
            if let Some(sel_idx) = selected_commit_index {
                // Determine repo and commit line: global vs per-repo
                let (repo_path, commit_line) = {
                    if selected_repo_index == usize::MAX {
                        // 'All' view: find mapping by global index
                        let mut offset = 0;
                        let mut found: Option<(PathBuf, String)> = None;
                        for (repo, repo_commits) in data {
                            if sel_idx < offset + repo_commits.len() {
                                found = Some((repo.clone(), repo_commits[sel_idx - offset].clone()));
                                break;
                            }
                            offset += repo_commits.len();
                        }
                        found.unwrap_or_else(|| {
                            // fallback to first available commit
                            if let Some((r, commits_vec)) = data.first() {
                                (r.clone(), commits_vec.first().cloned().unwrap_or_default())
                            } else {
                                (PathBuf::new(), String::new())
                            }
                        })
                    } else {
                        let (r, commits_vec) = &data[selected_repo_index];
                        (r.clone(), commits_vec[sel_idx].clone())
                    }
                };
                let hash = commit_line.split_whitespace().next().unwrap_or("");
                let details = get_commit_details(&repo_path, hash).unwrap_or_else(|e| e.to_string());
                // clear detail region
                f.render_widget(Clear, detail_chunk);
                // draw border around detail
                let detail_block = Block::default()
                    .title("Details")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Magenta));
                f.render_widget(detail_block, detail_chunk);
                // define padded inner area
                let padded = Rect {
                    x: detail_chunk.x + 1,
                    y: detail_chunk.y + 1,
                    width: detail_chunk.width.saturating_sub(2),
                    height: detail_chunk.height.saturating_sub(2),
                };
                // clear inner region too
                f.render_widget(Clear, padded);
                // fill padded area with spaces to erase any leftover text
                let blank_lines = vec![" ".repeat(padded.width as usize); padded.height as usize].join("\n");
                let blank_para = Paragraph::new(blank_lines.clone());
                f.render_widget(blank_para, padded);
                // split into text + scrollbar
                let detail_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
                    .split(padded);
                // render detail text
                let para = Paragraph::new(details.clone())
                    .wrap(Wrap { trim: false })
                    .scroll((detail_scroll, 0));
                f.render_widget(para, detail_chunks[0]);
                // render scrollbar
                let lines = details.lines().count();
                let mut ds = ScrollbarState::default()
                    .position(detail_scroll as usize)
                    .content_length(lines);
                f.render_stateful_widget(
                    Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
                    detail_chunks[1],
                    &mut ds,
                );
            }
        }
    } 

    // footer
    let filter_label = if filter_by_user {"u: Only mine"} else {"u: All"};
    let footer = Paragraph::new(format!(
        "Tab/Shift+Tab Timeframe | ↑/↓/ or h/j/k/l Navigation | <Space> Details | m Mark | s Show Marked | a AI summary | A AI summary (marked) | {} | Q Quit",
        filter_label
    ))
    .block(Block::default().borders(Borders::ALL))
    .style(Style::default().fg(Color::Gray).add_modifier(Modifier::DIM));
    f.render_widget(footer, vertical_chunks[1]);

    // popup
    if let Some(arc) = popup_quote {
        let popup = arc.lock().unwrap();
        if popup.visible {
            // Popup larger
            let popup_area = centered_rect(60,80,f.area());
            f.render_widget(Clear, popup_area);
            // Dynamic title
            let project = if selected_repo_index==usize::MAX {
                "All projects".to_string()
            } else if let Some((repo,_)) = data.get(selected_repo_index) {
                repo.file_name().unwrap_or_default().to_string_lossy().to_string()
            } else {
                "Project".to_string()
            };
            let title = format!("Summary for {} of the last {}", project, interval_label);
            let block = Block::default().title(title).borders(Borders::ALL).style(Style::default().fg(Color::Magenta).bg(Color::Black));
            let para = Paragraph::new(popup.text.clone())
                .block(block)
                .wrap(Wrap{trim:true})
                .alignment(Alignment::Left)
                .style(Style::default().fg(Color::White));
            f.render_widget(para, popup_area);
            // Footer below the popup
            let footer_area = Rect {
                x: popup_area.x,
                y: popup_area.y + popup_area.height,
                width: popup_area.width,
                height: 1,
            };
            let footer = Paragraph::new("Press c to copy to clipboard").style(Style::default().fg(Color::Gray).add_modifier(Modifier::DIM));
            f.render_widget(footer, footer_area);
        }
    }

    if let Some(selected_commits) = selected_commits {
        let sel = selected_commits.lock().unwrap();
        if sel.popup_visible {
            let popup_area = centered_rect(60, 40, f.area());
            f.render_widget(Clear, popup_area);
            let mut lines = vec![Line::from("Selected Commits:")];
            // Build a map of hash -> full commit line for lookup
            let mut hash_to_commit = std::collections::HashMap::new();
            for (_repo, commits) in data {
                for commit in commits {
                    if let Some(hash) = commit.split_whitespace().next() {
                        hash_to_commit.insert(hash, commit);
                    }
                }
            }
            for hash in &sel.set {
                if let Some(commit_line) = hash_to_commit.get(hash.as_str()) {
                    lines.push(Line::from((*commit_line).to_string()));
                } else {
                    lines.push(Line::from(hash.clone()));
                }
            }
            let para = Paragraph::new(lines)
                .block(Block::default().title("Selected Commits").borders(Borders::ALL).style(Style::default().fg(Color::Magenta).bg(Color::Black)))
                .wrap(Wrap{trim:true})
                .alignment(Alignment::Left)
                .style(Style::default().fg(Color::White));
            f.render_widget(para, popup_area);
        }
    }
}

/// Centers a rectangle within another rectangle.
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Percentage((100-percent_y)/2), Constraint::Percentage(percent_y), Constraint::Percentage((100-percent_y)/2)]).split(r)[1];
    Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage((100-percent_x)/2), Constraint::Percentage(percent_x), Constraint::Percentage((100-percent_x)/2)]).split(vertical)[1]
}