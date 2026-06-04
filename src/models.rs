use std::collections::BTreeMap;
use std::path::PathBuf;

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
///
/// Keyed by commit hash and storing the owning repo plus the full commit line
/// captured at mark time, so a marked commit survives timeframe changes even
/// when it is no longer part of the currently loaded commit set. Ordered by
/// hash for deterministic display.
#[derive(Debug)]
pub struct SelectedCommits {
    pub set: BTreeMap<String, (PathBuf, String)>,
    pub popup_visible: bool,
}