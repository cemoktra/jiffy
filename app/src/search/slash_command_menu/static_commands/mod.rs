pub mod bindings;
pub mod commands;

use bitflags::bitflags;
pub use commands::SlashCommandId;
use settings::SettingsMode;

bitflags! {
    /// Specifies the requirements for a slash command to be available.
    ///
    /// Each flag represents a requirement that the session context must satisfy. The command is
    /// available when the session supports *all* of the command's requirement flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Availability: u8 {
        /// No requirements — always available.
        const ALWAYS = 0;
        /// Requires a local session (not available in remote sessions).
        const LOCAL = 1 << 0;
        /// Requires a git repository.
        const REPOSITORY = 1 << 1;
        /// Requires the CLI agent rich input to be open. While it is open, only commands with
        /// this requirement are available.
        const CLI_AGENT_INPUT = 1 << 2;
    }
}
/// Stable identity for a static slash command.
///
/// Front-ends dispatch on this value instead of matching command-name strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlashCommandKind {
    CreateDockerSandbox,
    EditSkill,
    InvokeSkill,
    Edit,
    RenameTab,
    SetTabColor,
    OpenCodeReview,
    OpenSettingsFile,
    Feedback,
}

/// The application surfaces on which a static slash command is implemented.
///
/// This field is required on every [`StaticCommand`] so new commands must explicitly declare
/// whether they are GUI-only, TUI-only, or shared by both front-ends. GUI-capable variants also
/// require the icon path used to render the command in GUI menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashCommandSurfaces {
    GuiOnly { icon_path: &'static str },
    TuiOnly,
    GuiAndTui { icon_path: &'static str },
}

impl SlashCommandSurfaces {
    pub fn supports_gui(self) -> bool {
        matches!(self, Self::GuiOnly { .. } | Self::GuiAndTui { .. })
    }

    pub fn supports_tui(self) -> bool {
        matches!(self, Self::TuiOnly | Self::GuiAndTui { .. })
    }

    pub fn gui_icon_path(self) -> Option<&'static str> {
        match self {
            Self::GuiOnly { icon_path } | Self::GuiAndTui { icon_path } => Some(icon_path),
            Self::TuiOnly => None,
        }
    }
    pub fn includes(self, settings_mode: SettingsMode) -> bool {
        match settings_mode {
            SettingsMode::Gui => self.supports_gui(),
            SettingsMode::Tui => self.supports_tui(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Argument {
    pub hint_text: Option<&'static str>,
    pub is_optional: bool,
    /// If `true`, selecting the slash command from the menu (or via keybinding) will execute the
    /// slash command with no arguments.
    ///
    /// If `false`, selecting the slash command from the menu (or via keybinding) inserts the
    /// slash command into the input.
    ///
    /// Set this based on whether or not you want you think a user should always have the option to
    /// supply an argument.
    pub should_execute_on_selection: bool,
}

impl Argument {
    pub(super) fn optional() -> Self {
        Self {
            is_optional: true,
            ..Default::default()
        }
    }

    pub(super) fn required() -> Self {
        Self {
            is_optional: false,
            ..Default::default()
        }
    }

    pub(super) fn with_hint_text(mut self, text: &'static str) -> Self {
        self.hint_text = Some(text);
        self
    }

    pub(super) fn with_execute_on_selection(mut self) -> Self {
        self.should_execute_on_selection = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticCommand {
    pub kind: SlashCommandKind,
    pub name: &'static str,
    pub description: &'static str,
    pub supported_surfaces: SlashCommandSurfaces,
    /// Specifies the requirements for this command to be available. See [`Availability`].
    pub availability: Availability,
    pub argument: Option<Argument>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommandArgumentHint {
    pub input_prefix: String,
    pub text: &'static str,
}

impl StaticCommand {
    pub fn supports_gui(&self) -> bool {
        self.supported_surfaces.supports_gui()
    }

    pub fn supports_tui(&self) -> bool {
        self.supported_surfaces.supports_tui()
    }
    pub fn supports_surface(&self, settings_mode: SettingsMode) -> bool {
        self.supported_surfaces.includes(settings_mode)
    }

    pub fn matches_filter(&self, filter_text: &str) -> bool {
        if filter_text.is_empty() {
            return true;
        }

        let filter_lower = filter_text.to_lowercase();
        self.name
            .to_lowercase()
            .get(1..)
            .unwrap_or("")
            .starts_with(&filter_lower)
    }

    pub fn is_active(&self, session_context: Availability) -> bool {
        session_context.contains(self.availability)
    }

    pub fn argument_hint(&self) -> Option<SlashCommandArgumentHint> {
        let text = self.argument.as_ref()?.hint_text?;
        Some(SlashCommandArgumentHint {
            input_prefix: format!("{} ", self.name),
            text,
        })
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
