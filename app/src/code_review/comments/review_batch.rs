//! Types describing a code review submission that is sent to a CLI agent.
use std::collections::HashMap;
use std::ops::Range;

use serde::{Deserialize, Serialize};
use warp_editor::render::model::LineCount;

use super::AttachedReviewComment;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurrentHead {
    BranchName(String),
    HeadlessCommitSha(String),
}

impl CurrentHead {
    pub fn title(&self) -> String {
        match self {
            CurrentHead::BranchName(name) => name.clone(),
            CurrentHead::HeadlessCommitSha(sha) => {
                let short = sha.chars().take(7).collect::<String>();
                format!("Commit {short}")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffBase {
    BranchName(String),
    HeadlessCommitSha(String),
    UncommittedChanges,
}

/// A simplified diff hunk describing the changed lines a review comment refers to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffSetHunk {
    pub line_range: Range<LineCount>,
    pub diff_content: String,
    pub lines_added: u32,
    pub lines_removed: u32,
}

/// A batch of review comments submitted to a CLI agent in a single prompt.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentReviewCommentBatch {
    pub comments: Vec<AttachedReviewComment>,
    /// All diff hunks that have comments in this batch attached to them, grouped by file name.
    pub diff_set: HashMap<String, Vec<DiffSetHunk>>,
}
