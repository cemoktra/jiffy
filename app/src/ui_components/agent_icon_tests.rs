use super::*;

fn session(agent: CLIAgent, has_listener: bool, supports_rich_status: bool) -> CLISessionInputs {
    CLISessionInputs {
        agent,
        has_listener,
        status: CLIAgentSessionStatus::Blocked { message: None },
        supports_rich_status,
    }
}

fn status_of(variant: Option<IconWithStatusVariant>) -> Option<Option<CLIAgentSessionStatus>> {
    match variant? {
        IconWithStatusVariant::CLIAgent { status, .. } => Some(status),
        IconWithStatusVariant::Neutral { .. } | IconWithStatusVariant::NeutralElement { .. } => {
            None
        }
    }
}

#[test]
fn plugin_backed_session_shows_status() {
    let variant = agent_icon_variant_from_session(&session(CLIAgent::Claude, true, true));
    assert_eq!(
        status_of(variant),
        Some(Some(CLIAgentSessionStatus::Blocked { message: None }))
    );
}

#[test]
fn command_detected_session_hides_status() {
    let variant = agent_icon_variant_from_session(&session(CLIAgent::Claude, false, true));
    assert_eq!(status_of(variant), Some(None));
}

#[test]
fn session_without_rich_status_hides_status() {
    let variant = agent_icon_variant_from_session(&session(CLIAgent::Codex, true, false));
    assert_eq!(status_of(variant), Some(None));
}

#[test]
fn unknown_agent_has_no_icon() {
    assert!(agent_icon_variant_from_session(&session(CLIAgent::Unknown, true, true)).is_none());
}
