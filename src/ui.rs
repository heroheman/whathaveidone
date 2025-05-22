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
    detailed_commit_view: bool, // <-- new argument
) {
    let selected_set = selected_commits.map(|arc| arc.lock().unwrap().set.clone()).unwrap_or_default();
    let area = f.area();
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)]).split(area);

    // Determine if we should dim the background
    let dim_bg = popup_quote.map_or(false, |arc| arc.lock().unwrap().visible);
    let bg_fg = if dim_bg { Color::DarkGray } else { Color::White };
    let bg_cyan = if dim_bg { Color::DarkGray } else { Color::Cyan };
    let bg_magenta = if dim_bg { Color::DarkGray } else { Color::Magenta };
    let bg_green = if dim_bg { Color::DarkGray } else { Color::Green };
    let bg_yellow = if dim_bg { Color::DarkGray } else { Color::Yellow };
    let _bg_red = if dim_bg { Color::DarkGray } else { Color::Red };

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
    // Calculate total commit count for all projects
    let total_commits: usize = data.iter().map(|(_, c)| c.len()).sum();
    // 'All' entry
    let all_selected = selected_repo_index == usize::MAX;
    let all_style = if all_selected {
        Style::default().fg(bg_yellow).add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default().fg(bg_fg)
    };
    repo_list.push(ListItem::new(vec![
        Line::from(vec![Span::styled(
            format!("\u{1F30D}  All Projects ({} total)", filtered_repos.len()), // 🌍
            all_style
        )]),
        Line::from(vec![Span::styled(
            format!("  {} commit{} in {}", total_commits, if total_commits == 1 { "" } else { "s" }, interval_label),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)
        )]),
        Line::from(vec![Span::raw("")]),
    ]));
    // Visual divider
    repo_list.push(ListItem::new(Line::from(vec![Span::styled("━━━━━━━━━━━━━━━━━━━━", Style::default().fg(Color::Gray))])));
    // Per-repo entries (only those with commits)
    if filtered_repos.is_empty() {
        repo_list.push(ListItem::new(Line::from(vec![Span::styled(
            "No projects found. Try another timeframe with <Tab>",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        )])));
    } else {
        for (i, repo) in filtered_repos.iter().enumerate() {
            let name = if let Some(fname) = repo.file_name() {
                fname.to_string_lossy()
            } else if let Some(parent) = repo.parent() {
                parent.file_name().unwrap_or_default().to_string_lossy()
            } else {
                repo.to_string_lossy()
            };
            let selected = selected_repo_index == i;
            let style = if selected {
                Style::default().fg(bg_yellow).add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default().fg(Color::Cyan)
            };
            let count = data.iter().find(|(r,_)| r == *repo).map(|(_,c)| c.len()).unwrap_or(0);
            let count_style = if count > 0 {
                Style::default().fg(bg_green).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)
            };
            repo_list.push(ListItem::new(vec![
                Line::from(vec![Span::styled(
                    format!("\u{1F5C3}  {}", name), // 🗃️ (smaller folder icon)
                    style
                )]),
                Line::from(vec![Span::styled(
                    format!("   {} commit{}", count, if count == 1 { "" } else { "s" }),
                    count_style
                )]),
                Line::from(vec![Span::raw("")]),
            ]));
        }
    }
    let sidebar = List::new(repo_list)
        .highlight_symbol("▶ ")
        .style(Style::default().fg(bg_fg)); // removed .bg(Color::Rgb(30,34,40))
    let mut sidebar_state = ListState::default();
    sidebar_state.select(Some(if selected_repo_index==usize::MAX {0} else {selected_repo_index*3+2}));
    let sidebar_block = Block::default().title("Repositories [1]").borders(Borders::ALL)
        .style(Style::default().fg(bg_cyan)); // removed .bg(Color::Rgb(30,34,40))
    f.render_stateful_widget(sidebar.block(sidebar_block), sidebar_chunks[0], &mut sidebar_state);

    // Commit list layout with scrollbar
    let commit_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
        .split(commit_area);

    // Tabs for commit list (refactored)
    // let tab_titles = ["Timeframe [2]", "Selection [3]", "Stats [4]"];
    let tab_titles = ["Timeframe [2]", "Selection [3]"];
    let tabs = ratatui::widgets::Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::ALL).title("Select View"))
        .style(Style::default().fg(bg_fg))
        .highlight_style(Style::default().fg(bg_yellow).bold().underlined())
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
        let name = repo.file_name().unwrap_or_default().to_string_lossy();
        if filter_by_user { format!("{} (only mine) – {}", name, interval_label)} else {format!("{} – {}", name, interval_label)}
    } else { format!("Standup Commits – {}", interval_label) };
    let _header_style = Style::default().fg(bg_fg);

    // Render commit list depending on active tab
    match selected_tab {
        CommitTab::Timeframe => {
            if selected_repo_index==usize::MAX {
                let mut items = Vec::new();
                let mut offset=0;
                for (repo, commits) in data {
                    items.push(ListItem::new(Line::from(vec![Span::styled(
                        format!("\u{1F5C3}  {}", repo.file_name().unwrap_or_default().to_string_lossy()),
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    )])));
                    for (i, commit) in commits.iter().enumerate() {
                        let idx = offset + i;
                        let sel = Some(idx) == selected_commit_index;
                        let star = if let Some(hash) = commit.split_whitespace().next() { if selected_set.contains(hash) {"*"} else {" "} } else {" "};
                        let indicator = format!("{}{}", star, if sel {"→"} else {"  " });
                        let style = if sel {Style::default().fg(bg_yellow).add_modifier(Modifier::BOLD)} else {Style::default().fg(bg_fg)};
                        if detailed_commit_view {
                            // Render multi-line commit block
                            let mut lines = Vec::new();
                            let mut commit_lines = commit.lines();
                            if let Some(first) = commit_lines.next() {
                                // Highlight the first line (summary)
                                lines.push(Line::from(vec![Span::raw(indicator.clone()), Span::raw(" "), Span::styled(first.to_string(), Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))]));
                            }
                            for line in commit_lines {
                                lines.push(Line::from(format!("    {}", line)));
                            }
                            items.push(ListItem::new(lines).style(style));
                        } else {
                            let line = render_commit_line(commit, indicator, sel);
                            items.push(ListItem::new(line).style(style));
                        }
                    }
                    offset += commits.len();
                }
                let mut state = ListState::default(); state.select(selected_commit_index);
                let list = List::new(items).block(Block::default().title(header).borders(Borders::ALL)
                    .style(if focus==FocusArea::CommitList {Style::default().fg(bg_cyan).add_modifier(Modifier::BOLD)} else {Style::default().fg(bg_cyan)}));
                f.render_stateful_widget(list, list_area, &mut state);
                // scrollbar
                let total: usize = data.iter().map(|(_,c)|c.len()).sum();
                let visible = commit_area.height.saturating_sub(2) as usize;
                let pos = selected_commit_index.unwrap_or(0).saturating_sub(visible.saturating_sub(visible));
                let mut sb = ScrollbarState::default().position(pos).content_length(total);
                f.render_stateful_widget(Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight), commit_layout[1], &mut sb);
            } else if let Some((_repo, commits)) = data.get(selected_repo_index) {
                let items: Vec<ListItem> = commits.iter().enumerate().map(|(i, commit)|{
                    let sel=Some(i)==selected_commit_index;
                    let star = if let Some(hash) = commit.split_whitespace().next() { if selected_set.contains(hash) {"*"} else {" "} } else {" "};
                    let indicator = format!("{}{}", star, if sel {"→"} else {"  " });
                    let style=if sel {Style::default().fg(bg_yellow).add_modifier(Modifier::BOLD)} else {Style::default().fg(bg_fg)};
                    if detailed_commit_view {
                        let mut lines = Vec::new();
                        let mut commit_lines = commit.lines();
                        if let Some(first) = commit_lines.next() {
                            lines.push(Line::from(vec![Span::raw(indicator.clone()), Span::raw(" "), Span::styled(first.to_string(), Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))]));
                        }
                        for line in commit_lines {
                            lines.push(Line::from(format!("    {}", line)));
                        }
                        ListItem::new(lines).style(style)
                    } else {
                        let line = render_commit_line(commit, indicator, sel);
                        ListItem::new(line).style(style)
                    }
                }).collect();
                let mut state=ListState::default(); state.select(selected_commit_index);
                let list = List::new(items).block(Block::default().title(header).borders(Borders::ALL)
                    .style(if focus==FocusArea::CommitList {Style::default().fg(bg_cyan).add_modifier(Modifier::BOLD)} else {Style::default().fg(bg_cyan)}));
                f.render_stateful_widget(list, list_area, &mut state);
                // scrollbar
                let total=commits.len();
                let visible=commit_area.height.saturating_sub(2) as usize;
                let pos=selected_commit_index.unwrap_or(0).saturating_sub(visible.saturating_sub(visible));
                let mut sb=ScrollbarState::default().position(pos).content_length(total);
                f.render_stateful_widget(Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight), commit_layout[1], &mut sb);
            } else {
                // No repo at selected_repo_index, show placeholder
                let placeholder = Paragraph::new("No commits found.")
                    .block(Block::default().title(header).borders(Borders::ALL))
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(bg_fg));
                f.render_widget(placeholder, list_area);
            }
        },
        CommitTab::Selection => {
            if let Some(selected_commits) = selected_commits {
                let sel = selected_commits.lock().unwrap();
                if sel.set.is_empty() {
                    let placeholder = Paragraph::new("No commits selected. Press 'm' to add commits to your selection.")
                        .block(Block::default().title("Selected Commits").borders(Borders::ALL))
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(bg_fg));
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
                        items.push(ListItem::new(Line::from(vec![Span::styled(
                            format!("\u{1F5C3}  {}", repo.file_name().unwrap().to_string_lossy()),
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                        )])));
                        for commit in commits.iter() {
                            let star = if let Some(hash) = commit.split_whitespace().next() { if sel.set.contains(hash) {"*"} else {" "} } else {" "};
                            let indicator = format!("{}  ", star);
                            let style = Style::default().fg(bg_yellow).add_modifier(Modifier::BOLD);
                            let line = render_commit_line(commit, indicator, true);
                            items.push(ListItem::new(line).style(style));
                        }
                    }
                    let mut state = ListState::default(); state.select(selected_commit_index);
                    let list = List::new(items).block(Block::default().title("Selected Commits").borders(Borders::ALL)
                        .style(if focus==FocusArea::CommitList {Style::default().fg(bg_cyan).add_modifier(Modifier::BOLD)} else {Style::default().fg(bg_cyan)}));
                    f.render_stateful_widget(list, list_area, &mut state);
                }
            }
        }
        CommitTab::Stats => {
            // Render a 2x2 grid of 4 boxes with icons and color
            let grid = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(list_area);
            let top = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(grid[0]);
            let bottom = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(grid[1]);
            let boxes = [top[0], top[1], bottom[0], bottom[1]];
            let icons = ["\u{1F4C8}", "\u{1F465}", "\u{1F4C6}", "\u{1F4CB}"]; // 📈 👥 📆 📋
            let titles = ["Commits", "Authors", "Days", "Summary"];
            let colors = [Color::Green, Color::Cyan, Color::Yellow, Color::Magenta];
            for (i, area) in boxes.iter().enumerate() {
                let block = Block::default()
                    .title(format!("{}  {}", icons[i], titles[i]))
                    .borders(Borders::ALL)
                    .style(Style::default().fg(colors[i]));
                f.render_widget(block, *area);
            }
        }
    }

    // Unified detail view rendering on the right when toggled
    if let Some(detail_chunk) = detail_area {
        if show_details {
            if let Some(sel_idx) = selected_commit_index {
                let (repo_path, commit_line) = {
                    if selected_repo_index == usize::MAX {
                        let mut offset = 0;
                        let mut found: Option<(PathBuf, String)> = None;
                        for (repo, repo_commits) in data {
                            if sel_idx < offset + repo_commits.len() {
                                found = Some((repo.clone(), repo_commits.get(sel_idx - offset).cloned().unwrap_or_default()));
                                break;
                            }
                            offset += repo_commits.len();
                        }
                        found.unwrap_or_else(|| {
                            if let Some((r, commits_vec)) = data.first() {
                                (r.clone(), commits_vec.first().cloned().unwrap_or_default())
                            } else {
                                (PathBuf::new(), String::new())
                            }
                        })
                    } else if let Some((r, commits_vec)) = data.get(selected_repo_index) {
                        (r.clone(), commits_vec.get(sel_idx).cloned().unwrap_or_default())
                    } else {
                        (PathBuf::new(), String::new())
                    }
                };
                let details = if detailed_commit_view {
                    // Show the full multi-line commit block as the detail
                    commit_line.clone()
                } else {
                    let hash = commit_line.split_whitespace().next().unwrap_or("");
                    get_commit_details(&repo_path, hash).unwrap_or_else(|e| e.to_string())
                };
                // clear detail region
                f.render_widget(Clear, detail_chunk);
                // draw border around detail
                let detail_block = Block::default()
                    .title("Details")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(bg_magenta));
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
                    .scroll((detail_scroll, 0))
                    .style(Style::default().fg(bg_fg));
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
    let detail_label = if detailed_commit_view {"d: Details ON"} else {"d: Details OFF"};
    let footer = Paragraph::new(format!(
        "Tab/Shift+Tab Timeframe | ↑/↓/ or h/j/k/l Navigation | <Space> Details |  m Mark | s Show Marked | a AI summary | {} | {} | Q Quit",
        filter_label, detail_label
    ))
    .block(Block::default().borders(Borders::ALL))
    .style(Style::default().fg(if dim_bg { Color::DarkGray } else { Color::Gray }).add_modifier(Modifier::DIM));
    f.render_widget(footer, vertical_chunks[1]);

    // popup
    if let Some(arc) = popup_quote {
        let popup = arc.lock().unwrap();
        if popup.visible {
            // Dim the background
            let area = f.area();
            let dim_block = Block::default().style(Style::default().bg(Color::Rgb(30, 30, 30)).fg(Color::Reset));
            f.render_widget(dim_block, area);
            // Centered popup area
            let popup_area = centered_rect(60, 80, f.area());
            f.render_widget(Clear, popup_area);

            // Header: icon, project, interval
            let project = if selected_repo_index == usize::MAX {
                "All projects".to_string()
            } else if let Some((repo, _)) = data.get(selected_repo_index) {
                repo.file_name().unwrap_or_default().to_string_lossy().to_string()
            } else {
                "Project".to_string()
            };
            let title = format!("\u{1F916}  AI Summary for {}", project);
            let interval = format!("Interval: {}", interval_label);
            let x_button = Span::styled("[X]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));
            let mut title_line = vec![
                Span::styled(&title, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(&interval, Style::default().fg(Color::Yellow)),
            ];
            // Pad to right edge
            let popup_width = popup_area.width as usize;
            let title_width = title.len() + interval.len() + 2;
            let x_button_width = 3;
            let pad = if popup_width > title_width + x_button_width + 2 { popup_width - title_width - x_button_width - 2 } else { 1 };
            title_line.push(Span::raw(" ".repeat(pad)));
            title_line.push(x_button);

            // Block for popup
            let block = Block::default()
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black))
                .title(Line::from(title_line));

            let scroll = popup.scroll;
            let text_line_count = popup.text.lines().count() as u16;

            // Loading spinner/animation
            let spinner = if popup.loading {
                let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let frame = frames[(popup.spinner_frame as usize) % frames.len()];
                format!("{} ", frame)
            } else {
                String::new()
            };

            // Content: always show popup.text (which includes variables if loading)
            let padded_text = if popup.loading {
                // Show spinner above the text
                format!(
                    "\n   {}Loading...\n\n{}\n",
                    spinner,
                    popup.text
                        .lines()
                        .map(|line| format!("  {}  ", line))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            } else {
                format!(
                    "\n{}\n",
                    popup.text
                        .lines()
                        .map(|line| format!("  {}  ", line))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            };

            let para = Paragraph::new(padded_text)
                .block(block)
                .wrap(Wrap { trim: true })
                .alignment(Alignment::Left)
                .scroll((scroll, 0))
                .style(Style::default().fg(Color::White));
            f.render_widget(para, popup_area);

            // Draw a vertical scrollbar inside the popup
            let scrollbar_area = Rect {
                x: popup_area.x + popup_area.width - 1,
                y: popup_area.y + 1,
                width: 1,
                height: popup_area.height.saturating_sub(2),
            };
            let mut sb = ScrollbarState::default()
                .position(scroll as usize)
                .content_length(text_line_count as usize);
            f.render_stateful_widget(Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight), scrollbar_area, &mut sb);

            // Footer visually separated
            let footer_area = Rect {
                x: popup_area.x,
                y: popup_area.y + popup_area.height,
                width: popup_area.width,
                height: 1,
            };
            let footer = Paragraph::new("Press c to copy | ↑/↓ scroll | Esc close")
                .style(Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC));
            f.render_widget(footer, footer_area);
        }
    }

    if let Some(selected_commits) = selected_commits {
        let sel = selected_commits.lock().unwrap();
        if sel.popup_visible {
            let popup_area = centered_rect(60, 40, f.area());
            f.render_widget(Clear, popup_area);
            // Header with icon and color
            let mut lines = vec![Line::from(vec![
                Span::styled("\u{1F4CB}  Selected Commits", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            ])];
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
                .block(Block::default()
                    .title(Span::styled("\u{1F4CB}  Selected Commits", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)))
                    .borders(Borders::ALL)
                    .style(Style::default().bg(Color::Black)))
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