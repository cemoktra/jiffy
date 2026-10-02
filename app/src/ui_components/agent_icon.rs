//! Source-facing helper that derives the agent-icon shape ([`IconWithStatusVariant`]) for a
//! terminal running a third-party CLI agent, so every surface (vertical tabs, pane header)
//! renders the same brand glyph and status badge.
use warpui::{AppContext, SingletonEntity};

use crate::terminal::CLIAgent;
use crate::terminal::cli_agent_sessions::{CLIAgentSessionStatus, CLIAgentSessionsModel};
use crate::terminal::view::TerminalView;
use crate::ui_components::icon_with_status::IconWithStatusVariant;

/// Returns the agent-icon variant for a live [`TerminalView`], or `None` when the terminal is
/// not running a known CLI agent.
pub(crate) fn terminal_view_agent_icon_variant(
    terminal_view: &TerminalView,
    app: &AppContext,
) -> Option<IconWithStatusVariant> {
    let session = CLIAgentSessionsModel::as_ref(app).session(terminal_view.id())?;
    agent_icon_variant_from_session(&CLISessionInputs {
        agent: session.agent,
        has_listener: session.listener.is_some(),
        status: session.status.clone(),
        supports_rich_status: session.supports_rich_status(),
    })
}

/// CLI-session-derived inputs for [`agent_icon_variant_from_session`].
struct CLISessionInputs {
    agent: CLIAgent,
    /// Whether the session is backed by a plugin listener. Plugin-backed sessions report
    /// rich status; command-detected sessions only know that an agent is running.
    has_listener: bool,
    status: CLIAgentSessionStatus,
    /// Whether the agent's session handler exposes rich status (plugin-backed handlers report
    /// rich status; Codex's OSC 9 handler does not).
    supports_rich_status: bool,
}

/// Maps a CLI session to its icon variant. `Unknown` agents are filtered so an unrecognized
/// command doesn't render as an unbranded gray circle, and status is only shown when the
/// session is plugin-backed and the handler exposes rich status.
fn agent_icon_variant_from_session(session: &CLISessionInputs) -> Option<IconWithStatusVariant> {
    if matches!(session.agent, CLIAgent::Unknown) {
        return None;
    }
    let status =
        (session.has_listener && session.supports_rich_status).then(|| session.status.clone());
    Some(IconWithStatusVariant::CLIAgent {
        agent: session.agent,
        status,
    })
}

#[cfg(test)]
#[path = "agent_icon_tests.rs"]
mod tests;
