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
    repos: &Vec<PathBuf>,
    selected_repo_index: usize,
    data: &Vec<(PathBuf, Vec<String>)>,
    interval_label: &str,
    selected_commit_index: Option<usize>,
    show_details: bool,
    focus: FocusArea,
    sidebar_scroll: usize,
    mut commitlist_scroll: usize,
    detail_scroll: u16,
    filter_by_user: bool,
    popup_quote: Option<&Arc<Mutex<PopupQuote>>>,
) {
    let area = f.area();
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)]).split(area);
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(1)]).split(vertical_chunks[0]);

    // Sidebar
    let filtered_repos: Vec<&PathBuf> = data.iter().map(|(repo,_)| repo).collect();
    let mut repo_list = vec![ListItem::new(format!("{} Alle", if selected_repo_index==usize::MAX {"→"}else{" "}))
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
    f.render_stateful_widget(sidebar.block(sidebar_block), main_chunks[0], &mut sidebar_state);

    // Commit list + detail layout
    let commit_and_detail = if show_details && selected_commit_index.is_some() {
        Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(0), Constraint::Length(15)]).split(main_chunks[1])
    } else {
        Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(0)]).split(main_chunks[1])
    };
    let commit_layout = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Min(1), Constraint::Length(1)]).split(commit_and_detail[0]);

    // Header
    let header = if selected_repo_index==usize::MAX {
        if filter_by_user { format!("Standup Commits (nur eigene) – {}", interval_label) }
        else { format!("Standup Commits – {}", interval_label) }
    } else if let Some((repo,_)) = data.get(selected_repo_index) {
        let name = repo.file_name().unwrap().to_string_lossy();
        if filter_by_user { format!("{} (nur eigene) – {}", name, interval_label)} else {format!("{} – {}", name, interval_label)}
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
        let visible = commit_and_detail[0].height.saturating_sub(2) as usize;
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
            let visible=commit_and_detail[0].height.saturating_sub(2) as usize;
            let pos=selected_commit_index.unwrap_or(0).saturating_sub(visible.saturating_sub(visible));
            let mut sb=ScrollbarState::default().position(pos).content_length(total);
            f.render_stateful_widget(Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight), commit_layout[1], &mut sb);

            // detail
            if show_details {
                if let Some(idx) = selected_commit_index {
                    if let Some(commit) = commits.get(idx) {
                        let hash = commit.split_whitespace().next().unwrap_or("");
                        let details = get_commit_details(&repos[selected_repo_index], hash).unwrap_or_else(|e| e.to_string());
                        let detail_chunks = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Min(1), Constraint::Length(1)]).split(commit_and_detail[1]);
                        let mut para = Paragraph::new(details.clone()).block(Block::default().title("Details").borders(Borders::ALL)
                            .style(if focus==FocusArea::Detail {Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)} else {Style::default().fg(Color::Magenta)})).wrap(Wrap { trim:true }).scroll((detail_scroll,0));
                        f.render_widget(para, detail_chunks[0]);
                        let lines=details.lines().count();
                        let mut ds=ScrollbarState::default().position(detail_scroll as usize).content_length(lines);
                        f.render_stateful_widget(Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight), detail_chunks[1], &mut ds);
                    }
                }
            }
        }
    }

    // footer
    let filter_label = if filter_by_user {"u: Nur eigene"} else {"u: Alle"};
    let footer = Paragraph::new(format!("Tasten: ←/→ Zeitfenster | ↑/↓ Navigation/Scroll | Tab Fokus | Space Details | {} | z: Zitat | q Beenden", filter_label))
        .block(Block::default().borders(Borders::ALL)).style(Style::default().fg(Color::Gray).add_modifier(Modifier::DIM));
    f.render_widget(footer, vertical_chunks[1]);

    // popup
    if let Some(arc) = popup_quote {
        let popup = arc.lock().unwrap();
        if popup.visible {
            let popup_area = centered_rect(60,20,f.size());
            f.render_widget(Clear, popup_area);
            let block = Block::default().title("Zitat des Tages (ESC)").borders(Borders::ALL).style(Style::default().fg(Color::Magenta).bg(Color::Black));
            let para = Paragraph::new(popup.text.clone()).block(block).wrap(Wrap{trim:true}).alignment(Alignment::Center).style(Style::default().fg(Color::White));
            f.render_widget(para, popup_area);
        }
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Percentage((100-percent_y)/2), Constraint::Percentage(percent_y), Constraint::Percentage((100-percent_y)/2)]).split(r)[1];
    Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage((100-percent_x)/2), Constraint::Percentage(percent_x), Constraint::Percentage((100-percent_x)/2)]).split(vertical)[1]
}