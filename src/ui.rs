use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap, ListState, Clear},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::models::{FocusArea, PopupQuote};
use crate::git::get_commit_details;

pub fn render_commits(
    f: &mut Frame,
    _repos: &Vec<PathBuf>,
    selected_repo_index: usize,
    data: &Vec<(PathBuf, Vec<String>)>,
    interval_label: &str,
    selected_commit_index: Option<usize>,
    show_details: bool,
    focus: FocusArea,
    _sidebar_scroll: usize,
    _commitlist_scroll: usize,
    detail_scroll: u16,
    filter_by_user: bool,
    popup_quote: Option<&Arc<Mutex<PopupQuote>>>,
) {
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

    // Sidebar
    let filtered_repos: Vec<&PathBuf> = data.iter().map(|(repo,_)| repo).collect();
    let mut repo_list = vec![ListItem::new(format!("{} All", if selected_repo_index==usize::MAX {"→"}else{" "}))
        .style(if selected_repo_index==usize::MAX {Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)} else {Style::default().fg(Color::White)})];
    repo_list.extend(filtered_repos.iter().enumerate().map(|(i,repo)|{
        let name = repo.file_name().unwrap().to_string_lossy();
        let selected = selected_repo_index==i;
        let style = if selected {Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)} else {Style::default().fg(Color::White)};
        ListItem::new(format!("{} {}", if selected{'→'} else {' '}, name)).style(style)
    }));
    let sidebar = List::new(repo_list);
    let mut sidebar_state = ListState::default();
    sidebar_state.select(Some(if selected_repo_index==usize::MAX {0} else {selected_repo_index+1}));
    let sidebar_block = Block::default().title("Repositories").borders(Borders::ALL)
        .style(if focus==FocusArea::Sidebar {Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)} else {Style::default().fg(Color::Cyan)});
    f.render_stateful_widget(sidebar.block(sidebar_block), sidebar_area, &mut sidebar_state);

    // Commit list layout with scrollbar
    let commit_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
        .split(commit_area);

    // Header
    let header = if selected_repo_index==usize::MAX {
        if filter_by_user { format!("Standup Commits (only mine) – {}", interval_label) }
        else { format!("Standup Commits – {}", interval_label) }
    } else if let Some((repo,_)) = data.get(selected_repo_index) {
        let name = repo.file_name().unwrap().to_string_lossy();
        if filter_by_user { format!("{} (only mine) – {}", name, interval_label)} else {format!("{} – {}", name, interval_label)}
    } else { format!("Standup Commits – {}", interval_label) };

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
                let indicator = if sel {"→"} else {"  "};
                let style = if sel {Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)} else {Style::default()};
                items.push(ListItem::new(format!("{} {}", indicator, commit)).style(style));
            }
            offset+=commits.len();
        }
        let mut state = ListState::default(); state.select(selected_commit_index);
        let list = List::new(items).block(Block::default().title(header).borders(Borders::ALL)
            .style(if focus==FocusArea::CommitList {Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)} else {Style::default().fg(Color::Cyan)}));
        f.render_stateful_widget(list, commit_layout[0], &mut state);
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
                let indicator=if sel{"→"} else {"  "};
                let style=if sel {Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)} else {Style::default()};
                ListItem::new(format!("{} {}", indicator, commit)).style(style)
            }).collect();
            let mut state=ListState::default(); state.select(selected_commit_index);
            let list = List::new(items).block(Block::default().title(header).borders(Borders::ALL)
                .style(if focus==FocusArea::CommitList {Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)} else {Style::default().fg(Color::Cyan)}));
            f.render_stateful_widget(list, commit_layout[0], &mut state);
            // scrollbar
            let total=commits.len();
            let visible=commit_area.height.saturating_sub(2) as usize;
            let pos=selected_commit_index.unwrap_or(0).saturating_sub(visible.saturating_sub(visible));
            let mut sb=ScrollbarState::default().position(pos).content_length(total);
            f.render_stateful_widget(Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight), commit_layout[1], &mut sb);
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
    let footer = Paragraph::new(format!("Keys: ←/→ Timeframe | ↑/↓ Navigation/Scroll | Tab Focus | Space Details | {} | z: Quote | c: Copy | q Quit", filter_label))
        .block(Block::default().borders(Borders::ALL)).style(Style::default().fg(Color::Gray).add_modifier(Modifier::DIM));
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
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Percentage((100-percent_y)/2), Constraint::Percentage(percent_y), Constraint::Percentage((100-percent_y)/2)]).split(r)[1];
    Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage((100-percent_x)/2), Constraint::Percentage(percent_x), Constraint::Percentage((100-percent_x)/2)]).split(vertical)[1]
}