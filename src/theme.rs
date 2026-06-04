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
    pub detail_border: Color,

    // Specific components
    pub commit_hash: Style,
    pub commit_datetime: Style,
    pub commit_author: Style,
    pub commit_ticket: Style,
    pub repo_path: Style,
    pub repo_commit_count: Style,
    pub footer: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            root_bg: Color::Rgb(0x16, 0x18, 0x1d),        // near-black, slight blue
            focus_border: Color::Rgb(0x6c, 0xb6, 0xff),   // soft cyan/blue
            blurred_border: Color::Rgb(0x3a, 0x3f, 0x4b), // muted slate
            text: Color::Rgb(0xe4, 0xe6, 0xeb),           // off-white
            text_secondary: Color::Rgb(0x8b, 0x90, 0x9a), // muted gray
            text_highlight: Color::Rgb(0xf2, 0xc9, 0x5d), // warm gold
            selection_bg: Color::Rgb(0x2b, 0x31, 0x3c),   // subtle row highlight
            selection_fg: Color::Rgb(0xf2, 0xc9, 0x5d),
            detail_border: Color::Rgb(0xc5, 0x92, 0xff),  // soft violet

            commit_hash: Style::default().fg(Color::Rgb(0x6c, 0xb6, 0xff)).add_modifier(Modifier::BOLD),
            commit_datetime: Style::default().fg(Color::Rgb(0xc5, 0x92, 0xff)),
            commit_author: Style::default().fg(Color::Rgb(0x7e, 0xe0, 0x9e)),
            commit_ticket: Style::default().fg(Color::Rgb(0xf2, 0xc9, 0x5d)).add_modifier(Modifier::BOLD),
            repo_path: Style::default().fg(Color::Rgb(0x6c, 0xb6, 0xff)),
            repo_commit_count: Style::default().fg(Color::Rgb(0x7e, 0xe0, 0x9e)).add_modifier(Modifier::BOLD),
            footer: Style::default().fg(Color::Rgb(0x8b, 0x90, 0x9a)).add_modifier(Modifier::DIM),
        }
    }
} 