use std::path::Path;

use anyhow::Result;
use warp_util::jj::{JjSnapshot, run_jj_command};

use super::GitStatusMetadata;
use crate::code_review::diff_state::DiffStats;
use crate::context_chips::display_chip::GitBranchTrackingStatus;
use crate::util::jj::{
    WorkingCopyInfo, bookmark_revset, count_revisions, first_bookmark, remote_bookmark_revset,
    trunk_name, upstream_bookmark, working_copy_info,
};

const FIELD_SEPARATOR: char = '\x1f';
/// `description first line`, `empty marker`, `divergent marker` for `@`.
const PROMPT_TEMPLATE: &str = r#"description.first_line() ++ "\x1f" ++ if(empty, "empty") ++ "\x1f" ++ if(divergent, "divergent") ++ "\n""#;
/// The closest bookmarked ancestor of `@`, including `@` itself.
const NEAREST_BOOKMARK_REVSET: &str = "latest(heads(::@ & bookmarks()))";
const MAX_DESCRIPTION_CHARS: usize = 24;

/// Computes the prompt-chip metadata for a jj workspace. Snapshots the working copy once so the
/// stats include unrecorded edits; every later query reads that snapshot.
pub(super) async fn load_metadata(repo_path: &Path) -> Result<GitStatusMetadata> {
    let working_copy = working_copy_info(repo_path, JjSnapshot::Snapshot).await?;
    let current_branch_name = working_copy.display_name();
    let main_branch_name = trunk_name(repo_path).await;
    let stats_against_head = parse_diff_stat_summary(
        &run_jj_command(repo_path, &["diff", "-r", "@", "--stat"], JjSnapshot::Skip).await?,
    );
    let prompt_label = prompt_label(repo_path, &working_copy).await;
    let branch_tracking_status = match &working_copy.bookmark {
        Some(bookmark) => tracking_status(repo_path, bookmark, &prompt_label).await,
        None => GitBranchTrackingStatus::new(prompt_label.clone(), None, 0, 0),
    };

    Ok(GitStatusMetadata {
        current_branch_name,
        prompt_label,
        main_branch_name,
        stats_against_head,
        branch_tracking_status,
    })
}

async fn tracking_status(
    repo_path: &Path,
    bookmark: &str,
    display_name: &str,
) -> GitBranchTrackingStatus {
    let Some(upstream) = upstream_bookmark(repo_path, bookmark).await else {
        return GitBranchTrackingStatus::new(display_name.to_string(), None, 0, 0);
    };
    let local = bookmark_revset(bookmark);
    let remote = remote_bookmark_revset(bookmark);
    let ahead = count_revisions(repo_path, &format!("({remote})..({local})")).await;
    let behind = count_revisions(repo_path, &format!("({local})..({remote})")).await;
    match (ahead, behind) {
        (Some(ahead), Some(behind)) => {
            GitBranchTrackingStatus::new(display_name.to_string(), Some(upstream), ahead, behind)
        }
        _ => GitBranchTrackingStatus::without_counts(display_name.to_string(), Some(upstream)),
    }
}

/// A starship-jj style summary of `@`, e.g. `main⇡1 "Fix the prompt chip" (empty)`. Falls back
/// to [`WorkingCopyInfo::display_name`] when the details can't be read.
async fn prompt_label(repo_path: &Path, working_copy: &WorkingCopyInfo) -> String {
    let Ok(output) = run_jj_command(
        repo_path,
        &["log", "-r", "@", "--no-graph", "-T", PROMPT_TEMPLATE],
        JjSnapshot::Skip,
    )
    .await
    else {
        return working_copy.display_name();
    };
    let Some(details) = parse_prompt_details(&output) else {
        return working_copy.display_name();
    };
    let nearest_bookmark = match first_bookmark(repo_path, NEAREST_BOOKMARK_REVSET).await {
        Some(bookmark) => {
            let distance =
                count_revisions(repo_path, &format!("({NEAREST_BOOKMARK_REVSET})..@")).await;
            Some((bookmark, distance.unwrap_or_default()))
        }
        None => None,
    };
    format_prompt_label(working_copy, nearest_bookmark, &details)
}

#[derive(Debug, Default, PartialEq, Eq)]
struct PromptDetails {
    description: String,
    is_empty: bool,
    is_divergent: bool,
}

fn parse_prompt_details(output: &str) -> Option<PromptDetails> {
    let mut fields = output.lines().next()?.split(FIELD_SEPARATOR);
    let description = fields.next()?.trim().to_string();
    let is_empty = fields.next().is_some_and(|field| field.trim() == "empty");
    let is_divergent = fields
        .next()
        .is_some_and(|field| field.trim() == "divergent");
    Some(PromptDetails {
        description,
        is_empty,
        is_divergent,
    })
}

/// `nearest_bookmark` is the closest bookmarked ancestor of `@` and how many revisions `@` is
/// ahead of it.
fn format_prompt_label(
    working_copy: &WorkingCopyInfo,
    nearest_bookmark: Option<(String, u32)>,
    details: &PromptDetails,
) -> String {
    let mut label = match nearest_bookmark {
        Some((bookmark, 0)) => bookmark,
        Some((bookmark, distance)) => format!("{bookmark}⇡{distance}"),
        None => working_copy.change_id.clone(),
    };
    if details.description.is_empty() {
        label.push_str(" (no description)");
    } else {
        label.push_str(&format!(" \"{}\"", truncate(&details.description)));
    }
    let markers = [
        (working_copy.has_conflict, "conflict"),
        (details.is_divergent, "divergent"),
        (details.is_empty, "empty"),
    ];
    for (is_set, marker) in markers {
        if is_set {
            label.push_str(&format!(" ({marker})"));
        }
    }
    label
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= MAX_DESCRIPTION_CHARS {
        return text.to_string();
    }
    let truncated: String = text.chars().take(MAX_DESCRIPTION_CHARS - 1).collect();
    format!("{}…", truncated.trim_end())
}

/// Parses the summary line of `jj diff --stat`, e.g.
/// `3 files changed, 10 insertions(+), 2 deletions(-)`.
fn parse_diff_stat_summary(output: &str) -> DiffStats {
    let mut stats = DiffStats::default();
    let Some(summary) = output.lines().rev().find(|line| line.contains("changed")) else {
        return stats;
    };
    for part in summary.split(',') {
        let mut words = part.split_whitespace();
        let Some(count) = words.next().and_then(|count| count.parse().ok()) else {
            continue;
        };
        match words.next() {
            Some(word) if word.starts_with("file") => stats.files_changed = count,
            Some(word) if word.starts_with("insertion") => stats.total_additions = count,
            Some(word) if word.starts_with("deletion") => stats.total_deletions = count,
            _ => {}
        }
    }
    stats
}

#[cfg(test)]
#[path = "jj_tests.rs"]
mod tests;
