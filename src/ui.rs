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
use crate::utils::commit_hash;
use crate::models::SelectedCommits;
use crate::models::LockExt;
use crate::CommitTab;
use once_cell::sync::Lazy;
use regex::Regex;
use crate::theme::Theme;

// Type alias for commit data for clarity
pub type CommitData = Vec<(PathBuf, Vec<String>)>;

// Compile the ticket regex once for all uses
static TICKET_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"[A-Z]+-\d+").unwrap());

/// Renders a commit line with syntax highlighting and ticket detection.
fn render_commit_line<'a>(commit: &'a str, indicator: String, filter_by_user: bool, detailed: bool, theme: &Theme) -> Line<'a> {
    // Append a subject string, highlighting ticket references (e.g. ABC-123).
    let push_subject = |spans: &mut Vec<Span<'a>>, subject: &str| {
        let subject = subject.trim();
        let mut last = 0;
        for m in TICKET_REGEX.find_iter(subject) {
            if m.start() > last {
                spans.push(Span::raw(subject[last..m.start()].to_owned()));
            }
            spans.push(Span::styled(subject[m.start()..m.end()].to_owned(), theme.commit_ticket));
            last = m.end();
        }
        if last < subject.len() {
            spans.push(Span::raw(subject[last..].to_owned()));
        }
    };

    let mut spans = vec![Span::raw(indicator), Span::raw(" ")];

    // Detailed view: the first line is "<hash> <YYYY-MM-DD> <HH:MM>" (space
    // separated, no '|'); the subject is the next line, body lines follow.
    if detailed {
        let mut lines = commit.lines();
        let first = lines.next().unwrap_or(commit);
        let mut it = first.splitn(2, ' ');
        if let Some(hash) = it.next() {
            spans.push(Span::styled(hash.trim().to_owned(), theme.commit_hash));
        }
        if let Some(datetime) = it.next() {
            spans.push(Span::raw(" | "));
            spans.push(Span::styled(datetime.trim().to_owned(), theme.commit_datetime));
        }
        if let Some(subject) = lines.next() {
            if !subject.trim().is_empty() {
                spans.push(Span::raw(" | "));
                push_subject(&mut spans, subject);
            }
        }
        return Line::from(spans);
    }

    let parts: Vec<&str> = if filter_by_user {
        commit.splitn(3, '|').collect()
    } else {
        commit.splitn(4, '|').collect()
    };

    if let Some(hash) = parts.first() {
        spans.push(Span::styled(hash.trim().to_owned(), theme.commit_hash));
        spans.push(Span::raw(" | "));
    }

    if let Some(datetime) = parts.get(1) {
        spans.push(Span::styled(datetime.trim().to_owned(), theme.commit_datetime));
        spans.push(Span::raw(" | "));
    }

    if filter_by_user {
        if let Some(subject_str) = parts.get(2) {
            push_subject(&mut spans, subject_str);
        }
    } else {
        if let Some(author) = parts.get(2) {
            spans.push(Span::styled(author.trim().to_owned(), theme.commit_author));
            spans.push(Span::raw(" | "));
        }
        if let Some(subject_str) = parts.get(3) {
            push_subject(&mut spans, subject_str);
        }
    }

    Line::from(spans)
}

/// Builds the list rows for one commit: the highlighted summary line, plus a
/// short preview of the first body lines in detailed mode. Returned as the
/// lines of a single multi-line `ListItem` so the selection index stays aligned
/// (one item per commit). Full details remain available via the Space pane.
fn commit_item_lines<'a>(commit: &'a str, indicator: String, filter_by_user: bool, detailed: bool, theme: &Theme) -> Vec<Line<'a>> {
    let mut lines = vec![render_commit_line(commit, indicator, filter_by_user, detailed, theme)];
    if detailed {
        for line in commit.lines().skip(2).map(str::trim).filter(|l| !l.is_empty()).take(2) {
            lines.push(Line::from(vec![
                Span::raw("      "),
                Span::styled(line.to_owned(), Style::default().fg(theme.text_secondary)),
            ]));
        }
    }
    lines
}

/// Renders the commits view.
pub fn render_commits(
    f: &mut Frame,
    theme: &Theme,
    _repos: &[PathBuf],
    selected_repo_index: usize,
    data: &CommitData,
    interval_label: &str,
    from_date: &Option<String>,
    to_date: &Option<String>,
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
    detailed_commit_view: bool,
) {
    let display_interval = if let (Some(from), to) = (from_date, to_date) {
        let to_str = to.as_deref().unwrap_or("today");
        format!("{} to {}", from, to_str)
    } else {
        interval_label.to_string()
    };

    f.render_widget(Block::default().style(Style::default().bg(theme.root_bg)), f.area());

    // Hashes of marked commits, for the "*" indicator in the timeframe view.
    let selected_set: std::collections::BTreeSet<String> = selected_commits
        .map(|arc| arc.lock_safe().set.keys().cloned().collect())
        .unwrap_or_default();
    // Determine if we should dim the background
    let dim_bg = popup_quote.is_some_and(|arc| arc.lock_safe().visible);
    let bg_fg = if dim_bg { theme.blurred_border } else { theme.text };
    let bg_cyan = if dim_bg { theme.blurred_border } else { theme.focus_border };
    let bg_magenta = if dim_bg { theme.blurred_border } else { Color::Magenta }; // Not in theme yet
    // let bg_green = if dim_bg { theme.blurred_border } else { Color::Green }; // Not in theme yet
    let bg_yellow = if dim_bg { theme.blurred_border } else { theme.text_highlight };
    let _bg_red = if dim_bg { theme.blurred_border } else { Color::Red }; // Not in theme yet

    // Single source of truth for the screen regions (shared with main.rs).
    let layout = compute_layout(f.area(), show_details, selected_commit_index.is_some());
    let sidebar_area = layout.sidebar;
    let commit_area = layout.commit;
    let detail_area = layout.detail;

    // Split sidebar area into sidebar and button box
    let sidebar_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(2), // sidebar list
            // Removed button box area
        ])
        .split(sidebar_area);

    // Sidebar list (only repos with commits in the current timeframe). One line
    // per entry: "All" (item 0), a divider (item 1), then one repo per item.
    // The mouse hit-test in input.rs mirrors this layout, so keep them in sync.
    let filtered_repos: Vec<&PathBuf> = data.iter().map(|(repo,_)| repo).collect();
    let mut repo_list = Vec::new();
    let total_commits: usize = data.iter().map(|(_, c)| c.len()).sum();
    // 'All' entry
    let all_selected = selected_repo_index == usize::MAX;
    let all_style = if all_selected {
        Style::default().fg(bg_yellow).add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default().fg(bg_fg).add_modifier(Modifier::BOLD)
    };
    repo_list.push(ListItem::new(Line::from(vec![
        Span::styled("\u{1F30D} All Projects", all_style), // 🌍
        Span::styled(format!("  {}", total_commits), Style::default().fg(theme.text_secondary)),
    ])));
    // Visual divider, scaled to the sidebar width.
    let divider_width = sidebar_area.width.saturating_sub(2).max(1) as usize;
    repo_list.push(ListItem::new(Line::from(vec![Span::styled(
        "─".repeat(divider_width),
        Style::default().fg(theme.blurred_border),
    )])));
    // Per-repo entries (only those with commits)
    if filtered_repos.is_empty() {
        repo_list.push(ListItem::new(Line::from(vec![Span::styled(
            "No projects. Try <Tab>",
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
            let count = data.iter().find(|(r,_)| r == *repo).map(|(_,c)| c.len()).unwrap_or(0);
            let (name_style, count_style) = if selected {
                let s = Style::default().fg(bg_yellow).add_modifier(Modifier::BOLD | Modifier::REVERSED);
                (s, s)
            } else if count > 0 {
                (theme.repo_path, theme.repo_commit_count)
            } else {
                let dim = Style::default().fg(theme.blurred_border).add_modifier(Modifier::DIM);
                (dim, dim)
            };
            repo_list.push(ListItem::new(Line::from(vec![
                Span::styled(format!("\u{1F5C3} {}", name), name_style), // 🗃️
                Span::styled(format!("  {}", count), count_style),
            ])));
        }
    }
    let sidebar = List::new(repo_list)
        .style(Style::default().fg(bg_fg));
    let mut sidebar_state = ListState::default();
    // "All" is item 0, the divider item 1, so repo i is item i + 2. Selecting
    // the right item also lets the List auto-scroll to keep it visible.
    sidebar_state.select(Some(if selected_repo_index == usize::MAX { 0 } else { selected_repo_index + 2 }));
    let sidebar_block = Block::default().title("Repositories [1]").borders(Borders::ALL)
        .style(Style::default().fg(bg_cyan));
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
    f.render_widget(tabs, layout.tabs);

    let list_area = Rect {
        x: commit_area.x,
        y: commit_area.y + 3,
        width: commit_area.width,
        height: commit_area.height.saturating_sub(3),
    };

    // Header
    let header = if selected_repo_index==usize::MAX {
        if filter_by_user { format!("Standup Commits (only mine) – {}", display_interval) }
        else { format!("Standup Commits – {}", display_interval) }
    } else if let Some((repo,_)) = data.get(selected_repo_index) {
        let name = repo.file_name().unwrap_or_default().to_string_lossy();
        if filter_by_user { format!("{} (only mine) – {}", name, display_interval)} else {format!("{} – {}", name, display_interval)}
    } else { format!("Standup Commits – {}", display_interval) };
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
                        theme.repo_commit_count
                    )])));
                    for (i, commit) in commits.iter().enumerate() {
                        let idx = offset + i;
                        let sel = Some(idx) == selected_commit_index;
                        let star = if selected_set.contains(commit_hash(commit)) {"*"} else {" "};
                        let indicator = format!("{}{}", star, if sel {"→"} else {"  " });
                        let style = if sel {Style::default().fg(theme.selection_fg).add_modifier(Modifier::BOLD)} else {Style::default().fg(bg_fg)};
                        let item_lines = commit_item_lines(commit, indicator, filter_by_user, detailed_commit_view, theme);
                        let mut item = ListItem::new(item_lines).style(style);
                        if sel {
                            item = item.bg(theme.selection_bg);
                        }
                        items.push(item);
                    }
                    offset += commits.len();
                }
                let mut state = ListState::default(); state.select(selected_commit_index);
                let list = List::new(items).block(Block::default().title(header).borders(Borders::ALL)
                    .style(if focus==FocusArea::CommitList {Style::default().fg(bg_cyan).add_modifier(Modifier::BOLD)} else {Style::default().fg(bg_cyan)}));
                f.render_stateful_widget(list, list_area, &mut state);
                // scrollbar
                let total: usize = data.iter().map(|(_,c)|c.len()).sum();
                let pos = selected_commit_index.unwrap_or(0);
                let mut sb = ScrollbarState::default().position(pos).content_length(total);
                f.render_stateful_widget(Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight), commit_layout[1], &mut sb);
            } else if let Some((_repo, commits)) = data.get(selected_repo_index) {
                let items: Vec<ListItem> = commits.iter().enumerate().map(|(i, commit)| {
                    let sel = Some(i) == selected_commit_index;
                    let star = if selected_set.contains(commit_hash(commit)) {"*"} else {" "};
                    let indicator = format!("{}{}", star, if sel {"→"} else {"  " });
                    let style = if sel {Style::default().fg(theme.selection_fg).add_modifier(Modifier::BOLD)} else {Style::default().fg(bg_fg)};
                    let item_lines = commit_item_lines(commit, indicator, filter_by_user, detailed_commit_view, theme);
                    let mut item = ListItem::new(item_lines).style(style);
                    if sel {
                        item = item.bg(theme.selection_bg);
                    }
                    item
                }).collect();
                let mut state=ListState::default(); state.select(selected_commit_index);
                let list = List::new(items).block(Block::default().title(header).borders(Borders::ALL)
                    .style(if focus==FocusArea::CommitList {Style::default().fg(bg_cyan).add_modifier(Modifier::BOLD)} else {Style::default().fg(bg_cyan)}));
                f.render_stateful_widget(list, list_area, &mut state);
                // scrollbar
                let total=commits.len();
                let pos=selected_commit_index.unwrap_or(0);
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
                let sel = selected_commits.lock_safe();
                if sel.set.is_empty() {
                    let placeholder = Paragraph::new("No commits selected. Press 'm' to add commits to your selection.")
                        .block(Block::default().title("Selected Commits").borders(Borders::ALL))
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(bg_fg));
                    f.render_widget(placeholder, list_area);
                } else {
                    // Build from the stored selection so marks made under other
                    // timeframes still appear; group by repo, ordered by hash.
                    let mut repo_to_commits: std::collections::BTreeMap<&PathBuf, Vec<&String>> = std::collections::BTreeMap::new();
                    for (repo, line) in sel.set.values() {
                        repo_to_commits.entry(repo).or_default().push(line);
                    }
                    let mut items = Vec::new();
                    for (repo, commits) in repo_to_commits.iter() {
                        items.push(ListItem::new(Line::from(vec![Span::styled(
                            format!("\u{1F5C3}  {}", repo.file_name().unwrap_or_default().to_string_lossy()),
                            theme.repo_commit_count
                        )])));
                        for commit in commits.iter() {
                            let indicator = "*  ".to_string();
                            let style = Style::default().fg(theme.selection_fg).add_modifier(Modifier::BOLD);
                            let line = render_commit_line(commit, indicator, filter_by_user, detailed_commit_view, theme);
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
                // Space always shows the full `git show` output, independent of
                // the detailed-list toggle.
                let hash = commit_hash(&commit_line);
                let details = get_commit_details(&repo_path, hash).unwrap_or_else(|e| e.to_string());
                // Clear the region and draw the border; the block's inner area
                // is the padded text region (no manual space-blanking needed).
                f.render_widget(Clear, detail_chunk);
                let detail_block = Block::default()
                    .title("Details")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(bg_magenta));
                let inner = detail_block.inner(detail_chunk);
                f.render_widget(detail_block, detail_chunk);
                // split into text + scrollbar
                let detail_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
                    .split(inner);
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

    // footer — colored state chips (timeframe / filter / detail toggle) make
    // the active modes obvious at a glance, followed by the keys relevant to
    // the focused area. Stays on one line; chips sit first so they survive a
    // truncation on narrow terminals.
    let footer_block = Block::default().borders(Borders::ALL);
    if dim_bg {
        // A summary popup is open; it carries its own action hints.
        let footer = Paragraph::new("Esc close summary")
            .block(footer_block)
            .style(theme.footer.fg(theme.blurred_border));
        f.render_widget(footer, layout.footer);
    } else {
        // A filled background reads as "on" / active; dim text reads as "off".
        let chip = |label: String, bg: Color| {
            Span::styled(format!(" {label} "), Style::default().fg(Color::Black).bg(bg).add_modifier(Modifier::BOLD))
        };
        let gap = Span::raw(" ");

        let tf_chip = chip(format!("\u{23F1} {display_interval}"), theme.focus_border); // ⏱
        let filter_chip = if filter_by_user {
            chip("\u{25C9} mine".into(), Color::Green) // ◉
        } else {
            chip("\u{25C9} all".into(), Color::Magenta)
        };
        let detail_chip = if detailed_commit_view {
            chip("\u{25C9} details".into(), theme.text_highlight) // ◉
        } else {
            Span::styled(" \u{25CB} details ", Style::default().fg(theme.blurred_border).add_modifier(Modifier::DIM)) // ○
        };

        let keys = match focus {
            FocusArea::Sidebar =>
                "\u{2191}/\u{2193} repo \u{00B7} \u{2192}/l commits \u{00B7} Tab timeframe \u{00B7} u mine/all \u{00B7} a summary \u{00B7} q quit",
            FocusArea::CommitList =>
                "\u{2191}/\u{2193} commit \u{00B7} Space details \u{00B7} m mark \u{00B7} s selection \u{00B7} u mine/all \u{00B7} d details \u{00B7} a summary \u{00B7} q quit",
            FocusArea::Detail =>
                "\u{2191}/\u{2193} scroll \u{00B7} \u{2190}/h back \u{00B7} Space close \u{00B7} a summary \u{00B7} q quit",
        };

        let line = Line::from(vec![
            tf_chip,
            gap.clone(),
            filter_chip,
            gap.clone(),
            detail_chip,
            Span::styled("  \u{2502} ", Style::default().fg(theme.blurred_border)), // │
            Span::styled(keys, theme.footer),
        ]);
        let footer = Paragraph::new(line).block(footer_block);
        f.render_widget(footer, layout.footer);
    }

    // popup
    if let Some(arc) = popup_quote {
        let popup = arc.lock_safe();
        if popup.visible {
            // Dim the background
            let area = f.area();
            let dim_block = Block::default().style(Style::default().bg(theme.dim_bg).fg(Color::Reset));
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
            let interval = format!("Interval: {}", display_interval);
            let x_button = Span::styled("[X]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));
            let mut title_line = vec![
                Span::styled(&title, theme.popup_title),
                Span::raw("  "),
                Span::styled(&interval, Style::default().fg(theme.text_highlight)),
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
                .style(theme.popup_border)
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
                .style(theme.popup_text);
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
            let footer = if popup.loading {
                Paragraph::new("Esc cancel")
                    .style(Style::default().fg(theme.text_secondary).add_modifier(Modifier::ITALIC))
            } else if popup.copied {
                Paragraph::new("✓ Copied to clipboard")
                    .style(Style::default().fg(theme.commit_author.fg.unwrap_or(Color::Green)).add_modifier(Modifier::BOLD))
            } else {
                Paragraph::new("Enter copy & close | c copy | r regenerate | ↑/↓ scroll | Esc close")
                    .style(Style::default().fg(theme.text_secondary).add_modifier(Modifier::ITALIC))
            };
            f.render_widget(footer, footer_area);
        }
    }
}

/// Screen regions for the main view, computed once so rendering and mouse
/// hit-testing agree on the exact same rectangles.
pub struct AppLayout {
    pub sidebar: Rect,
    pub commit: Rect,
    pub detail: Option<Rect>,
    pub tabs: Rect,
    pub footer: Rect,
}

/// Computes the main layout. The detail column only appears when the detail
/// pane is toggled on and a commit is selected.
pub fn compute_layout(area: Rect, show_details: bool, has_selection: bool) -> AppLayout {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);
    // Responsive sidebar: ~1/4 of the width, clamped so it stays readable on
    // narrow terminals and doesn't waste space on wide ones.
    let sidebar_w = (area.width / 4).clamp(22, 36);
    let columns = if show_details && has_selection {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(sidebar_w), Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(vertical[0])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(sidebar_w), Constraint::Min(1)])
            .split(vertical[0])
    };
    let sidebar = columns[0];
    let commit = columns[1];
    let detail = if columns.len() > 2 { Some(columns[2]) } else { None };
    let tabs = Rect { x: commit.x, y: commit.y, width: commit.width, height: 3 };
    AppLayout { sidebar, commit, detail, tabs, footer: vertical[1] }
}

/// Centers a rectangle within another rectangle.
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Percentage((100-percent_y)/2), Constraint::Percentage(percent_y), Constraint::Percentage((100-percent_y)/2)]).split(r)[1];
    Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage((100-percent_x)/2), Constraint::Percentage(percent_x), Constraint::Percentage((100-percent_x)/2)]).split(vertical)[1]
}