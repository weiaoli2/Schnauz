//! Per-tab file-type visibility, also stored in sessions and workspaces.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EntryFilter {
    #[default]
    All,
    Folders,
    Files,
}

impl EntryFilter {
    pub fn accepts(self, is_dir: bool) -> bool {
        match self {
            Self::All => true,
            Self::Folders => is_dir,
            Self::Files => !is_dir,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Folders => "Folders",
            Self::Files => "Files",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::All => "All items",
            Self::Folders => "Folders only",
            Self::Files => "Files only",
        }
    }
}
