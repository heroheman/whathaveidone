use ratatui::{
    prelude::*,
    widgets::{Block, Borders, BorderType, Padding, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap, ListState, Clear, BarChart, Sparkline},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Line},
    symbols,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::models::{FocusArea, OverviewState, OverviewFocus};
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
#[allow(clippy::too_many_arguments)]
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
    overview_state: &Arc<Mutex<OverviewState>>,
    selected_commits: Option<&Arc<Mutex<SelectedCommits>>>,
    selected_tab: CommitTab,
    detailed_commit_view: bool,
    show_overview: bool,
    overview_selected: usize,
    overview_focus: OverviewFocus,
    overview_detail_scroll: u16,
    show_help: bool,
    show_stats: bool,
) {
    let display_interval = if let (Some(from), to) = (from_date, to_date) {
        let to_str = to.as_deref().unwrap_or("today");
        format!("{} to {}", from, to_str)
    } else {
        interval_label.to_string()
    };

    f.render_widget(Block::default().style(Style::default().bg(theme.root_bg)), f.area());

    // The overview view fully owns the screen when active.
    if show_overview {
        render_overview(f, theme, overview_state, overview_selected, overview_focus, overview_detail_scroll, &display_interval, filter_by_user);
        if show_help { render_help_overlay(f, theme, true); }
        return;
    }

    // The stats dashboard likewise owns the whole screen, recomputed live from
    // the commits currently loaded for the active timeframe/filter.
    if show_stats {
        let stats = crate::stats::compute_stats(data, filter_by_user, detailed_commit_view, selected_repo_index);
        let overview_count = overview_state.lock_safe().items.len();
        render_stats(f, theme, &stats, &display_interval, filter_by_user, overview_count);
        if show_help { render_help_overlay(f, theme, false); }
        return;
    }

    // Hashes of marked commits, for the "*" indicator in the timeframe view.
    let selected_set: std::collections::BTreeSet<String> = selected_commits
        .map(|arc| arc.lock_safe().set.keys().cloned().collect())
        .unwrap_or_default();
    let bg_fg = theme.text;
    let bg_magenta = theme.detail_border;
    let bg_yellow = theme.text_highlight;

    // Single source of truth for the screen regions (shared with main.rs).
    let layout = compute_layout(f.area(), show_details, selected_commit_index.is_some());
    let sidebar_area = layout.sidebar;
    let commit_area = layout.commit;
    let detail_area = layout.detail;

    // Persistent top bar: view switcher + global chips.
    let overview_count = overview_state.lock_safe().items.len();
    render_top_bar(f, theme, layout.topbar, TopView::Commits, overview_count, &display_interval, filter_by_user);

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
    let total_marked: usize = data.iter()
        .flat_map(|(_, c)| c.iter())
        .filter(|c| selected_set.contains(commit_hash(c)))
        .count();
    // Bulk-selection indicator for a sidebar row: ◉ when all current commits
    // are marked, ◐ when some are, nothing when none. `sel_style` carries the
    // row's reversed highlight so the glyph stays consistent when selected.
    let mark_indicator = |marked: usize, count: usize, sel_style: Option<Style>| -> Option<Span> {
        if count == 0 || marked == 0 {
            return None;
        }
        let (glyph, color) = if marked >= count {
            ("\u{25C9}", Color::Green) // ◉ fully marked
        } else {
            ("\u{25D0}", theme.text_highlight) // ◐ partially marked
        };
        let style = sel_style.unwrap_or_else(|| Style::default().fg(color).add_modifier(Modifier::BOLD));
        Some(Span::styled(format!("  {glyph}"), style))
    };
    // 'All' entry
    let all_selected = selected_repo_index == usize::MAX;
    let all_style = if all_selected {
        Style::default().fg(bg_yellow).add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default().fg(bg_fg).add_modifier(Modifier::BOLD)
    };
    let mut all_spans = vec![
        Span::styled("\u{1F30D} All Projects", all_style), // 🌍
        Span::styled(format!("  {}", total_commits), Style::default().fg(theme.text_secondary)),
    ];
    if let Some(span) = mark_indicator(total_marked, total_commits, all_selected.then_some(all_style)) {
        all_spans.push(span);
    }
    repo_list.push(ListItem::new(Line::from(all_spans)));
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
            let repo_commits = data.iter().find(|(r,_)| r == *repo).map(|(_,c)| c.as_slice()).unwrap_or(&[]);
            let count = repo_commits.len();
            let marked = repo_commits.iter().filter(|c| selected_set.contains(commit_hash(c))).count();
            let (name_style, count_style) = if selected {
                let s = Style::default().fg(bg_yellow).add_modifier(Modifier::BOLD | Modifier::REVERSED);
                (s, s)
            } else if count > 0 {
                (theme.repo_path, theme.repo_commit_count)
            } else {
                let dim = Style::default().fg(theme.blurred_border).add_modifier(Modifier::DIM);
                (dim, dim)
            };
            let mut spans = vec![
                Span::styled(format!("\u{1F5C3} {}", name), name_style), // 🗃️
                Span::styled(format!("  {}", count), count_style),
            ];
            if let Some(span) = mark_indicator(marked, count, selected.then_some(name_style)) {
                spans.push(span);
            }
            repo_list.push(ListItem::new(Line::from(spans)));
        }
    }
    let sidebar = List::new(repo_list)
        .style(Style::default().fg(bg_fg));
    let mut sidebar_state = ListState::default();
    // "All" is item 0, the divider item 1, so repo i is item i + 2. Selecting
    // the right item also lets the List auto-scroll to keep it visible.
    sidebar_state.select(Some(if selected_repo_index == usize::MAX { 0 } else { selected_repo_index + 2 }));
    let sidebar_block = focus_block("Repositories", focus == FocusArea::Sidebar, theme);
    f.render_stateful_widget(sidebar.block(sidebar_block), sidebar_chunks[0], &mut sidebar_state);

    // Commit list layout with scrollbar
    let commit_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
        .split(commit_area);

    // Segment toggle for the commit list mode (switched with `s`), styled like a
    // pair of tabs. The top-level view switch lives in the top bar, not here.
    let tab_titles = ["Timeframe", "Selection"];
    let tabs = ratatui::widgets::Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title("Mode (s)"))
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
                // The list interleaves a repo-header row before each repo's
                // commits, but `selected_commit_index` counts only commits — map
                // it to the actual list-item index so the highlight lands right.
                let mut selected_item = None;
                let mut global = 0usize;
                for (repo, commits) in data {
                    items.push(ListItem::new(Line::from(vec![Span::styled(
                        format!("\u{1F5C3}  {}", repo.file_name().unwrap_or_default().to_string_lossy()),
                        theme.repo_commit_count
                    )])));
                    for commit in commits.iter() {
                        if Some(global) == selected_commit_index { selected_item = Some(items.len()); }
                        let star = if selected_set.contains(commit_hash(commit)) {"*"} else {" "};
                        let item_lines = commit_item_lines(commit, star.to_string(), filter_by_user, detailed_commit_view, theme);
                        items.push(ListItem::new(item_lines).style(Style::default().fg(bg_fg)));
                        global += 1;
                    }
                }
                let mut state = ListState::default(); state.select(selected_item);
                let list = List::new(items)
                    .block(focus_block(&header, focus==FocusArea::CommitList, theme))
                    .highlight_symbol(commit_highlight_symbol(theme))
                    .highlight_style(commit_highlight_style(theme));
                f.render_stateful_widget(list, list_area, &mut state);
                // scrollbar
                let total: usize = data.iter().map(|(_,c)|c.len()).sum();
                let pos = selected_commit_index.unwrap_or(0);
                let mut sb = ScrollbarState::default().position(pos).content_length(total);
                f.render_stateful_widget(Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight), commit_layout[1], &mut sb);
            } else if let Some((_repo, commits)) = data.get(selected_repo_index) {
                let items: Vec<ListItem> = commits.iter().map(|commit| {
                    let star = if selected_set.contains(commit_hash(commit)) {"*"} else {" "};
                    let item_lines = commit_item_lines(commit, star.to_string(), filter_by_user, detailed_commit_view, theme);
                    ListItem::new(item_lines).style(Style::default().fg(bg_fg))
                }).collect();
                let mut state=ListState::default(); state.select(selected_commit_index);
                let list = List::new(items)
                    .block(focus_block(&header, focus==FocusArea::CommitList, theme))
                    .highlight_symbol(commit_highlight_symbol(theme))
                    .highlight_style(commit_highlight_style(theme));
                f.render_stateful_widget(list, list_area, &mut state);
                // scrollbar
                let total=commits.len();
                let pos=selected_commit_index.unwrap_or(0);
                let mut sb=ScrollbarState::default().position(pos).content_length(total);
                f.render_stateful_widget(Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight), commit_layout[1], &mut sb);
            } else {
                // No repo at selected_repo_index, show placeholder
                let placeholder = Paragraph::new("No commits found.")
                    .block(Block::default().title(header).borders(Borders::ALL).border_type(BorderType::Rounded))
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
                        .block(Block::default().title("Selected Commits").borders(Borders::ALL).border_type(BorderType::Rounded))
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
                    // Same header-interleave as the "All" list: map the commit
                    // cursor to the real list-item index for the highlight.
                    let mut selected_item = None;
                    let mut c = 0usize;
                    for (repo, commits) in repo_to_commits.iter() {
                        items.push(ListItem::new(Line::from(vec![Span::styled(
                            format!("\u{1F5C3}  {}", repo.file_name().unwrap_or_default().to_string_lossy()),
                            theme.repo_commit_count
                        )])));
                        for commit in commits.iter() {
                            if Some(c) == selected_commit_index { selected_item = Some(items.len()); }
                            let style = Style::default().fg(theme.selection_fg).add_modifier(Modifier::BOLD);
                            let line = render_commit_line(commit, "*".to_string(), filter_by_user, detailed_commit_view, theme);
                            items.push(ListItem::new(line).style(style));
                            c += 1;
                        }
                    }
                    let mut state = ListState::default(); state.select(selected_item);
                    let list = List::new(items)
                        .block(focus_block("Selected Commits", focus==FocusArea::CommitList, theme))
                        .highlight_symbol(commit_highlight_symbol(theme))
                        .highlight_style(commit_highlight_style(theme));
                    f.render_stateful_widget(list, list_area, &mut state);
                }
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
                    .border_type(BorderType::Rounded)
                    .padding(Padding::horizontal(1))
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

    // footer — universal navigation keys first (always the same), then the
    // actions relevant to the focused pane. Timeframe/filter state lives in the
    // top bar now, so the footer is purely about what you can press here.
    let footer_block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded);
    {
        let universal = "Tab focus \u{00B7} 1/2 views \u{00B7} ? help \u{00B7} q quit";
        let keys = match focus {
            FocusArea::Sidebar =>
                "\u{2191}/\u{2193} repo \u{00B7} \u{2192} commits \u{00B7} m mark repo \u{00B7} [ ] timeframe \u{00B7} u mine/all \u{00B7} a summary",
            FocusArea::CommitList =>
                "\u{2191}/\u{2193} commit \u{00B7} Space detail \u{00B7} m mark \u{00B7} s mode \u{00B7} [ ] timeframe \u{00B7} u mine/all \u{00B7} d detailed \u{00B7} a summary",
            FocusArea::Detail =>
                "\u{2191}/\u{2193} scroll \u{00B7} \u{2190} back \u{00B7} Space close \u{00B7} a summary",
        };
        let line = Line::from(vec![
            Span::styled(format!(" {universal} "), Style::default().fg(theme.text_secondary).add_modifier(Modifier::BOLD)),
            Span::styled("\u{2502} ", Style::default().fg(theme.blurred_border)), // │
            Span::styled(keys, theme.footer),
        ]);
        let footer = Paragraph::new(line).block(footer_block);
        f.render_widget(footer, layout.footer);
    }

    if show_help { render_help_overlay(f, theme, false); }
}

/// Renders the overview view: a left list of stored AI overviews (newest
/// first), a right detail pane with a metadata header and the scrollable text,
/// and a context footer. Fully keyboard-driven.
#[allow(clippy::too_many_arguments)]
fn render_overview(
    f: &mut Frame,
    theme: &Theme,
    overview_state: &Arc<Mutex<OverviewState>>,
    selected: usize,
    overview_focus: OverviewFocus,
    detail_scroll: u16,
    interval_label: &str,
    filter_by_user: bool,
) {
    let state = overview_state.lock_safe();

    let spinner = {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        frames[(state.spinner_frame as usize) % frames.len()]
    };

    // Vertical layout: shared top bar (1 line), optional banner, content, footer.
    let mut constraints = vec![Constraint::Length(1)];
    if state.generating {
        constraints.push(Constraint::Length(3)); // banner
    }
    constraints.push(Constraint::Min(1)); // content
    constraints.push(Constraint::Length(3)); // footer
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(f.area());
    let topbar_area = rows[0];
    let (banner_area, content, footer_area) = if state.generating {
        (Some(rows[1]), rows[2], rows[3])
    } else {
        (None, rows[1], rows[2])
    };

    // Shared top bar (Overviews active here).
    render_top_bar(f, theme, topbar_area, TopView::Overviews, state.items.len(), interval_label, filter_by_user);

    // --- Generating banner (prominent, animated) ---
    if let Some(banner) = banner_area {
        let project = state
            .last_request
            .as_ref()
            .map(|(_, _, _, m)| m.project.clone())
            .unwrap_or_else(|| "your commits".to_string());
        let text = format!("\u{1F916} {}  Generating overview for {}…", spinner, project); // 🤖
        let para = Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(theme.text_highlight)))
            .style(Style::default().fg(Color::Black).bg(theme.text_highlight).add_modifier(Modifier::BOLD));
        f.render_widget(para, banner);
    }

    // Master/detail columns.
    let list_w = (content.width / 3).clamp(28, 44);
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(list_w), Constraint::Min(1)])
        .split(content);
    let list_area = columns[0];
    let detail_area = columns[1];

    let list_focused = overview_focus == OverviewFocus::List;
    let detail_focused = overview_focus == OverviewFocus::Detail;
    let border = |focused: bool| if focused {
        Style::default().fg(theme.focus_border).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.focus_border)
    };

    // --- Left: list of overviews ---
    let mut items: Vec<ListItem> = Vec::new();
    for rec in state.items.iter() {
        let line1 = Line::from(vec![
            Span::styled(rec.project.clone(), Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
        ]);
        let line2 = Line::from(vec![
            Span::styled(rec.created_at.clone(), theme.commit_datetime),
            Span::raw("  "),
            Span::styled(rec.interval.clone(), Style::default().fg(theme.text_secondary)),
        ]);
        items.push(ListItem::new(vec![line1, line2]));
    }
    if items.is_empty() {
        let msg = if state.generating { "Generating your first overview…" } else { "No overviews yet. Press 'a' in the commit view." };
        items.push(ListItem::new(Line::from(vec![Span::styled(
            msg,
            Style::default().fg(theme.text_secondary),
        )])));
    }
    let mut list_state = ListState::default();
    if !state.items.is_empty() {
        list_state.select(Some(selected));
    }
    let list = List::new(items)
        .block(Block::default().title("Overviews").borders(Borders::ALL).border_type(BorderType::Rounded).padding(Padding::horizontal(1)).style(border(list_focused)))
        .highlight_style(Style::default().bg(theme.selection_bg).fg(theme.selection_fg).add_modifier(Modifier::BOLD));
    f.render_stateful_widget(list, list_area, &mut list_state);

    // --- Right: detail pane ---
    let detail_block = Block::default().title("Detail").borders(Borders::ALL).border_type(BorderType::Rounded).padding(Padding::horizontal(1)).style(border(detail_focused));
    let inner = detail_block.inner(detail_area);
    f.render_widget(detail_block, detail_area);

    let selected_rec = state.items.get(selected);
    if let Some(rec) = selected_rec {
        // Metadata header (fixed) above the scrollable text.
        let header_lines = vec![
            Line::from(vec![
                Span::styled("\u{1F4C1} ", Style::default().fg(theme.text_highlight)),
                Span::styled(rec.project.clone(), Style::default().fg(theme.text_highlight).add_modifier(Modifier::BOLD)),
                Span::raw("   "),
                Span::styled(format!("{} commits", rec.commit_count), Style::default().fg(theme.text_secondary)),
            ]),
            Line::from(vec![
                Span::styled(format!("\u{1F551} {}", rec.created_at), theme.commit_datetime),
                Span::raw("   "),
                Span::styled(format!("\u{23F1} {}  ({} to {})", rec.interval, rec.from, rec.to), Style::default().fg(theme.text_secondary)),
            ]),
            Line::from(vec![
                Span::styled(format!("\u{1F916} {} / {}", rec.provider, rec.model), Style::default().fg(theme.text_secondary)),
                Span::raw("   "),
                Span::styled(format!("\u{1F310} {}  \u{00B7} {}", rec.lang, rec.tab), Style::default().fg(theme.text_secondary)),
            ]),
            Line::from(vec![Span::styled(
                "\u{2500}".repeat(inner.width.saturating_sub(1).max(1) as usize),
                Style::default().fg(theme.blurred_border),
            )]),
        ];
        let header_height = header_lines.len() as u16;
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(header_height), Constraint::Min(1)])
            .split(inner);
        f.render_widget(Paragraph::new(header_lines), rows[0]);

        // Body: full overview text + scrollbar.
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(rows[1]);
        let para = Paragraph::new(rec.text.clone())
            .wrap(Wrap { trim: false })
            .scroll((detail_scroll, 0))
            .style(Style::default().fg(theme.text));
        f.render_widget(para, body[0]);
        let line_count = rec.text.lines().count();
        let mut sb = ScrollbarState::default().position(detail_scroll as usize).content_length(line_count);
        f.render_stateful_widget(Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight), body[1], &mut sb);
    } else if let Some(transient) = &state.transient {
        // No record selected yet (e.g. first generation in flight) — show the
        // transient loading/error text.
        let prefix = if state.generating { format!("{} ", spinner) } else { String::new() };
        let para = Paragraph::new(format!("{}{}", prefix, transient))
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(theme.text));
        f.render_widget(para, inner);
    }

    // --- Footer ---
    let footer_text = if state.pending_delete {
        "Delete this overview?  y = yes  \u{00B7}  n / Esc = cancel".to_string()
    } else if state.copied {
        "\u{2713} Copied to clipboard".to_string()
    } else {
        "\u{2191}/\u{2193} select \u{00B7} \u{2190}/\u{2192} focus \u{00B7} c copy \u{00B7} r regenerate \u{00B7} x delete \u{00B7} Esc back \u{00B7} ? help".to_string()
    };
    let footer_style = if state.pending_delete {
        Style::default().fg(Color::Black).bg(Color::Red).add_modifier(Modifier::BOLD)
    } else if state.copied {
        Style::default().fg(theme.commit_author.fg.unwrap_or(Color::Green)).add_modifier(Modifier::BOLD)
    } else {
        theme.footer
    };
    let footer = Paragraph::new(footer_text)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded))
        .style(footer_style);
    f.render_widget(footer, footer_area);
}

/// Full-screen statistics dashboard. Pure painter: all aggregates are computed
/// upstream by `stats::compute_stats` from the live commit data, so the charts
/// reflect the active timeframe/filter and update on every redraw.
fn render_stats(
    f: &mut Frame,
    theme: &Theme,
    stats: &crate::stats::CommitStats,
    interval_label: &str,
    filter_by_user: bool,
    overview_count: usize,
) {
    f.render_widget(Block::default().style(Style::default().bg(theme.root_bg)), f.area());

    // Top bar (1) + content + footer (3), like the overview view.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(3)])
        .split(f.area());
    render_top_bar(f, theme, rows[0], TopView::Stats, overview_count, interval_label, filter_by_user);
    let content = rows[1];
    let footer_area = rows[2];

    // Footer hint (mirrors the overview footer styling).
    let footer = Paragraph::new(
        "1/2/3 views \u{00B7} [ ] timeframe \u{00B7} u mine/all \u{00B7} d detailed \u{00B7} ? help \u{00B7} q quit",
    )
    .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded))
    .style(theme.footer);
    f.render_widget(footer, footer_area);

    // A rounded, unfocused pane used for every chart box.
    let pane = |title: &str| Block::default()
        .title(format!("  {title}"))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(Padding::horizontal(1))
        .border_style(Style::default().fg(theme.blurred_border))
        .title_style(Style::default().fg(theme.text_secondary));

    // Vertical split: number cards, the daily trend, then the per-X trio.
    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Percentage(45), Constraint::Min(6)])
        .split(content);

    // --- Number cards ---
    let card_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 5); 5])
        .split(body[0]);
    let busiest = stats
        .busiest_day
        .as_ref()
        .map(|(d, c)| format!("{} ({})", &d[d.len().saturating_sub(5)..], c))
        .unwrap_or_else(|| "–".to_string());
    let cards: [(&str, String); 5] = [
        ("Commits", stats.total_commits.to_string()),
        ("Active days", stats.active_days.to_string()),
        ("Avg / day", format!("{:.1}", stats.avg_per_active_day)),
        ("Busiest", busiest),
        ("Repos", stats.repo_count.to_string()),
    ];
    for (area, (title, value)) in card_areas.iter().zip(cards.iter()) {
        let block = pane(title);
        let inner = block.inner(*area);
        f.render_widget(block, *area);
        let para = Paragraph::new(Line::from(Span::styled(
            value.clone(),
            Style::default().fg(theme.text_highlight).add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center);
        // Vertically centre the number within the card body.
        let vcenter = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1), Constraint::Min(0)])
            .split(inner);
        f.render_widget(para, vcenter[1]);
    }

    // Empty timeframe: cards already show zeros; replace the charts with a hint.
    if stats.is_empty() {
        let block = pane("Commits per day");
        let inner = block.inner(body[1]);
        f.render_widget(block, body[1]);
        let hint = Paragraph::new("No commits in this timeframe. Press [ ] to change the window.")
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme.text_secondary));
        f.render_widget(hint, inner);
        return;
    }

    // --- Commits per day: BarChart, or Sparkline when too many days to fit ---
    {
        let block = pane("Commits per day");
        let inner = block.inner(body[1]);
        f.render_widget(block, body[1]);
        let n = stats.per_day.len();
        if n * 2 > inner.width as usize {
            // Too many bars to label — fall back to a compact trend line.
            let data: Vec<u64> = stats.per_day.iter().map(|(_, c)| *c).collect();
            let spark = Sparkline::default()
                .data(data)
                .style(Style::default().fg(theme.focus_border));
            f.render_widget(spark, inner);
        } else {
            // "MM-DD" labels; build owned strings the BarChart can borrow.
            let labels: Vec<String> = stats
                .per_day
                .iter()
                .map(|(d, _)| d[d.len().saturating_sub(5)..].to_string())
                .collect();
            let data: Vec<(&str, u64)> = labels
                .iter()
                .zip(stats.per_day.iter())
                .map(|(l, (_, c))| (l.as_str(), *c))
                .collect();
            let bar_width = ((inner.width as usize / n.max(1)).saturating_sub(1)).clamp(1, 6) as u16;
            let chart = BarChart::default()
                .data(&data)
                .bar_width(bar_width)
                .bar_gap(1)
                .bar_style(Style::default().fg(theme.focus_border))
                .value_style(Style::default().fg(theme.root_bg).bg(theme.focus_border).add_modifier(Modifier::BOLD))
                .label_style(Style::default().fg(theme.text_secondary));
            f.render_widget(chart, inner);
        }
    }

    // --- Trio: weekday, hour, repo/authors ---
    let trio = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(34)])
        .split(body[2]);

    // Weekday bars (Mon–Sun).
    {
        let block = pane("Per weekday");
        let inner = block.inner(trio[0]);
        f.render_widget(block, trio[0]);
        let names = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
        let data: Vec<(&str, u64)> = names.iter().zip(stats.per_weekday.iter()).map(|(n, c)| (*n, *c)).collect();
        let bar_width = ((inner.width as usize / 7).saturating_sub(1)).clamp(1, 5) as u16;
        let chart = BarChart::default()
            .data(&data)
            .bar_width(bar_width)
            .bar_gap(1)
            .bar_style(Style::default().fg(theme.detail_border))
            .value_style(Style::default().fg(theme.root_bg).bg(theme.detail_border).add_modifier(Modifier::BOLD))
            .label_style(Style::default().fg(theme.text_secondary));
        f.render_widget(chart, inner);
    }

    // Per-hour activity (0–23) as a sparkline — too many bars to label.
    {
        let block = pane("Per hour (0\u{2013}23)");
        let inner = block.inner(trio[1]);
        f.render_widget(block, trio[1]);
        let data: Vec<u64> = stats.per_hour.to_vec();
        let spark = Sparkline::default()
            .data(data)
            .style(Style::default().fg(theme.detail_border));
        // Reserve the bottom line for a 0 / 6 / 12 / 18 / 23 axis hint.
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);
        f.render_widget(spark, split[0]);
        f.render_widget(
            Paragraph::new("0      6      12      18    23").style(Style::default().fg(theme.text_secondary)),
            split[1],
        );
    }

    // Per-repo distribution + top authors as text rows with mini bars.
    {
        let block = pane("Per repo \u{00B7} authors");
        let inner = block.inner(trio[2]);
        f.render_widget(block, trio[2]);
        let max_count = stats.per_repo.iter().map(|(_, c)| *c).max().unwrap_or(1).max(1);
        let bar_cells = 10usize;
        let mut lines: Vec<Line> = Vec::new();
        for (name, count) in stats.per_repo.iter().take(5) {
            let filled = ((*count as usize * bar_cells) / max_count as usize).max(1);
            let bar: String = "\u{2588}".repeat(filled) + &"\u{2591}".repeat(bar_cells - filled);
            let short: String = name.chars().take(14).collect();
            lines.push(Line::from(vec![
                Span::styled(format!("{:<14} ", short), Style::default().fg(theme.text)),
                Span::styled(bar, Style::default().fg(theme.focus_border)),
                Span::styled(format!(" {count}"), theme.repo_commit_count),
            ]));
        }
        lines.push(Line::from(Span::styled(
            "\u{2500}".repeat(inner.width.max(1) as usize),
            Style::default().fg(theme.blurred_border),
        )));
        if stats.per_author.is_empty() {
            lines.push(Line::from(Span::styled(
                "Authors: press u (all) or d (detailed)",
                Style::default().fg(theme.text_secondary),
            )));
        } else {
            for (name, count) in stats.per_author.iter().take(5) {
                let short: String = name.chars().take(20).collect();
                lines.push(Line::from(vec![
                    Span::styled(format!("{:<20} ", short), theme.commit_author),
                    Span::styled(format!("{count}"), theme.repo_commit_count),
                ]));
            }
        }
        f.render_widget(Paragraph::new(lines), inner);
    }
}

/// Screen regions for the main view, computed once so rendering and mouse
/// hit-testing agree on the exact same rectangles.
pub struct AppLayout {
    pub topbar: Rect,
    pub sidebar: Rect,
    pub commit: Rect,
    pub detail: Option<Rect>,
    pub tabs: Rect,
    pub footer: Rect,
}

/// Computes the main layout: a 1-line top bar (view switcher + global chips),
/// the content row, and a 3-line footer. The detail column only appears when
/// the detail pane is toggled on and a commit is selected.
pub fn compute_layout(area: Rect, show_details: bool, has_selection: bool) -> AppLayout {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(3)])
        .split(area);
    let topbar = vertical[0];
    let content = vertical[1];
    // Responsive sidebar: ~1/4 of the width, clamped so it stays readable on
    // narrow terminals and doesn't waste space on wide ones.
    let sidebar_w = (area.width / 4).clamp(22, 36);
    let columns = if show_details && has_selection {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(sidebar_w), Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(content)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(sidebar_w), Constraint::Min(1)])
            .split(content)
    };
    let sidebar = columns[0];
    let commit = columns[1];
    let detail = if columns.len() > 2 { Some(columns[2]) } else { None };
    let tabs = Rect { x: commit.x, y: commit.y, width: commit.width, height: 3 };
    AppLayout { topbar, sidebar, commit, detail, tabs, footer: vertical[2] }
}

/// The two top-level views, for the top bar's switcher.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TopView {
    Commits,
    Overviews,
    Stats,
}

/// Renders the persistent top bar: the view switcher on the left and global
/// status chips on the right. Shown identically in both views so the user
/// always knows where they are and how to switch.
fn render_top_bar(
    f: &mut Frame,
    theme: &Theme,
    area: Rect,
    active: TopView,
    overview_count: usize,
    interval_label: &str,
    filter_by_user: bool,
) {
    // A view entry: filled when active, dimmed when disabled, plain otherwise.
    let view_chip = |label: &str, key: &str, is_active: bool, disabled: bool| {
        let text = format!(" {label} {key} ");
        let style = if disabled {
            Style::default().fg(theme.blurred_border).add_modifier(Modifier::DIM)
        } else if is_active {
            Style::default().fg(Color::Black).bg(theme.focus_border).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
        };
        Span::styled(text, style)
    };
    let left = Line::from(vec![
        view_chip("\u{25A3} Commits", "1", active == TopView::Commits, false), // ▣
        Span::raw(" "),
        view_chip("\u{25A2} Overviews", "2", active == TopView::Overviews, overview_count == 0), // ▢
        Span::raw(" "),
        view_chip("\u{25A4} Stats", "3", active == TopView::Stats, false), // ▤
    ]);

    let filter = if filter_by_user { "\u{25C9} mine" } else { "\u{25C9} all" }; // ◉
    let right = Line::from(vec![
        Span::styled(format!("\u{23F1} {} ", interval_label), Style::default().fg(theme.text_highlight)), // ⏱
        Span::styled("\u{00B7} ", Style::default().fg(theme.blurred_border)),
        Span::styled(format!("{} ", filter), Style::default().fg(theme.text_secondary)),
        Span::styled("\u{00B7} ", Style::default().fg(theme.blurred_border)),
        Span::styled("? help ", Style::default().fg(theme.text_secondary)),
    ]).right_aligned();

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(28)])
        .split(area);
    f.render_widget(Paragraph::new(left), cols[0]);
    f.render_widget(Paragraph::new(right), cols[1]);
}

/// A bordered block whose border + title reflect focus: the focused pane gets a
/// bright bold border and a `▸` marker so the active area is unmistakable.
fn focus_block(title: &str, focused: bool, theme: &Theme) -> Block<'static> {
    let (marker, border_style) = if focused {
        ("\u{25B8} ", Style::default().fg(theme.focus_border).add_modifier(Modifier::BOLD)) // ▸
    } else {
        ("  ", Style::default().fg(theme.blurred_border))
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(Padding::horizontal(1))
        .title(format!("{marker}{title}"))
        .border_style(border_style)
}

/// The styled cursor symbol drawn in front of the selected commit row. Uses
/// ratatui 0.30's `Into<Line>` highlight symbol so the marker carries its own
/// color; non-selected rows get an equal-width blank, keeping columns aligned.
fn commit_highlight_symbol(theme: &Theme) -> Line<'static> {
    Line::from(Span::styled(
        "\u{25B6} ", // ▶
        Style::default().fg(theme.selection_fg).add_modifier(Modifier::BOLD),
    ))
}

/// The row style applied to the selected commit (background highlight).
fn commit_highlight_style(theme: &Theme) -> Style {
    Style::default()
        .bg(theme.selection_bg)
        .fg(theme.selection_fg)
        .add_modifier(Modifier::BOLD)
}

/// Centers a rectangle of the given size within `area` (clamped to it).
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}

/// Centered help overlay listing all keybindings, grouped by scope. Dismissed
/// by any key (handled in `input`).
fn render_help_overlay(f: &mut Frame, theme: &Theme, for_overview: bool) {
    let area = centered_rect(64, 22, f.area());
    f.render_widget(Clear, area);

    let head = |s: &str| Line::from(Span::styled(s.to_string(), Style::default().fg(theme.text_highlight).add_modifier(Modifier::BOLD)));
    let row = |k: &str, d: &str| Line::from(vec![
        Span::styled(format!("  {:<14}", k), Style::default().fg(theme.focus_border).add_modifier(Modifier::BOLD)),
        Span::styled(d.to_string(), Style::default().fg(theme.text)),
    ]);

    let mut lines = vec![
        head("Global"),
        row("1 / 2 / 3", "switch view: Commits / Overviews / Stats"),
        row("Tab / ⇧Tab", "move focus between panes"),
        row("← → / h l", "move focus left / right"),
        row("↑ ↓ / j k", "navigate / scroll in the focused pane"),
        row("?", "toggle this help"),
        row("q", "quit"),
        Line::raw(""),
    ];
    if for_overview {
        lines.push(head("Overviews"));
        lines.push(row("Enter / c", "copy the selected overview"));
        lines.push(row("r", "regenerate the last overview"));
        lines.push(row("x / Del", "delete (asks y/n to confirm)"));
        lines.push(row("a", "generate a new overview"));
        lines.push(row("Esc", "back to Commits (cancels generation)"));
    } else {
        lines.push(head("Commits"));
        lines.push(row("Space", "open/close the detail pane"));
        lines.push(row("s", "toggle list mode: Timeframe / Selection"));
        lines.push(row("m", "mark commit (or whole repo from sidebar)"));
        lines.push(row("[ / ]", "previous / next timeframe"));
        lines.push(row("u", "toggle mine / all authors"));
        lines.push(row("d", "toggle detailed commit lines"));
        lines.push(row("a", "generate an AI overview"));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Keys — press any key to close ")
        .border_style(Style::default().fg(theme.focus_border).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(theme.root_bg));
    f.render_widget(Paragraph::new(lines).block(block), area);
}