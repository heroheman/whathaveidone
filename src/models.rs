use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FocusArea {
    Sidebar,
    CommitList,
    Detail,
}

pub struct PopupQuote {
    pub visible: bool,
    pub text: String,
    pub loading: bool,
}

pub struct SelectedCommits {
    pub set: HashSet<String>,
    pub popup_visible: bool,
}