#[cfg(feature = "local_fs")]
use agent_context::skills::SKILL_PROVIDER_DEFINITIONS;
#[cfg(feature = "local_fs")]
use repo_metadata::RepoMetadataModel;
use repo_metadata::repositories::DetectedRepositories;
use repo_metadata::watcher::DirectoryWatcher;
use warp_core::ui::appearance::Appearance;
use warpui::platform::WindowStyle;
use warpui::{App, ViewHandle, WindowId};
use watcher::HomeDirectoryWatcher;

use super::settings::initialize_history_persistence_for_tests;
use crate::AgentNotificationsModel;
use crate::code_review::git_repo_model::GitRepoModels;
use crate::context_chips::prompt::Prompt;
use crate::persisted_workspace::PersistedWorkspace;
use crate::repo_outlines::RepoOutlines;
use crate::search::files::model::FileSearchModel;
use crate::settings::PrivacySettings;
use crate::settings_view::keybindings::KeybindingChangedNotifier;
use crate::skills::SkillManager;
use crate::suggestions::ignored_suggestions_model::IgnoredSuggestionsModel;
use crate::system::{SystemInfo, SystemStats};
use crate::terminal::alt_screen_reporting::AltScreenReporting;
use crate::terminal::cli_agent_sessions::CLIAgentSessionsModel;
use crate::terminal::model::block::SerializedBlock;
use crate::terminal::resizable_data::ResizableData;
use crate::terminal::{History, TerminalView};
use crate::undo_close::UndoCloseStack;
use crate::warp_managed_paths_watcher::WarpManagedPathsWatcher;
use crate::workspace::sync_inputs::SyncedInputState;
use crate::workspace::{ActiveSession, WorkspaceRegistry};

/// Initializes all of the necessary models to use a terminal view.
pub fn initialize_app_for_terminal_view(app: &mut App) {
    initialize_history_persistence_for_tests(app);

    app.add_singleton_model(|_| SystemStats::new());
    app.add_singleton_model(|_| Prompt::mock());
    app.add_singleton_model(|_| Appearance::mock());
    app.add_singleton_model(PrivacySettings::mock);
    app.add_singleton_model(|_ctx| SyncedInputState::mock());
    app.add_singleton_model(|_| ResizableData::default());
    app.add_singleton_model(|_| History::default());
    app.add_singleton_model(|_| CLIAgentSessionsModel::new());
    app.add_singleton_model(AgentNotificationsModel::new);
    app.add_singleton_model(UndoCloseStack::new);
    app.add_singleton_model(|_| KeybindingChangedNotifier::new());
    app.add_singleton_model(|_| ActiveSession::default());
    app.add_singleton_model(DirectoryWatcher::new);
    app.add_singleton_model(|_| DetectedRepositories::default());
    #[cfg(feature = "local_fs")]
    app.add_singleton_model(|ctx| {
        let model = RepoMetadataModel::new(ctx);
        model.register_force_included_paths(
            SKILL_PROVIDER_DEFINITIONS
                .iter()
                .map(|provider| provider.skills_path.clone()),
            ctx,
        );
        model.set_project_skill_provider_paths(
            SKILL_PROVIDER_DEFINITIONS
                .iter()
                .map(|provider| provider.skills_path.clone()),
            ctx,
        );
        model
    });
    app.add_singleton_model(FileSearchModel::new);
    app.add_singleton_model(|_| GitRepoModels::new());
    app.add_singleton_model(RepoOutlines::new_for_test);
    app.add_singleton_model(HomeDirectoryWatcher::new_for_test);
    app.add_singleton_model(WarpManagedPathsWatcher::new_for_testing);
    app.add_singleton_model(SkillManager::new);

    #[cfg(not(target_family = "wasm"))]
    app.add_singleton_model(SystemInfo::new);

    app.add_singleton_model(|_| WorkspaceRegistry::new());
    app.add_singleton_model(|_| IgnoredSuggestionsModel::new(vec![]));
    app.add_singleton_model(PersistedWorkspace::new_for_test);
    AltScreenReporting::register(app);
}

/// Creates a window in `app` with a [`TerminalView`] as the root view.
/// Returns the handle to that terminal view.
pub fn add_window_with_terminal(
    app: &mut App,
    restored_blocks: Option<&[SerializedBlock]>,
) -> ViewHandle<TerminalView> {
    add_window_with_id_and_terminal(app, restored_blocks).1
}

/// Creates a window in `app` with a [`TerminalView`] as the root view.
/// Returns the WindowID and the handle to that terminal view.
pub fn add_window_with_id_and_terminal(
    app: &mut App,
    restored_blocks: Option<&[SerializedBlock]>,
) -> (WindowId, ViewHandle<TerminalView>) {
    app.add_window(WindowStyle::NotStealFocus, |ctx| {
        TerminalView::new_for_test(restored_blocks, ctx)
    })
}
