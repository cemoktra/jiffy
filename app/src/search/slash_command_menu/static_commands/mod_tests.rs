use super::{Availability, commands};

#[test]
fn argument_hint_uses_shared_command_prefix_and_text() {
    let hint = commands::RENAME_TAB
        .argument_hint()
        .expect("rename-tab has an argument hint");

    assert_eq!(hint.input_prefix, "/rename-tab ");
    assert_eq!(hint.text, "<tab name>");
    assert_eq!(commands::OPEN_CODE_REVIEW.argument_hint(), None);
}

/// Helper: constructs a session context for a local session with a repo.
fn local_session_with_repo() -> Availability {
    Availability::LOCAL | Availability::REPOSITORY
}

#[test]
fn always_available_in_any_context() {
    let command = Availability::ALWAYS;
    assert!(local_session_with_repo().contains(command));
    // Even a remote session outside a repo satisfies ALWAYS.
    assert!(Availability::empty().contains(command));
}

#[test]
fn repository_requirement_satisfied_when_in_repo() {
    let command = Availability::REPOSITORY;
    assert!(local_session_with_repo().contains(command));
}

#[test]
fn repository_requirement_not_satisfied_when_not_in_repo() {
    let command = Availability::REPOSITORY;
    assert!(!Availability::LOCAL.contains(command));
}

#[test]
fn local_requirement_satisfied_in_local_session() {
    let command = Availability::LOCAL;
    assert!(local_session_with_repo().contains(command));
}

#[test]
fn local_requirement_not_satisfied_in_remote_session() {
    let command = Availability::LOCAL;
    // Remote session: no LOCAL flag.
    assert!(!Availability::REPOSITORY.contains(command));
}

#[test]
fn cli_agent_input_requirement_only_satisfied_while_rich_input_is_open() {
    let command = Availability::CLI_AGENT_INPUT;
    assert!(!local_session_with_repo().contains(command));
    assert!((local_session_with_repo() | Availability::CLI_AGENT_INPUT).contains(command));
}

#[test]
fn all_flags_required() {
    let command = Availability::LOCAL | Availability::REPOSITORY | Availability::CLI_AGENT_INPUT;
    assert!(command.contains(command));
    assert!(!local_session_with_repo().contains(command));
    assert!(!(Availability::LOCAL | Availability::CLI_AGENT_INPUT).contains(command));
    assert!(!(Availability::REPOSITORY | Availability::CLI_AGENT_INPUT).contains(command));
}

#[test]
fn invoke_skill_is_only_active_in_cli_agent_rich_input() {
    assert!(!commands::INVOKE_SKILL.is_active(local_session_with_repo()));
    assert!(
        commands::INVOKE_SKILL.is_active(local_session_with_repo() | Availability::CLI_AGENT_INPUT)
    );
}
