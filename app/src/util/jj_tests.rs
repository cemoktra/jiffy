use super::*;

#[test]
fn parse_revision_info_reads_all_fields() {
    assert_eq!(
        parse_revision_info("wqpolpwy\x1fmain,feature\x1fconflict\n"),
        Some((
            "wqpolpwy".to_string(),
            vec!["main".to_string(), "feature".to_string()],
            true
        ))
    );
}

#[test]
fn parse_revision_info_handles_missing_bookmarks_and_conflict() {
    assert_eq!(
        parse_revision_info("kxyz\x1f\x1f\n"),
        Some(("kxyz".to_string(), Vec::new(), false))
    );
}

#[test]
fn parse_revision_info_rejects_empty_output() {
    assert_eq!(parse_revision_info(""), None);
    assert_eq!(parse_revision_info("\x1f\x1f\n"), None);
}

#[test]
fn quote_revset_string_escapes_quotes() {
    assert_eq!(quote_revset_string(r#"a"b\c"#), r#""a\"b\\c""#);
    assert_eq!(root_file_fileset("sp ace.txt"), r#"root-file:"sp ace.txt""#);
}

#[test]
fn working_copy_display_name_marks_conflicts() {
    let info = WorkingCopyInfo {
        change_id: "kxyz".to_string(),
        bookmark: None,
        has_conflict: true,
    };
    assert_eq!(info.display_name(), "kxyz (conflict)");
}

#[test]
fn main_workspace_root_resolves_secondary_workspaces_only() {
    let _jj_guard = FeatureFlag::JjSupport.override_enabled(true);
    let temp_dir = tempfile::tempdir().unwrap();
    let main = temp_dir.path().join("main");
    let secondary = temp_dir.path().join("secondary");
    std::fs::create_dir_all(main.join(".jj").join("repo")).unwrap();
    std::fs::create_dir_all(secondary.join(".jj")).unwrap();
    std::fs::write(secondary.join(".jj").join("repo"), "../../main/.jj/repo").unwrap();

    assert_eq!(
        main_workspace_root(&secondary),
        Some(main.canonicalize().unwrap())
    );
    assert_eq!(main_workspace_root(&main), None);
}
