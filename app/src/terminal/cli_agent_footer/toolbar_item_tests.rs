use super::*;

#[test]
fn defaults_are_all_offered_in_the_configurator() {
    let available = CLIAgentToolbarItemKind::all_available();
    for item in CLIAgentToolbarItemKind::default_left()
        .into_iter()
        .chain(CLIAgentToolbarItemKind::default_right())
    {
        assert!(
            available.contains(&item),
            "{item:?} missing from all_available"
        );
    }
}

#[test]
fn settings_item_is_a_right_side_default() {
    assert!(CLIAgentToolbarItemKind::default_right().contains(&CLIAgentToolbarItemKind::Settings));
    assert!(!CLIAgentToolbarItemKind::default_left().contains(&CLIAgentToolbarItemKind::Settings));
}

#[test]
fn legacy_image_attach_name_deserializes_as_file_attach() {
    let item: CLIAgentToolbarItemKind =
        serde_json::from_value(serde_json::json!("ImageAttach")).unwrap();
    assert_eq!(item, CLIAgentToolbarItemKind::FileAttach);
}
