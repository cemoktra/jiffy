use warpui_core::{Entity, ModelContext, SingletonEntity};

/// Execution mode that Warp is running under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Warp is running as a normal desktop app.
    App,
    /// Warp is running as the headless terminal UI.
    Tui,
    /// Warp is running as a CLI.
    Sdk,
}

/// Model tracking the mode that Warp is running in.
///
/// This gates functionality that's disabled when Warp is running in SDK mode.
#[derive(Clone, Debug)]
pub struct AppExecutionMode {
    mode: ExecutionMode,
}

impl AppExecutionMode {
    /// Create an `AppExecutionMode` model with the execution mode set.
    pub fn new(mode: ExecutionMode, _ctx: &mut ModelContext<Self>) -> Self {
        Self { mode }
    }

    /// Whether the app can save and restore sessions.
    pub fn can_save_session(&self) -> bool {
        matches!(self.mode, ExecutionMode::App | ExecutionMode::Tui)
    }
}

impl Entity for AppExecutionMode {
    type Event = ();
}

impl SingletonEntity for AppExecutionMode {}
