use indexmap::IndexMap;
use settings::Setting;
use warp_errors::report_if_error;
use warpui::{App, SingletonEntity};

use super::*;
use crate::test_util::settings::initialize_settings_for_tests;

// ToolbarCommandMap Tests

#[test]
fn test_toolbar_command_map_deserialize_from_map() {
    let json = serde_json::json!({
        "^claude": "Claude",
        "^gemini": "Gemini",
        "^codex": ""
    });
    let map: ToolbarCommandMap = serde_json::from_value(json).unwrap();
    assert_eq!(map.0.len(), 3);
    assert_eq!(map.0["^claude"], "Claude");
    assert_eq!(map.0["^gemini"], "Gemini");
    assert_eq!(map.0["^codex"], "");
}

#[test]
fn test_toolbar_command_map_deserialize_from_legacy_vec() {
    let json = serde_json::json!(["^claude", "^gemini", "^custom"]);
    let map: ToolbarCommandMap = serde_json::from_value(json).unwrap();
    assert_eq!(map.0.len(), 3);
    // Legacy vec format should assign empty agent values.
    for (_, agent) in map.0.iter() {
        assert_eq!(agent, "");
    }
    let keys: Vec<_> = map.0.keys().collect();
    assert_eq!(keys, vec!["^claude", "^gemini", "^custom"]);
}

#[test]
fn test_toolbar_command_map_from_file_value_map_format() {
    use settings_value::SettingsValue;

    let value = serde_json::json!({
        "^claude": "Claude",
        "^amp": "Amp"
    });
    let map = ToolbarCommandMap::from_file_value(&value).unwrap();
    assert_eq!(map.0.len(), 2);
    assert_eq!(map.0["^claude"], "Claude");
    assert_eq!(map.0["^amp"], "Amp");
}

#[test]
fn test_toolbar_command_map_from_file_value_legacy_array() {
    use settings_value::SettingsValue;

    // Patterns are intentionally non-alphabetical to verify insertion order is preserved.
    let value = serde_json::json!(["^zebra", "^alpha", "^middle"]);
    let map = ToolbarCommandMap::from_file_value(&value).unwrap();
    assert_eq!(map.0.len(), 3);
    assert_eq!(map.0["^zebra"], "");
    assert_eq!(map.0["^alpha"], "");
    assert_eq!(map.0["^middle"], "");
    let keys: Vec<_> = map.0.keys().collect();
    assert_eq!(keys, vec!["^zebra", "^alpha", "^middle"]);
}

#[test]
fn test_toolbar_command_map_from_file_value_invalid() {
    use settings_value::SettingsValue;

    let value = serde_json::json!(42);
    assert!(ToolbarCommandMap::from_file_value(&value).is_none());
}

#[test]
fn test_toolbar_command_map_roundtrip() {
    use settings_value::SettingsValue;

    let mut inner = IndexMap::new();
    inner.insert("^claude".to_string(), "Claude".to_string());
    inner.insert("^custom".to_string(), String::new());
    let original = ToolbarCommandMap::new(inner);

    let file_value = original.to_file_value();
    let restored = ToolbarCommandMap::from_file_value(&file_value).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn test_toolbar_command_map_matched_agent() {
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);

        let mut map = IndexMap::new();
        map.insert("^claude".to_string(), "Claude".to_string());
        map.insert("^gemini".to_string(), "Gemini".to_string());
        map.insert("^custom-tool".to_string(), String::new());

        CLIAgentSettings::handle(&app).update(&mut app, |settings, ctx| {
            report_if_error!(
                settings
                    .cli_agent_footer_enabled_commands
                    .set_value(ToolbarCommandMap::new(map), ctx)
            );
        });

        app.read(|ctx| {
            let agent = CompiledCommandsForCodingAgentToolbar::matched_agent(ctx, "claude chat");
            assert_eq!(agent, Some(CLIAgent::Claude));

            let agent = CompiledCommandsForCodingAgentToolbar::matched_agent(ctx, "gemini ask");
            assert_eq!(agent, Some(CLIAgent::Gemini));

            let agent =
                CompiledCommandsForCodingAgentToolbar::matched_agent(ctx, "custom-tool --flag");
            assert_eq!(agent, Some(CLIAgent::Unknown));

            let agent =
                CompiledCommandsForCodingAgentToolbar::matched_agent(ctx, "unmatched-command");
            assert_eq!(agent, None);
        });
    });
}
