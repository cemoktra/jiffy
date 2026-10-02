#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CodingPanelEnablementState {
    Enabled,
    /// The active session is on a remote host (or an SSH command is still connecting), where the
    /// local coding panels have no data to show.
    RemoteSession,
    UnsupportedSession,
    Disabled,
}

impl CodingPanelEnablementState {
    pub(crate) fn from_session_env(
        is_enabled: bool,
        is_remote: bool,
        is_unsupported_session: bool,
    ) -> Self {
        if is_remote {
            Self::RemoteSession
        } else if is_unsupported_session {
            Self::UnsupportedSession
        } else if is_enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }
}
