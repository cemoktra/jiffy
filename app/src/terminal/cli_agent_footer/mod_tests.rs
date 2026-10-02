use warpui::App;

use super::*;
use crate::terminal::cli_agent_sessions::{
    CLIAgentInputState, CLIAgentSession, CLIAgentSessionContext, CLIAgentSessionStatus,
    CLIAgentSessionsModel,
};
use crate::test_util::add_window_with_terminal;
use crate::test_util::terminal::initialize_app_for_terminal_view;

fn claude_session() -> CLIAgentSession {
    CLIAgentSession {
        agent: CLIAgent::Claude,
        status: CLIAgentSessionStatus::InProgress,
        session_context: CLIAgentSessionContext::default(),
        input_state: CLIAgentInputState::Open,
        listener: None,
        plugin_version: None,
        remote_host: None,
        draft_text: None,
        custom_command_prefix: None,
        received_rich_notification: false,
        should_auto_toggle_input: false,
    }
}

fn start_cli_session(
    view: &mut crate::terminal::TerminalView,
    ctx: &mut ViewContext<crate::terminal::TerminalView>,
) {
    let view_id = view.id();
    CLIAgentSessionsModel::handle(ctx).update(ctx, |sessions, ctx| {
        sessions.set_session(view_id, claude_session(), ctx);
    });
}

fn rendered_footer_child_ids(
    view: &crate::terminal::TerminalView,
    ctx: &AppContext,
) -> Vec<EntityId> {
    view.input()
        .as_ref(ctx)
        .cli_agent_footer()
        .as_ref(ctx)
        .render(ctx)
        .debug_child_view_ids()
}

#[test]
fn cli_footer_renders_default_host_items() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);

        terminal.update(&mut app, |view, ctx| {
            start_cli_session(view, ctx);
            let footer = view.input().as_ref(ctx).cli_agent_footer().as_ref(ctx);
            let settings_id = footer.settings_button_id();
            let file_id = footer.file_button_id();
            let child_ids = rendered_footer_child_ids(view, ctx);
            assert!(
                child_ids.contains(&settings_id),
                "host footer should render the settings item, got {child_ids:?}"
            );
            assert!(
                child_ids.contains(&file_id),
                "host footer should render the attach-file item, got {child_ids:?}"
            );
        });
    });
}

#[test]
fn plugin_chip_key_includes_remote_host() {
    assert_eq!(plugin_chip_key("claude", &None), "claude");
    assert_eq!(
        plugin_chip_key("claude", &Some("devbox".to_owned())),
        "claude@devbox"
    );
}
