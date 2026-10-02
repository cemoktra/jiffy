#[cfg(not(target_family = "wasm"))]
use settings::Setting as _;
use warpui::AppContext;
#[cfg(not(target_family = "wasm"))]
use warpui::SingletonEntity as _;

#[cfg(not(target_family = "wasm"))]
use crate::search::ai_context_menu::view::AIContextMenu;
#[cfg(not(target_family = "wasm"))]
use crate::settings::InputSettings;
use crate::terminal::model::block::BlockMetadata;
#[cfg(not(target_family = "wasm"))]
use crate::terminal::model::session::SessionType;
use crate::terminal::model::session::Sessions;

pub enum AtContextMenuDisabledReason {
    #[cfg(target_family = "wasm")]
    Wasm,
    #[cfg(not(target_family = "wasm"))]
    NoObjectsAvailable,
    #[cfg(not(target_family = "wasm"))]
    SshWithoutRemoteServer,
    #[cfg(not(target_family = "wasm"))]
    Subshell,
    #[cfg(not(target_family = "wasm"))]
    DisabledInTerminalMode,
}

impl AtContextMenuDisabledReason {
    #[cfg(target_family = "wasm")]
    pub fn get_disable_reason(
        _active_block_metadata: Option<&BlockMetadata>,
        _sessions: &Sessions,
        _ctx: &AppContext,
    ) -> Option<AtContextMenuDisabledReason> {
        Some(AtContextMenuDisabledReason::Wasm)
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn get_disable_reason(
        active_block_metadata: Option<&BlockMetadata>,
        sessions: &Sessions,
        ctx: &AppContext,
    ) -> Option<AtContextMenuDisabledReason> {
        // Derive session information from block metadata and sessions
        let (is_ssh_without_remote_server, is_subshell) = active_block_metadata
            .and_then(|metadata| metadata.session_id())
            .and_then(|session_id| sessions.get(session_id))
            .map(|session| {
                // The @ menu requires repo metadata, which is only available for local sessions.
                let is_ssh_without_remote_server = session.is_ssh_wrapper_session()
                    || matches!(session.session_type(), SessionType::WarpifiedRemote);
                let is_subshell = session.subshell_info().is_some();
                (is_ssh_without_remote_server, is_subshell)
            })
            .unwrap_or((false, false));

        if !*InputSettings::as_ref(ctx)
            .at_context_menu_in_terminal_mode
            .value()
        {
            return Some(AtContextMenuDisabledReason::DisabledInTerminalMode);
        }

        if is_ssh_without_remote_server {
            return Some(AtContextMenuDisabledReason::SshWithoutRemoteServer);
        }
        if is_subshell {
            return Some(AtContextMenuDisabledReason::Subshell);
        }

        // Repo-based restrictions are enforced at the category level (e.g. Code is only
        // available inside a git repository).
        if AIContextMenu::get_categories(ctx).is_empty() {
            return Some(AtContextMenuDisabledReason::NoObjectsAvailable);
        }

        None
    }
}
