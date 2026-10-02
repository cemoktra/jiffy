use std::fmt;

use serde::{Deserialize, Serialize};
use warp_util::local_or_remote_path::LocalOrRemotePath;

/// An unique reference to a skill.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum SkillReference {
    /// A skill identified by the path to its SKILL.md file.
    Path(LocalOrRemotePath),
}

impl SkillReference {
    /// A user-facing label for status and error copy (e.g. "Failed to read
    /// skill {label}").
    pub fn display_label(&self) -> String {
        match self {
            SkillReference::Path(path) => path.display_path(),
        }
    }
}

impl fmt::Display for SkillReference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SkillReference::Path(path) => path.display_path().fmt(f),
        }
    }
}
