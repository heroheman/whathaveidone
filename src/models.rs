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