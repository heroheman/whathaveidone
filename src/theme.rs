use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub root_bg: Color,
    pub focus_border: Color,
    pub blurred_border: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub text_highlight: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub dim_bg: Color,

    // Specific components
    pub commit_hash: Style,
    pub commit_datetime: Style,
    pub commit_author: Style,
    pub commit_ticket: Style,
    pub repo_path: Style,
    pub repo_commit_count: Style,
    pub footer: Style,
    pub popup_title: Style,
    pub popup_border: Style,
    pub popup_text: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            root_bg: Color::Black,
            focus_border: Color::Cyan,
            blurred_border: Color::DarkGray,
            text: Color::White,
            text_secondary: Color::Gray,
            text_highlight: Color::Yellow,
            selection_bg: Color::DarkGray,
            selection_fg: Color::Yellow,
            dim_bg: Color::Rgb(30, 30, 30),

            commit_hash: Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
            commit_datetime: Style::default().fg(Color::Magenta),
            commit_author: Style::default().fg(Color::Green),
            commit_ticket: Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            repo_path: Style::default().fg(Color::Cyan),
            repo_commit_count: Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            footer: Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
            popup_title: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            popup_border: Style::default().bg(Color::Black),
            popup_text: Style::default().fg(Color::White),
        }
    }
} 