use std::fs;

use super::*;
use crate::util::jj::test_support::{init_test_repo, run_jj};

#[test]
fn parse_changed_file_maps_statuses() {
    assert_eq!(
        parse_changed_file("renamed\x1fold.rs\x1fnew.rs\x1f").unwrap(),
        ChangedFile {
            path: "new.rs".to_string(),
            source_path: "old.rs".to_string(),
            status: GitFileStatus::Renamed {
                old_path: "old.rs".to_string()
            },
        }
    );
    assert_eq!(
        parse_changed_file("added\x1fa b.txt\x1fa b.txt\x1f")
            .unwrap()
            .status,
        GitFileStatus::New
    );
    assert_eq!(
        parse_changed_file("removed\x1fgone\x1fgone\x1f")
            .unwrap()
            .status,
        GitFileStatus::Deleted
    );
    assert_eq!(
        parse_changed_file("modified\x1fc\x1fc\x1fconflict")
            .unwrap()
            .status,
        GitFileStatus::Conflicted
    );
    assert!(parse_changed_file("modified").is_err());
}

#[test]
fn split_git_patch_keys_by_header_and_counts_lines() {
    let output = "diff --git a/a.txt b/a.txt\nindex 1..2 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -1,1 +1,2 @@\n-a\n+b\n+c\ndiff --git a/sp ace b/sp ace\nnew file mode 100644\n--- /dev/null\n+++ b/sp ace\n@@ -0,0 +1,1 @@\n+x\n";
    let patches = split_git_patch(output);
    assert_eq!(patches.len(), 2);
    assert_eq!(
        count_patch_lines(&patches[&patch_key("a.txt", "a.txt")]),
        (2, 1)
    );
    assert_eq!(
        count_patch_lines(&patches[&patch_key("sp ace", "sp ace")]),
        (1, 0)
    );
}

#[test]
fn parse_commits_reads_per_file_stats() {
    let commits = parse_commits("abc\x1fAdd feature\x1fa.rs\x1d3\x1d1\x1eb.rs\x1d2\x1d0\n");
    assert_eq!(commits.len(), 1);
    let commit = &commits[0];
    assert_eq!(commit.hash, "abc");
    assert_eq!(commit.subject, "Add feature");
    assert_eq!(commit.files_changed, 2);
    assert_eq!(commit.additions, 5);
    assert_eq!(commit.deletions, 1);
}

/// Creates a repo whose `main` bookmark holds `a.txt`, with the working copy on top of it.
fn repo_with_main() -> Option<tempfile::TempDir> {
    let temp_dir = init_test_repo()?;
    let repo = temp_dir.path();
    fs::write(repo.join("a.txt"), "one\n").unwrap();
    run_jj(repo, &["commit", "-m", "base"]);
    run_jj(repo, &["bookmark", "create", "main", "-r", "@-"]);
    Some(temp_dir)
}

#[tokio::test]
async fn load_diffs_covers_working_copy_and_base_branch() {
    let Some(temp_dir) = repo_with_main() else {
        return;
    };
    let repo = temp_dir.path();
    fs::write(repo.join("committed.txt"), "c\n").unwrap();
    run_jj(repo, &["commit", "-m", "feature work"]);
    fs::write(repo.join("a.txt"), "one\ntwo\n").unwrap();

    let head = load_diffs(repo, &DiffMode::Head).await.unwrap();
    assert_eq!(head.files_changed, 1);
    let file = &head.files[0];
    assert_eq!(file.file_diff.file_path, "a.txt");
    assert_eq!(file.file_diff.status, GitFileStatus::Modified);
    assert_eq!(file.file_diff.additions(), 1);
    assert_eq!(file.content_at_head.as_deref(), Some("one\n"));

    let against_main = load_diffs(repo, &DiffMode::OtherBranch("main".to_string()))
        .await
        .unwrap();
    let mut paths: Vec<_> = against_main
        .files
        .iter()
        .map(|file| file.file_diff.file_path.as_str())
        .collect();
    paths.sort();
    assert_eq!(paths, ["a.txt", "committed.txt"]);
}

#[tokio::test]
async fn retrieve_diff_state_tracks_single_file() {
    let Some(temp_dir) = repo_with_main() else {
        return;
    };
    let repo = temp_dir.path();
    fs::write(repo.join("a.txt"), "changed\n").unwrap();

    let (path, diff) = retrieve_diff_state(repo, "a.txt".to_string(), &DiffMode::Head, None)
        .await
        .unwrap();
    assert_eq!(path, "a.txt");
    assert!(diff.is_some());

    fs::write(repo.join("a.txt"), "one\n").unwrap();
    let (_, diff) = retrieve_diff_state(repo, "a.txt".to_string(), &DiffMode::Head, None)
        .await
        .unwrap();
    assert!(diff.is_none());
}

#[tokio::test]
async fn load_metadata_lists_unpushed_commits() {
    let Some(temp_dir) = repo_with_main() else {
        return;
    };
    let repo = temp_dir.path();
    fs::write(repo.join("b.txt"), "b\n").unwrap();
    run_jj(repo, &["commit", "-m", "second"]);
    run_jj(repo, &["bookmark", "create", "feature", "-r", "@-"]);
    fs::write(repo.join("c.txt"), "c\n").unwrap();

    let metadata = load_metadata(repo, true).await.unwrap();
    assert_eq!(metadata.current_branch_name, "feature");
    assert_eq!(metadata.against_head.aggregate_stats.files_changed, 1);
    assert!(metadata.upstream_ref.is_none());
    let subjects: Vec<_> = metadata
        .unpushed_commits
        .iter()
        .map(|commit| commit.subject.as_str())
        .collect();
    assert!(subjects.contains(&"second"), "{subjects:?}");
}

#[tokio::test]
async fn discard_files_restores_from_parent() {
    let Some(temp_dir) = repo_with_main() else {
        return;
    };
    let repo = temp_dir.path();
    fs::write(repo.join("a.txt"), "changed\n").unwrap();
    fs::write(repo.join("new.txt"), "new\n").unwrap();

    discard_files(
        repo,
        &[
            ("a.txt".to_string(), GitFileStatus::Modified),
            ("new.txt".to_string(), GitFileStatus::New),
        ],
        None,
    )
    .await
    .unwrap();

    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "one\n");
    assert!(!repo.join("new.txt").exists());
}

#[tokio::test]
async fn commit_and_push_create_bookmark_on_remote() {
    let Some(temp_dir) = repo_with_main() else {
        return;
    };
    let repo = temp_dir.path();
    let remote = tempfile::tempdir().unwrap();
    let init = command::blocking::Command::new("git")
        .args(["init", "--bare", "-q"])
        .arg(remote.path())
        .output()
        .unwrap();
    assert!(init.status.success());
    run_jj(
        repo,
        &[
            "git",
            "remote",
            "add",
            "origin",
            remote.path().to_str().unwrap(),
        ],
    );

    fs::write(repo.join("feature.txt"), "f\n").unwrap();
    crate::util::jj::commit(repo, "add feature", None)
        .await
        .unwrap();
    assert!(
        crate::util::jj::commit(repo, "nothing", None)
            .await
            .is_err(),
        "an empty working copy should not be committed"
    );

    crate::util::jj::push_bookmark(repo, "feature", None)
        .await
        .unwrap();
    let (commits, upstream_ref) = unpushed_state(repo).await;
    assert_eq!(upstream_ref.as_deref(), Some("feature@origin"));
    assert!(commits.is_empty());
}
