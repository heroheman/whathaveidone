use std::collections::HashSet;

/// Which UI area is currently focused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FocusArea {
    Sidebar,
    CommitList,
    Detail,
}

/// State for the AI quote popup.
#[derive(Debug)]
pub struct PopupQuote {
    pub visible: bool,
    pub text: String,
    pub loading: bool,
    pub scroll: u16, // scroll offset for popup summary
    pub spinner_frame: u8, // frame index for loading spinner
}

/// State for selected/marked commits.
#[derive(Debug)]
pub struct SelectedCommits {
    pub set: HashSet<String>,
    pub popup_visible: bool,
}