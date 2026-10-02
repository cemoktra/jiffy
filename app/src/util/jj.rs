//! Shared jj (Jujutsu) queries used by Warp's VCS surfaces.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Result, anyhow};
use warp_util::jj::{JjSnapshot, run_jj_command, run_jj_command_blocking, run_jj_command_with_env};

use crate::features::FeatureFlag;
use crate::util::git::BranchEntry;

/// The git remote jj bookmarks are compared against and pushed to.
pub(crate) const REMOTE: &str = "origin";

const FIELD_SEPARATOR: char = '\x1f';

/// `change id`, `local bookmark names`, `conflict marker` for a single revision.
const REVISION_TEMPLATE: &str = r#"change_id.shortest(8) ++ "\x1f" ++ local_bookmarks.map(|b| b.name()).join(",") ++ "\x1f" ++ if(conflict, "conflict") ++ "\n""#;
const BOOKMARKS_TEMPLATE: &str = r#"local_bookmarks.map(|b| b.name()).join(",") ++ "\n""#;
const COUNT_TEMPLATE: &str = r#""x\n""#;

/// Whether `repo_path` is the root of a jj workspace that Warp should drive with jj.
pub(crate) fn is_jj_workspace(repo_path: &Path) -> bool {
    FeatureFlag::JjSupport.is_enabled() && repo_path.join(".jj").join("repo").exists()
}

/// The root of the main workspace when `workspace_root` is a secondary jj workspace, whose
/// `.jj/repo` is a file pointing at the main workspace's store.
pub(crate) fn main_workspace_root(workspace_root: &Path) -> Option<std::path::PathBuf> {
    if !FeatureFlag::JjSupport.is_enabled() {
        return None;
    }
    let jj_dir = workspace_root.join(".jj");
    let store = std::fs::read_to_string(jj_dir.join("repo")).ok()?;
    let store = jj_dir.join(store.trim()).canonicalize().ok()?;
    // `<main>/.jj/repo` -> `<main>`
    store.parent()?.parent().map(Path::to_path_buf)
}

/// Quotes `value` as a revset/fileset string literal.
pub(crate) fn quote_revset_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A revset matching exactly the local bookmark `name`.
pub(crate) fn bookmark_revset(name: &str) -> String {
    format!("bookmarks(exact:{})", quote_revset_string(name))
}

/// A revset matching exactly the bookmark `name` on [`REMOTE`].
pub(crate) fn remote_bookmark_revset(name: &str) -> String {
    format!(
        "remote_bookmarks(exact:{}, exact:{})",
        quote_revset_string(name),
        quote_revset_string(REMOTE)
    )
}

/// A fileset matching exactly the repo-relative `path`.
pub(crate) fn root_file_fileset(path: &str) -> String {
    format!("root-file:{}", quote_revset_string(path))
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct WorkingCopyInfo {
    pub change_id: String,
    /// The first bookmark on `@`, falling back to `@-`.
    pub bookmark: Option<String>,
    pub has_conflict: bool,
}

impl WorkingCopyInfo {
    /// The name shown where git would show the current branch.
    pub fn display_name(&self) -> String {
        let mut name = self
            .bookmark
            .clone()
            .unwrap_or_else(|| self.change_id.clone());
        if self.has_conflict {
            name.push_str(" (conflict)");
        }
        name
    }
}

pub(crate) async fn working_copy_info(
    repo_path: &Path,
    snapshot: JjSnapshot,
) -> Result<WorkingCopyInfo> {
    let output = run_jj_command(
        repo_path,
        &["log", "-r", "@", "--no-graph", "-T", REVISION_TEMPLATE],
        snapshot,
    )
    .await?;
    let (change_id, bookmarks, has_conflict) =
        parse_revision_info(&output).ok_or_else(|| anyhow!("Unexpected jj log output for @"))?;
    let bookmark = match bookmarks.into_iter().next() {
        Some(bookmark) => Some(bookmark),
        None => first_bookmark(repo_path, "@-").await,
    };
    Ok(WorkingCopyInfo {
        change_id,
        bookmark,
        has_conflict,
    })
}

pub(crate) async fn first_bookmark(repo_path: &Path, revset: &str) -> Option<String> {
    let output = run_jj_command(
        repo_path,
        &["log", "-r", revset, "--no-graph", "-T", BOOKMARKS_TEMPLATE],
        JjSnapshot::Skip,
    )
    .await
    .ok()?;
    parse_bookmark_list(output.lines().next()?)
        .into_iter()
        .next()
}

/// The bookmark name of `trunk()`, or `trunk()` itself when it has none.
pub(crate) async fn trunk_name(repo_path: &Path) -> String {
    first_bookmark(repo_path, "trunk()")
        .await
        .unwrap_or_else(|| "trunk()".to_string())
}

pub(crate) async fn count_revisions(repo_path: &Path, revset: &str) -> Option<u32> {
    let output = run_jj_command(
        repo_path,
        &["log", "-r", revset, "--no-graph", "-T", COUNT_TEMPLATE],
        JjSnapshot::Skip,
    )
    .await
    .ok()?;
    u32::try_from(output.lines().count()).ok()
}

/// `<bookmark>@origin` when that remote bookmark exists.
pub(crate) async fn upstream_bookmark(repo_path: &Path, bookmark: &str) -> Option<String> {
    let exists = count_revisions(repo_path, &remote_bookmark_revset(bookmark))
        .await
        .is_some_and(|count| count > 0);
    exists.then(|| format!("{bookmark}@{REMOTE}"))
}

/// Resolves the commit where `@` forked from `branch` (a bookmark name or `trunk()`).
pub(crate) async fn fork_point_commit(repo_path: &Path, branch: &str) -> Result<String> {
    let branch_revset = if branch == "trunk()" {
        branch.to_string()
    } else {
        bookmark_revset(branch)
    };
    let output = run_jj_command(
        repo_path,
        &[
            "log",
            "-r",
            &format!("fork_point(({branch_revset}) | @)"),
            "--no-graph",
            "-T",
            r#"commit_id ++ "\n""#,
        ],
        JjSnapshot::Skip,
    )
    .await?;
    output
        .lines()
        .next()
        .map(str::trim)
        .filter(|commit| !commit.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("No fork point between @ and {branch}"))
}

/// Local bookmarks, most recently changed first, with `trunk()`'s bookmark marked as main.
pub(crate) async fn list_bookmarks(repo_path: &Path) -> Result<Vec<BranchEntry>> {
    let main = trunk_name(repo_path).await;
    let output = run_jj_command(
        repo_path,
        &[
            "bookmark",
            "list",
            "--sort",
            "committer-date-",
            "-T",
            r#"if(!remote, name ++ "\n")"#,
        ],
        JjSnapshot::Skip,
    )
    .await?;
    Ok(output
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| BranchEntry {
            name: name.to_string(),
            is_main: name == main,
        })
        .collect())
}

/// [`list_bookmarks`], falling back to `@-` (git's `HEAD` counterpart) in repos without bookmarks
/// so base-revision pickers always have an entry.
pub(crate) async fn list_bookmarks_or_parent(repo_path: &Path) -> Result<Vec<BranchEntry>> {
    let bookmarks = list_bookmarks(repo_path).await?;
    if !bookmarks.is_empty() {
        return Ok(bookmarks);
    }
    Ok(vec![BranchEntry {
        name: "@-".to_string(),
        is_main: true,
    }])
}

/// Names of all workspaces and local bookmarks, for picking a name that collides with neither.
/// Synchronous: for view handlers that need the result immediately.
pub(crate) fn list_workspace_and_bookmark_names_sync(repo_path: &Path) -> HashSet<String> {
    let queries: [&[&str]; 2] = [
        &["workspace", "list", "-T", r#"name ++ "\n""#],
        &["bookmark", "list", "-T", r#"if(!remote, name ++ "\n")"#],
    ];
    queries
        .into_iter()
        .filter_map(|args| run_jj_command_blocking(repo_path, args, JjSnapshot::Skip).ok())
        .flat_map(|output| {
            output
                .lines()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Describes the working-copy change and starts a new empty one on top (`jj commit`).
pub(crate) async fn commit(
    repo_path: &Path,
    message: &str,
    path_env: Option<&str>,
) -> Result<String> {
    // Snapshot first so the emptiness check sees the latest edits.
    working_copy_info(repo_path, JjSnapshot::Snapshot).await?;
    if count_revisions(repo_path, "@ & empty()")
        .await
        .is_some_and(|count| count > 0)
    {
        anyhow::bail!("nothing to commit, working copy is empty");
    }
    run_jj_command_with_env(
        repo_path,
        &["commit", "-m", message],
        JjSnapshot::Skip,
        path_env,
    )
    .await
}

/// Pushes `bookmark` to [`REMOTE`], first creating it on `@-` (the last committed change) when
/// it doesn't exist yet.
pub(crate) async fn push_bookmark(
    repo_path: &Path,
    bookmark: &str,
    path_env: Option<&str>,
) -> Result<String> {
    let exists = count_revisions(repo_path, &bookmark_revset(bookmark))
        .await
        .is_some_and(|count| count > 0);
    // Without a bookmark the caller only knows the change id; name the bookmark the way
    // `jj git push --change` would instead of after the bare id.
    let bookmark = if !exists && is_change_id(bookmark) {
        format!("push-{bookmark}")
    } else {
        bookmark.to_string()
    };
    let bookmark = bookmark.as_str();
    if !exists {
        run_jj_command(
            repo_path,
            &["bookmark", "create", bookmark, "-r", "@-"],
            JjSnapshot::Skip,
        )
        .await?;
    }
    run_jj_command_with_env(
        repo_path,
        &["git", "push", "--remote", REMOTE, "-b", bookmark],
        JjSnapshot::Skip,
        path_env,
    )
    .await
}

/// Whether `value` looks like a jj change id, which uses the letters `k`-`z` only.
fn is_change_id(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| ('k'..='z').contains(&c))
}

/// The git patch of the working-copy change, for AI commit messages.
pub(crate) async fn working_copy_patch(repo_path: &Path) -> Result<String> {
    run_jj_command(
        repo_path,
        &["diff", "--git", "-r", "@"],
        JjSnapshot::Snapshot,
    )
    .await
}

/// The bookmark on `@` or `@-`, if any.
pub(crate) async fn working_copy_bookmark(repo_path: &Path) -> Option<String> {
    working_copy_info(repo_path, JjSnapshot::Skip)
        .await
        .ok()
        .and_then(|info| info.bookmark)
}

/// The revision a PR is opened for: the working-copy bookmark, else `@-`.
async fn pr_head_revset(repo_path: &Path) -> String {
    match working_copy_info(repo_path, JjSnapshot::Skip).await {
        Ok(WorkingCopyInfo {
            bookmark: Some(bookmark),
            ..
        }) => bookmark_revset(&bookmark),
        _ => "@-".to_string(),
    }
}

/// The git patch of committed changes since the fork point with `trunk()`, for AI PR content.
pub(crate) async fn branch_patch(repo_path: &Path) -> Result<String> {
    let trunk = trunk_name(repo_path).await;
    let fork_point = fork_point_commit(repo_path, &trunk).await?;
    let head = pr_head_revset(repo_path).await;
    run_jj_command(
        repo_path,
        &["diff", "--git", "--from", &fork_point, "--to", &head],
        JjSnapshot::Skip,
    )
    .await
}

/// First description lines of the committed changes since `trunk()`.
pub(crate) async fn branch_commit_messages(repo_path: &Path) -> Result<Vec<String>> {
    let head = pr_head_revset(repo_path).await;
    let output = run_jj_command(
        repo_path,
        &[
            "log",
            "-r",
            &format!("trunk()..({head})"),
            "--no-graph",
            "-T",
            r#"description.first_line() ++ "\n""#,
        ],
        JjSnapshot::Skip,
    )
    .await?;
    Ok(output
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

fn parse_revision_info(output: &str) -> Option<(String, Vec<String>, bool)> {
    let mut fields = output.lines().next()?.split(FIELD_SEPARATOR);
    let change_id = fields.next()?.trim().to_string();
    if change_id.is_empty() {
        return None;
    }
    let bookmarks = parse_bookmark_list(fields.next().unwrap_or_default());
    let has_conflict = fields
        .next()
        .is_some_and(|field| field.trim() == "conflict");
    Some((change_id, bookmarks, has_conflict))
}

fn parse_bookmark_list(line: &str) -> Vec<String> {
    line.split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::Path;

    /// Runs jj in `dir` with a fixed identity, panicking on failure.
    pub(crate) fn run_jj(dir: &Path, args: &[&str]) -> String {
        let output = command::blocking::Command::new("jj")
            .args(args)
            .current_dir(dir)
            .env("JJ_USER", "Test")
            .env("JJ_EMAIL", "test@example.com")
            .output()
            .unwrap();
        assert!(output.status.success(), "jj {args:?} failed: {output:?}");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// Creates a colocated jj repo in a temp dir, or returns `None` when jj isn't installed.
    pub(crate) fn init_test_repo() -> Option<tempfile::TempDir> {
        if command::blocking::Command::new("jj")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("Skipping: jj is not installed");
            return None;
        }
        let temp_dir = tempfile::tempdir().unwrap();
        run_jj(temp_dir.path(), &["git", "init", "--colocate"]);
        Some(temp_dir)
    }
}

#[cfg(test)]
#[path = "jj_tests.rs"]
mod tests;
