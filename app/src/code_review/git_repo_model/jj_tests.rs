use super::*;
use crate::util::jj::test_support::{init_test_repo, run_jj};

#[test]
fn parse_diff_stat_summary_reads_summary_line() {
    let output =
        "a.txt   | 3 ++-\nnew.txt | 1 +\n2 files changed, 3 insertions(+), 1 deletion(-)\n";
    let stats = parse_diff_stat_summary(output);
    assert_eq!(stats.files_changed, 2);
    assert_eq!(stats.total_additions, 3);
    assert_eq!(stats.total_deletions, 1);
}

#[test]
fn parse_diff_stat_summary_handles_no_changes() {
    let stats = parse_diff_stat_summary("0 files changed, 0 insertions(+), 0 deletions(-)\n");
    assert!(stats.has_no_changes());
    assert!(parse_diff_stat_summary("").has_no_changes());
}

#[tokio::test]
async fn load_metadata_reports_bookmark_and_working_copy_stats() {
    let Some(temp_dir) = init_test_repo() else {
        return;
    };
    let repo = temp_dir.path();
    std::fs::write(repo.join("a.txt"), "one\n").unwrap();
    run_jj(repo, &["commit", "-m", "first"]);
    run_jj(repo, &["bookmark", "create", "main", "-r", "@-"]);
    std::fs::write(repo.join("a.txt"), "one\ntwo\n").unwrap();
    std::fs::write(repo.join("b.txt"), "new\n").unwrap();

    let metadata = load_metadata(repo).await.unwrap();

    assert_eq!(metadata.current_branch_name, "main");
    assert_eq!(metadata.prompt_label, "main⇡1 (no description)");
    assert_eq!(metadata.stats_against_head.files_changed, 2);
    assert_eq!(metadata.stats_against_head.total_additions, 2);
    assert_eq!(metadata.stats_against_head.total_deletions, 0);
    assert!(metadata.branch_tracking_status.upstream.is_none());
}

#[tokio::test]
async fn load_metadata_prompt_label_describes_working_copy() {
    let Some(temp_dir) = init_test_repo() else {
        return;
    };
    let repo = temp_dir.path();
    std::fs::write(repo.join("a.txt"), "one\n").unwrap();
    run_jj(repo, &["describe", "-m", "Add a.txt"]);
    run_jj(repo, &["bookmark", "create", "feature", "-r", "@"]);

    let metadata = load_metadata(repo).await.unwrap();

    assert_eq!(metadata.current_branch_name, "feature");
    assert_eq!(metadata.prompt_label, "feature \"Add a.txt\"");
    assert_eq!(
        metadata.branch_tracking_status.branch,
        metadata.prompt_label
    );
}

fn working_copy(has_conflict: bool) -> WorkingCopyInfo {
    WorkingCopyInfo {
        change_id: "kmzqwtpr".to_string(),
        bookmark: None,
        has_conflict,
    }
}

#[test]
fn format_prompt_label_shows_bookmark_distance_and_markers() {
    let details = PromptDetails {
        description: String::new(),
        is_empty: true,
        is_divergent: true,
    };
    assert_eq!(
        format_prompt_label(&working_copy(true), Some(("main".to_string(), 2)), &details),
        "main⇡2 (no description) (conflict) (divergent) (empty)"
    );
}

#[test]
fn format_prompt_label_falls_back_to_change_id_and_truncates_description() {
    let details = PromptDetails {
        description: "Rework the prompt chip for jj workspaces".to_string(),
        ..Default::default()
    };
    assert_eq!(
        format_prompt_label(&working_copy(false), None, &details),
        "kmzqwtpr \"Rework the prompt chip…\""
    );
}

#[test]
fn parse_prompt_details_reads_markers() {
    assert_eq!(
        parse_prompt_details("Fix bug\x1fempty\x1f\n"),
        Some(PromptDetails {
            description: "Fix bug".to_string(),
            is_empty: true,
            is_divergent: false,
        })
    );
}
