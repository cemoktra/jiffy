use warpui::{App, SingletonEntity};

use super::*;
use crate::terminal::CLIAgent;
use crate::terminal::cli_agent_sessions::{
    CLIAgentInputState, CLIAgentSession, CLIAgentSessionContext, CLIAgentSessionStatus,
    CLIAgentSessionsModel,
};
use crate::terminal::model::ansi::{BootstrappedValue, Handler as _, InitShellValue};
use crate::test_util::add_window_with_terminal;
use crate::test_util::terminal::initialize_app_for_terminal_view;

fn simulate_user_started_long_running_command(view: &mut TerminalView) {
    {
        let mut model = view.model.lock();
        model.init_shell(InitShellValue {
            session_id: 0.into(),
            shell: "zsh".to_owned(),
            ..Default::default()
        });
        model.bootstrapped(BootstrappedValue {
            shell: "zsh".to_owned(),
            ..Default::default()
        });
        model.simulate_long_running_block("ssh localhost", "Password:");
    }
}

#[test]
fn cli_agent_footer_renders_for_claude_session() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);

        let terminal = add_window_with_terminal(&mut app, None);

        terminal.update(&mut app, |view, ctx| {
            simulate_user_started_long_running_command(view);

            CLIAgentSessionsModel::handle(ctx).update(ctx, |sessions, ctx| {
                sessions.set_session(
                    view.id(),
                    CLIAgentSession {
                        agent: CLIAgent::Claude,
                        status: CLIAgentSessionStatus::InProgress,
                        session_context: CLIAgentSessionContext::default(),
                        input_state: CLIAgentInputState::Closed,
                        listener: None,
                        plugin_version: None,
                        remote_host: None,
                        draft_text: None,
                        custom_command_prefix: None,
                        received_rich_notification: false,
                        should_auto_toggle_input: false,
                    },
                    ctx,
                );
            });

            view.maybe_show_use_agent_footer_in_blocklist(ctx);

            assert!(view.should_render_use_agent_footer(ctx));
            let model = view.model.lock();
            let active_block_index = model.block_list().active_block_index();
            let rendered_footer_view_id = model
                .block_list()
                .last_non_hidden_rich_content_block_after_block(Some(active_block_index))
                .map(|(_, item)| item.view_id);
            assert_eq!(rendered_footer_view_id, Some(view.use_agent_footer.id()));
        });
    })
}

#[test]
fn cli_agent_footer_does_not_render_for_warp_tui_session() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);

        let terminal = add_window_with_terminal(&mut app, None);

        terminal.update(&mut app, |view, ctx| {
            simulate_user_started_long_running_command(view);

            CLIAgentSessionsModel::handle(ctx).update(ctx, |sessions, ctx| {
                sessions.set_session(
                    view.id(),
                    CLIAgentSession {
                        agent: CLIAgent::WarpTui,
                        status: CLIAgentSessionStatus::InProgress,
                        session_context: CLIAgentSessionContext::default(),
                        input_state: CLIAgentInputState::Closed,
                        listener: None,
                        plugin_version: None,
                        remote_host: None,
                        draft_text: None,
                        custom_command_prefix: None,
                        received_rich_notification: false,
                        should_auto_toggle_input: false,
                    },
                    ctx,
                );
            });

            view.maybe_show_use_agent_footer_in_blocklist(ctx);

            assert!(!view.should_render_use_agent_footer(ctx));
            let model = view.model.lock();
            let active_block_index = model.block_list().active_block_index();
            assert!(
                model
                    .block_list()
                    .last_non_hidden_rich_content_block_after_block(Some(active_block_index))
                    .is_none()
            );
        });
    })
}
#[test]
fn test_rich_input_submit_strategy_for_oh_my_pi() {
    assert_eq!(
        rich_input_submit_strategy(CLIAgent::OhMyPi),
        RichInputSubmitStrategy::BracketedPaste
    );
}

/// Hermes interprets embedded newlines as submit actions when text is written
/// directly. Bracketed paste preserves them as part of one input payload.
#[test]
fn test_rich_input_submit_strategy_for_hermes_uses_bracketed_paste() {
    assert_eq!(
        rich_input_submit_strategy(CLIAgent::Hermes),
        RichInputSubmitStrategy::BracketedPaste
    );
}
