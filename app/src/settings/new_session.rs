//! Settings controlling what a newly created session opens as.

use settings::{
    RespectUserSyncSetting, Setting as _, SupportedPlatforms, SyncToCloud, define_settings_group,
};
use strum_macros::EnumIter;
use warp_core::features::FeatureFlag;
use warpui::{AppContext, SingletonEntity};

/// The default mode for new terminal sessions.
#[derive(
    Default,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Copy,
    Clone,
    EnumIter,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Default mode for new sessions.",
    rename_all = "snake_case"
)]
pub enum DefaultSessionMode {
    /// New sessions start in the terminal mode (default).
    #[default]
    Terminal,
    /// New sessions open a user-defined tab config.
    /// The specific config is identified by the companion `default_tab_config_path` setting.
    TabConfig,
    /// New sessions open in a local Docker sandbox.
    /// Requires the `LocalDockerSandbox` feature flag; falls back to `Terminal` when disabled.
    DockerSandbox,
}

settings::macros::implement_setting_for_enum!(
    DefaultSessionMode,
    NewSessionSettings,
    SupportedPlatforms::ALL,
    SyncToCloud::Globally(RespectUserSyncSetting::Yes),
    surface: settings::SettingSurfaces::GUI,
    private: false,
    toml_path: "general.default_session_mode",
    description: "The default mode for new terminal sessions.",
);

impl DefaultSessionMode {
    /// Display name for the settings dropdown.
    pub fn display_name(&self) -> &'static str {
        match self {
            DefaultSessionMode::Terminal => "Terminal",
            DefaultSessionMode::TabConfig => "Tab Config",
            DefaultSessionMode::DockerSandbox => "Local Docker Sandbox",
        }
    }
}

define_settings_group!(NewSessionSettings, settings: [
    // The raw stored default mode for new sessions. Use `default_session_mode()` to retrieve the
    // effective value, which is gated on feature availability.
    default_session_mode_internal: DefaultSessionMode,

    // The file path of the tab config used when default_session_mode_internal is TabConfig.
    // Machine-local (tab config paths vary per machine), so never synced to cloud.
    default_tab_config_path: DefaultTabConfigPath {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "general.default_tab_config_path",
    }
]);

impl NewSessionSettings {
    pub fn default_session_mode(&self) -> DefaultSessionMode {
        let mode = *self.default_session_mode_internal.value();
        match mode {
            DefaultSessionMode::Terminal | DefaultSessionMode::TabConfig => mode,
            // Fall back to Terminal when the feature is disabled so a stale stored value doesn't
            // wedge the user.
            DefaultSessionMode::DockerSandbox => {
                if FeatureFlag::LocalDockerSandbox.is_enabled() {
                    mode
                } else {
                    DefaultSessionMode::Terminal
                }
            }
        }
    }

    /// Returns the stored default tab config path (only meaningful when mode is `TabConfig`).
    pub fn default_tab_config_path(&self) -> &str {
        &self.default_tab_config_path
    }

    /// Looks up the `TabConfig` matching the stored `default_tab_config_path`.
    /// Returns `None` if the path is empty or no loaded config matches.
    pub fn resolved_default_tab_config(
        &self,
        app: &AppContext,
    ) -> Option<crate::tab_configs::TabConfig> {
        let path_str = self.default_tab_config_path.as_str();
        if path_str.is_empty() {
            return None;
        }
        let path = std::path::Path::new(path_str);
        crate::user_config::WarpConfig::as_ref(app)
            .tab_configs()
            .iter()
            .find(|config| config.source_path.as_deref().is_some_and(|p| p == path))
            .cloned()
    }
}
