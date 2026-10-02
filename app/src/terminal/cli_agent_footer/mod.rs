pub(super) mod chips;
pub mod editor;
pub mod toolbar_item;

#[cfg(not(target_family = "wasm"))]
use std::env;
#[cfg(not(target_family = "wasm"))]
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(not(target_family = "wasm"))]
use std::time::Duration;

use parking_lot::FairMutex;
use pathfinder_color::ColorU;
use pathfinder_geometry::vector::Vector2F;
#[cfg(not(target_family = "wasm"))]
use tokio::fs;
use toolbar_item::CLIAgentToolbarItemKind;
use warp_core::ui::color::ContrastingColor;
use warp_core::ui::color::blend::Blend;
use warp_core::ui::color::contrast::MinimumAllowedContrast;
use warp_core::ui::theme::Fill;
use warp_core::ui::theme::color::internal_colors;
#[cfg(not(target_family = "wasm"))]
use warp_errors::report_error;
#[cfg(not(target_family = "wasm"))]
use warpui::r#async::Timer;
use warpui::elements::{
    ChildView, ConstrainedBox, Container, CrossAxisAlignment, DispatchEventResult, Element,
    EventHandler, Flex, MainAxisAlignment, MainAxisSize, ParentElement, Wrap, WrapFill,
    WrapFillEntireRun,
};
use warpui::{
    AppContext, Entity, EntityId, ModelHandle, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use crate::appearance::Appearance;
use crate::completer::SessionContext;
use crate::context_chips::display_chip::{DisplayChip, DisplayChipConfig, PromptChipShellCommand};
use crate::context_chips::prompt_type::PromptType;
use crate::context_chips::{self, ContextChipKind};
use crate::features::FeatureFlag;
use crate::settings::{CLIAgentSettings, CodeSettings, CodeSettingsChangedEvent};
#[cfg(not(target_family = "wasm"))]
use crate::settings_view::SettingsSection;
#[cfg(not(target_family = "wasm"))]
use crate::terminal::ShellLaunchData;
#[cfg(not(target_family = "wasm"))]
use crate::terminal::cli_agent_sessions::plugin_manager::{
    CliAgentPluginManager, PluginInstallError, PluginModalKind, compare_versions,
    plugin_manager_for, plugin_manager_for_with_shell,
};
use crate::terminal::cli_agent_sessions::{
    CLIAgentInputState, CLIAgentSessionsModel, CLIAgentSessionsModelEvent,
};
#[cfg(not(target_family = "wasm"))]
use crate::terminal::local_shell::LocalShellState;
use crate::terminal::session_settings::{SessionSettings, SessionSettingsChangedEvent};
use crate::terminal::view::init::{ATTACH_FILE_KEYBINDING, OPEN_CLI_AGENT_RICH_INPUT_KEYBINDING};
use crate::terminal::{CLIAgent, TerminalModel};
use crate::ui_components::icons::Icon;
use crate::view_components::DismissibleToast;
#[cfg(not(target_family = "wasm"))]
use crate::view_components::ToastLink;
use crate::view_components::action_button::{
    ActionButton, ActionButtonTheme, AdjoinedSide, ButtonSize, KeystrokeSource, TooltipAlignment,
};
use crate::workspace::ToastStack;
#[cfg(not(target_family = "wasm"))]
use crate::workspace::WorkspaceAction;
use crate::workspace::view::TOGGLE_PROJECT_EXPLORER_BINDING_NAME;

/// How long to wait after session creation before showing the install chip.
/// Gives the plugin time to connect and send its `SessionStart` event.
#[cfg(not(target_family = "wasm"))]
const PLUGIN_CHIP_DEBOUNCE: Duration = Duration::from_secs(3);

#[cfg_attr(target_family = "wasm", allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginChipKind {
    Install,
    Update,
}

/// Builds a composite key for per-agent, per-host plugin chip dismissal.
/// Returns `"<agent_prefix>"` for local sessions or `"<agent_prefix>@<host>"` for remote.
fn plugin_chip_key(agent_prefix: &str, remote_host: &Option<String>) -> String {
    match remote_host {
        Some(host) => format!("{agent_prefix}@{host}"),
        None => agent_prefix.to_owned(),
    }
}

/// Footer control bar rendered while a third-party CLI agent (Claude Code, Codex, etc.) runs in
/// a terminal: agent icon, plugin install/update chip, and the configurable toolbar items.
///
/// A single `ViewHandle<CLIAgentFooter>` is shared between the rich input (`Input`) and the
/// long-running command toolbar (`UseAgentToolbar`), so it renders identically in both.
pub struct CLIAgentFooter {
    terminal_view_id: EntityId,
    file_button: ViewHandle<ActionButton>,
    display_chips: Vec<ViewHandle<DisplayChip>>,
    display_chip_config: DisplayChipConfig,

    terminal_model: Arc<FairMutex<TerminalModel>>,

    /// Opens the file explorer side panel.
    file_explorer_button: ViewHandle<ActionButton>,
    rich_input_button: ViewHandle<ActionButton>,
    settings_button: ViewHandle<ActionButton>,
    install_plugin_button: ViewHandle<ActionButton>,
    plugin_instructions_button: ViewHandle<ActionButton>,
    update_plugin_button: ViewHandle<ActionButton>,
    update_instructions_button: ViewHandle<ActionButton>,
    dismiss_plugin_chip_button: ViewHandle<ActionButton>,
    plugin_operation_in_progress: bool,
    /// When `true`, the install chip is allowed to render.
    /// Starts `false` and is set to `true` after a debounce timer fires,
    /// giving the plugin time to connect before we prompt installation.
    /// Reset to `false` when a listener connects.
    plugin_chip_ready: bool,
}

impl CLIAgentFooter {
    pub fn new(
        terminal_view_id: EntityId,
        terminal_model: Arc<FairMutex<TerminalModel>>,
        prompt: ModelHandle<PromptType>,
        display_chip_config: DisplayChipConfig,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let button_size = ButtonSize::AgentInputButton;

        let file_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("", AgentInputButtonTheme)
                .with_icon(Icon::Plus)
                .with_tooltip("Attach file")
                .with_tooltip_keybinding(ATTACH_FILE_KEYBINDING)
                .with_size(button_size)
                .with_tooltip_alignment(TooltipAlignment::Left)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(CLIAgentFooterAction::SelectFile);
                })
        });

        let cli_button_size = ButtonSize::AgentInputButton;
        let file_explorer_button = ctx.add_typed_action_view(|ctx| {
            ActionButton::new("File explorer", AgentInputButtonTheme)
                .with_icon(Icon::FileCopy)
                .with_tooltip("Open file explorer")
                .with_size(cli_button_size)
                .with_tooltip_alignment(TooltipAlignment::Left)
                .with_keybinding(
                    KeystrokeSource::Binding(TOGGLE_PROJECT_EXPLORER_BINDING_NAME),
                    ctx,
                )
                .with_compact_keybinding(true)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(CLIAgentFooterAction::ToggleFileExplorer);
                })
        });
        // CLI agent-specific buttons (only rendered when a CLI agent session is active).
        let rich_input_button = ctx.add_typed_action_view(|ctx| {
            ActionButton::new("Rich Input", AgentInputButtonTheme)
                .with_icon(Icon::TextInput)
                .with_tooltip("Open Rich Input")
                .with_size(cli_button_size)
                .with_tooltip_alignment(TooltipAlignment::Left)
                .with_keybinding(
                    KeystrokeSource::Binding(OPEN_CLI_AGENT_RICH_INPUT_KEYBINDING),
                    ctx,
                )
                .with_compact_keybinding(true)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(CLIAgentFooterAction::ToggleRichInput);
                })
        });
        let settings_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("", AgentInputButtonTheme)
                .with_icon(Icon::Settings)
                .with_tooltip("Open coding agent settings")
                .with_size(cli_button_size)
                .with_tooltip_alignment(TooltipAlignment::Left)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(CLIAgentFooterAction::OpenCodingAgentSettings);
                })
        });

        let install_plugin_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("Enable notifications", InstallPluginButtonTheme)
                .with_icon(Icon::Download)
                .with_tooltip(
                    "Install the Warp plugin to enable rich agent notifications within Warp",
                )
                .with_size(cli_button_size)
                .with_tooltip_alignment(TooltipAlignment::Left)
                .with_adjoined_side(AdjoinedSide::Right)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(CLIAgentFooterAction::InstallPlugin);
                })
        });

        let plugin_instructions_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("Notifications setup instructions", InstallPluginButtonTheme)
                .with_icon(Icon::Info)
                .with_tooltip("View instructions to install the Warp plugin")
                .with_size(cli_button_size)
                .with_tooltip_alignment(TooltipAlignment::Left)
                .with_adjoined_side(AdjoinedSide::Right)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(
                        CLIAgentFooterAction::OpenPluginInstallInstructionsPane,
                    );
                })
        });

        let update_plugin_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("Update Warp plugin", InstallPluginButtonTheme)
                .with_icon(Icon::Download)
                .with_tooltip("A new version of the Warp plugin is available")
                .with_size(cli_button_size)
                .with_tooltip_alignment(TooltipAlignment::Left)
                .with_adjoined_side(AdjoinedSide::Right)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(CLIAgentFooterAction::UpdatePlugin);
                })
        });

        let update_instructions_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("Plugin update instructions", InstallPluginButtonTheme)
                .with_icon(Icon::Info)
                .with_tooltip("View instructions to update the Warp plugin")
                .with_size(cli_button_size)
                .with_tooltip_alignment(TooltipAlignment::Left)
                .with_adjoined_side(AdjoinedSide::Right)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(
                        CLIAgentFooterAction::OpenPluginUpdateInstructionsPane,
                    );
                })
        });

        let dismiss_plugin_chip_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("", InstallPluginButtonTheme)
                .with_icon(Icon::X)
                .with_size(cli_button_size)
                .with_tooltip("Dismiss")
                .with_tooltip_alignment(TooltipAlignment::Left)
                .with_adjoined_side(AdjoinedSide::Left)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(CLIAgentFooterAction::DismissPluginChip);
                })
        });

        // Toggle rich input button label when CLI input session opens/closes.
        ctx.subscribe_to_model(
            &CLIAgentSessionsModel::handle(ctx),
            move |me, _, event, ctx| {
                if event.terminal_view_id() != terminal_view_id {
                    return;
                }

                // Reset the debounce when a session ends so the next
                // session gets a fresh debounce window.
                if let CLIAgentSessionsModelEvent::Ended { .. } = event {
                    me.plugin_chip_ready = false;
                }

                // When a structured plugin connects, the plugin is verified
                // installed — hide the chip. Codex's OSC 9 fallback is not a
                // structured plugin, so its chip stays until the plugin connects.
                if CLIAgentSessionsModel::as_ref(ctx)
                    .session(me.terminal_view_id)
                    .is_some_and(|s| s.supports_rich_status())
                {
                    me.plugin_chip_ready = false;
                }

                // When a session starts, update the install chip label and
                // start a debounce timer for non-auto-install agents.
                #[cfg(not(target_family = "wasm"))]
                if let CLIAgentSessionsModelEvent::Started { .. } = event
                    && let Some(agent) = me.cli_agent(ctx)
                {
                    let label = format!("Enable {} notifications", agent.display_name());
                    me.install_plugin_button.update(ctx, |button, ctx| {
                        button.set_label(label, ctx);
                    });
                    if let Some(manager) = plugin_manager_for(agent)
                        && !manager.can_auto_install()
                    {
                        ctx.spawn(
                            Timer::after(PLUGIN_CHIP_DEBOUNCE),
                            |me, _, ctx: &mut ViewContext<Self>| {
                                let suppress = CLIAgentSessionsModel::as_ref(ctx)
                                    .session(me.terminal_view_id)
                                    .is_some_and(|s| s.supports_rich_status());
                                if !suppress {
                                    me.plugin_chip_ready = true;
                                    ctx.notify();
                                }
                            },
                        );
                    }
                }

                let CLIAgentSessionsModelEvent::InputSessionChanged {
                    new_input_state, ..
                } = event
                else {
                    ctx.notify();
                    return;
                };
                let is_open = matches!(new_input_state, CLIAgentInputState::Open);
                me.rich_input_button.update(ctx, |button, ctx| {
                    if is_open {
                        button.set_label("Hide Rich Input", ctx);
                        button.set_tooltip(Some("Hide Rich Input"), ctx);
                        button.set_keybinding(
                            Some(KeystrokeSource::Binding(
                                OPEN_CLI_AGENT_RICH_INPUT_KEYBINDING,
                            )),
                            ctx,
                        );
                    } else {
                        button.set_label("Rich Input", ctx);
                        button.set_tooltip(Some("Open Rich Input"), ctx);
                        button.set_keybinding(
                            Some(KeystrokeSource::Binding(
                                OPEN_CLI_AGENT_RICH_INPUT_KEYBINDING,
                            )),
                            ctx,
                        );
                    }
                });
                ctx.notify();
            },
        );

        // The File explorer item's availability follows this setting, so the footer has to
        // repaint when it is toggled rather than waiting for an unrelated re-render.
        ctx.subscribe_to_model(&CodeSettings::handle(ctx), |_, _, event, ctx| {
            if matches!(event, CodeSettingsChangedEvent::ShowProjectExplorer { .. }) {
                ctx.notify()
            }
        });

        let prompt_for_session_settings = prompt.clone();
        ctx.subscribe_to_model(&SessionSettings::handle(ctx), move |me, _, event, ctx| {
            if let SessionSettingsChangedEvent::CLIAgentToolbarChipSelectionSetting { .. }
            | SessionSettingsChangedEvent::GithubPrChipDefaultValidation { .. } = event
            {
                me.update_display_chips(&prompt_for_session_settings, ctx);
                ctx.notify();
            }
        });

        ctx.observe(&prompt, |me, model, ctx| {
            me.update_display_chips(&model, ctx);
        });

        let mut me = Self {
            terminal_view_id,
            file_button,
            file_explorer_button,
            rich_input_button,
            settings_button,
            install_plugin_button,
            plugin_instructions_button,
            update_plugin_button,
            update_instructions_button,
            dismiss_plugin_chip_button,
            plugin_operation_in_progress: false,
            plugin_chip_ready: false,
            terminal_model,
            display_chips: vec![],
            display_chip_config,
        };
        me.update_display_chips(&prompt, ctx);
        me
    }

    pub fn set_current_repo_path(
        &mut self,
        repo_path: Option<std::path::PathBuf>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.display_chip_config.current_repo_path = repo_path;
        // Chips will be rebuilt on the next GitRepoStatusEvent::MetadataChanged.
        // Notify to ensure any existing chips reflect the change.
        ctx.notify();
    }

    pub fn update_session_context(
        &mut self,
        session_context: Option<SessionContext>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.display_chip_config.session_context = session_context.clone();
        for chip_view in &self.display_chips {
            chip_view.update(ctx, |chip, chip_ctx| {
                chip.update_session_context(session_context.clone(), chip_ctx);
            });
        }
    }

    fn has_active_cli_agent_input_session(&self, app: &AppContext) -> bool {
        CLIAgentSessionsModel::as_ref(app).is_input_open(self.terminal_view_id)
    }

    fn cli_agent(&self, app: &AppContext) -> Option<CLIAgent> {
        CLIAgentSessionsModel::as_ref(app)
            .session(self.terminal_view_id)
            .map(|session| session.agent)
    }

    pub(crate) fn select_file(&mut self, ctx: &mut ViewContext<Self>) {
        let window_id = ctx.window_id();
        let view_id = ctx.view_id();
        let file_picker_config = warpui::platform::FilePickerConfiguration::new();

        ctx.open_file_picker(
            move |result, ctx| match result {
                Ok(paths) => {
                    if let Some(path) = paths.first() {
                        ctx.dispatch_typed_action_for_view(
                            window_id,
                            view_id,
                            &CLIAgentFooterAction::InsertFilePath(path.clone()),
                        );
                    }
                }
                Err(err) => {
                    let window_id = ctx.window_id();
                    ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
                        toast_stack.add_ephemeral_toast(
                            DismissibleToast::error(format!("{err}")),
                            window_id,
                            ctx,
                        );
                    });
                }
            },
            file_picker_config,
        );
    }

    /// Which plugin chip to show, if any.
    fn plugin_chip_kind(&self, app: &AppContext) -> Option<PluginChipKind> {
        #[cfg(target_family = "wasm")]
        {
            let _ = (app, self.plugin_operation_in_progress);
            None
        }
        #[cfg(not(target_family = "wasm"))]
        {
            if self.plugin_operation_in_progress || !FeatureFlag::HOANotifications.is_enabled() {
                return None;
            }

            let cli_agent_settings = CLIAgentSettings::as_ref(app);
            if !*cli_agent_settings.show_agent_notifications {
                return None;
            }

            let session = CLIAgentSessionsModel::as_ref(app).session(self.terminal_view_id)?;

            let manager = plugin_manager_for(session.agent)?;
            let min_version = manager.minimum_plugin_version();
            let chip_key = plugin_chip_key(session.agent.command_prefix(), &session.remote_host);
            // If a structured plugin is connected and this agent supports
            // version-based updates, check the reported version.
            if session.supports_rich_status() && manager.supports_update() {
                let needs_update = match &session.plugin_version {
                    // No version reported = pre-versioning plugin, definitely outdated.
                    None => true,
                    Some(v) => compare_versions(v, min_version).is_lt(),
                };
                if !needs_update {
                    return None;
                }
                // Check update chip dismissal.
                let dismissed_version =
                    cli_agent_settings.plugin_update_chip_dismissed_version(&chip_key);
                if !dismissed_version.is_empty()
                    && compare_versions(dismissed_version, min_version).is_ge()
                {
                    return None;
                }
                return Some(PluginChipKind::Update);
            }

            // For agents without auto-install, wait for the debounce timer
            // before showing the install chip.
            if !manager.can_auto_install() && !self.plugin_chip_ready {
                return None;
            }

            let install_chip_dismissed =
                cli_agent_settings.is_plugin_install_chip_dismissed(&chip_key);

            // For remote sessions, we can't check the filesystem.
            if session.is_remote() {
                return (!install_chip_dismissed).then_some(PluginChipKind::Install);
            }

            if manager.is_installed() {
                // Installed but no listener yet. Check the on-disk version as a fallback
                // — the plugin may be too old to send structured events.
                if manager.needs_update() {
                    let dismissed_version =
                        cli_agent_settings.plugin_update_chip_dismissed_version(&chip_key);
                    if !dismissed_version.is_empty()
                        && compare_versions(dismissed_version, min_version).is_ge()
                    {
                        return None;
                    }
                    return Some(PluginChipKind::Update);
                }
                // Up to date on disk — wait for the listener to connect.
                return None;
            }

            // Not installed locally.
            (!install_chip_dismissed).then_some(PluginChipKind::Install)
        }
    }

    /// Whether the chip should open the manual instructions modal instead of auto-operating.
    fn should_use_manual_mode(&self, app: &AppContext) -> bool {
        let sessions_model = CLIAgentSessionsModel::as_ref(app);
        let session = match sessions_model.session(self.terminal_view_id) {
            Some(s) => s,
            None => return false,
        };

        // Custom toolbar commands always use manual mode because the user's
        // binary may differ from the agent's standard CLI tool.
        if session.custom_command_prefix.is_some() {
            return true;
        }

        #[cfg(not(target_family = "wasm"))]
        if let Some(manager) = plugin_manager_for(session.agent)
            && !manager.can_auto_install()
        {
            return true;
        }
        if session.is_remote() {
            return true;
        }
        // Docker sandbox sessions run inside a container; our auto-install
        // path operates on the host's shell config and would install the
        // plugin in the wrong place. Fall back to the manual-instructions
        // modal so the user can paste the install command into the sandbox
        // PTY and install inside the container.
        //
        // `ShellLaunchData` is only available on native builds; wasm builds
        // can't produce a `DockerSandbox` variant either way.
        #[cfg(not(target_family = "wasm"))]
        {
            let shell_data = {
                let model = self.terminal_model.lock();
                model.active_shell_launch_data().cloned()
            };
            if matches!(shell_data, Some(ShellLaunchData::DockerSandbox { .. })) {
                return true;
            }
        }
        sessions_model.has_plugin_auto_failed(session.agent, &session.remote_host)
    }

    /// Records that the auto plugin operation could not start, shows an error toast,
    /// and re-renders so the chip switches to manual-instructions mode.
    #[cfg(not(target_family = "wasm"))]
    fn record_plugin_auto_failure_and_notify(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(agent) = self.cli_agent(ctx) {
            let remote_host = CLIAgentSessionsModel::as_ref(ctx)
                .session(self.terminal_view_id)
                .and_then(|s| s.remote_host.clone());
            CLIAgentSessionsModel::handle(ctx).update(ctx, |model, _| {
                model.record_plugin_auto_failure(agent, remote_host);
            });
        }
        let window_id = ctx.window_id();
        ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
            toast_stack.add_ephemeral_toast(
                DismissibleToast::error(
                    "Could not automatically install plugin. \
                     Please click the chip again for manual installation steps."
                        .to_owned(),
                ),
                window_id,
                ctx,
            );
        });
        ctx.notify();
    }

    /// Shared handler for both install and update plugin operations.
    /// `progress_toast` is shown while the operation runs; `success_toast` on success.
    #[cfg(not(target_family = "wasm"))]
    fn handle_plugin_operation<F, Fut>(
        &mut self,
        progress_toast: &str,
        error_label: &str,
        success_toast: &str,
        operation: F,
        ctx: &mut ViewContext<Self>,
    ) -> bool
    where
        F: FnOnce(Box<dyn CliAgentPluginManager>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), PluginInstallError>> + Send + 'static,
    {
        let Some(agent) = self.cli_agent(ctx) else {
            return false;
        };
        let shell_data = {
            let model = self.terminal_model.lock();
            model.active_shell_launch_data().cloned()
        };
        let (shell_path, shell_type) = match shell_data {
            Some(ShellLaunchData::Executable {
                executable_path,
                shell_type,
            })
            | Some(ShellLaunchData::MSYS2 {
                executable_path,
                shell_type,
            }) => (Some(executable_path), Some(shell_type)),
            // Shell not yet resolved (e.g. still bootstrapping).
            None => (None, None),
            // WSL is not supported for auto-install.
            Some(ShellLaunchData::WSL { .. }) => return false,
            // Auto-install isn't supported for Docker sandbox sessions — the
            // install would run against the *host's* shell config, not the
            // container's. `should_use_manual_mode` already routes sandbox
            // sessions to the manual-instructions modal, so this arm is a
            // defensive fallthrough; users can still install the plugin by
            // pasting the command into the sandbox PTY themselves.
            //
            // TODO(advait): Add native auto-install support for sandboxes,
            // e.g. by routing the install through the session's in-band
            // executor so it runs inside the container and targets the
            // container's shell / package layout. A common use case will be
            // running a 3p harness (e.g. Claude Code) inside a sandbox and
            // needing the Warp plugin to integrate with it.
            Some(ShellLaunchData::DockerSandbox { .. }) => return false,
        };

        // Await the interactive PATH so nvm-installed tools like `claude`
        // are on PATH, matching how LSP operations capture the PATH.
        let path_future = LocalShellState::handle(ctx).update(ctx, |shell_state, ctx| {
            shell_state.get_interactive_path_env_var(ctx)
        });

        self.plugin_operation_in_progress = true;
        ctx.notify();

        let window_id = ctx.window_id();
        let toast_id = "cli-agent-plugin-operation".to_owned();

        ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
            toast_stack.add_persistent_toast(
                DismissibleToast::default(progress_toast.to_owned())
                    .with_object_id(toast_id.clone()),
                window_id,
                ctx,
            );
        });

        let toast_id_for_callback = toast_id.clone();
        let error_label = error_label.to_owned();
        let success_toast = success_toast.to_owned();
        ctx.spawn(
            async move {
                let path_env_var = path_future.await;
                let Some(manager) =
                    plugin_manager_for_with_shell(agent, shell_path, shell_type, path_env_var)
                else {
                    return Err((
                        PluginInstallError {
                            message: "No plugin manager available".to_owned(),
                            log: String::new(),
                        },
                        None,
                    ));
                };

                match operation(manager).await {
                    Ok(()) => Ok(()),
                    Err(err) => {
                        let log_path = write_install_log(agent, &err).await;
                        Err((err, log_path))
                    }
                }
            },
            move |me, result, ctx| {
                me.plugin_operation_in_progress = false;

                if result.is_ok() {
                    ctx.emit(CLIAgentFooterEvent::PluginInstalled(agent));
                }

                ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
                    let toast = match result {
                        Ok(()) => DismissibleToast::success(success_toast.clone()),
                        Err((err, log_path)) => {
                            let remote_host = CLIAgentSessionsModel::as_ref(ctx)
                                .session(me.terminal_view_id)
                                .and_then(|s| s.remote_host.clone());
                            CLIAgentSessionsModel::handle(ctx).update(ctx, |model, _| {
                                model.record_plugin_auto_failure(agent, remote_host);
                            });
                            log::error!("Failed plugin operation log: {}", err.log);
                            let mut toast =
                                DismissibleToast::error(format!("{error_label}: {err}"));
                            report_error!(
                                anyhow::Error::new(err).context("Failed plugin operation"),
                                extra: { "agent" => ?agent }
                            );
                            if let Some(log_path) = log_path {
                                toast = toast.with_link(
                                    ToastLink::new("See logs for details".to_owned())
                                        .with_onclick_action(WorkspaceAction::OpenFilePath {
                                            path: log_path,
                                        }),
                                );
                            }
                            toast
                        }
                    };
                    toast_stack.add_ephemeral_toast(
                        toast.with_object_id(toast_id_for_callback),
                        window_id,
                        ctx,
                    );
                });
                ctx.notify();
            },
        );
        true
    }

    #[cfg(not(target_family = "wasm"))]
    fn handle_install_plugin(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        let success_msg = self
            .cli_agent(ctx)
            .and_then(plugin_manager_for)
            .map(|m| m.install_success_message())
            .unwrap_or("Warp plugin installed. Please restart the session to activate.");
        self.handle_plugin_operation(
            "Installing Warp plugin...",
            "Failed to install Warp plugin",
            success_msg,
            |manager| async move { manager.install().await },
            ctx,
        )
    }

    #[cfg(not(target_family = "wasm"))]
    fn handle_update_plugin(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        let success_msg = self
            .cli_agent(ctx)
            .and_then(plugin_manager_for)
            .map(|m| m.update_success_message())
            .unwrap_or("Warp plugin updated. Please restart the session to activate.");
        self.handle_plugin_operation(
            "Updating Warp plugin...",
            "Failed to update Warp plugin",
            success_msg,
            |manager| async move { manager.update().await },
            ctx,
        )
    }

    fn cli_display_chip(
        &self,
        chip_kind: ContextChipKind,
        app: &AppContext,
    ) -> Option<Box<dyn Element>> {
        self.display_chips
            .iter()
            .find(|chip| chip.as_ref(app).chip_kind() == &chip_kind)
            .map(|chip| ChildView::new(chip).finish())
    }

    fn render_toolbar_item(
        &self,
        item: &CLIAgentToolbarItemKind,
        app: &AppContext,
    ) -> Option<Box<dyn Element>> {
        match item {
            CLIAgentToolbarItemKind::ContextChip(chip_kind) => {
                self.cli_display_chip(chip_kind.clone(), app)
            }
            CLIAgentToolbarItemKind::FileExplorer => item
                .is_available(app)
                .then(|| ChildView::new(&self.file_explorer_button).finish()),
            CLIAgentToolbarItemKind::RichInput => FeatureFlag::CLIAgentRichInput
                .is_enabled()
                .then(|| ChildView::new(&self.rich_input_button).finish()),
            CLIAgentToolbarItemKind::FileAttach => Some(ChildView::new(&self.file_button).finish()),
            CLIAgentToolbarItemKind::Settings => {
                Some(ChildView::new(&self.settings_button).finish())
            }
        }
    }

    fn render_footer(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let cli_icon_size = ButtonSize::AgentInputButton.icon_size(appearance, app);

        // Extract everything we need from the terminal model up front and drop
        // the lock before calling into helpers like `should_use_manual_mode`
        // and `render_cli_toolbar_item`, which may re-lock the same model and
        // would deadlock since the lock is non-reentrant.
        let background_color = {
            let terminal_model = self.terminal_model.lock();
            if terminal_model.is_alt_screen_active() {
                terminal_model
                    .alt_screen()
                    .inferred_bg_color()
                    .unwrap_or_else(|| appearance.theme().surface_1().into_solid())
            } else {
                appearance.theme().surface_1().into_solid()
            }
        };

        let session_settings = SessionSettings::as_ref(app);
        let left_items = session_settings
            .cli_agent_footer_chip_selection
            .left_items();
        let right_items = session_settings
            .cli_agent_footer_chip_selection
            .right_items();

        let mut left_buttons = Wrap::row()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::Start)
            .with_run_spacing(4.)
            .with_spacing(4.);

        // CLI agent brand icon is always rendered (not configurable).
        if let Some(agent) = self.cli_agent(app)
            && let Some(icon) = agent.icon()
        {
            let icon_color = agent
                .brand_color()
                .map(|c| c.on_background(background_color, MinimumAllowedContrast::NonText))
                .unwrap_or_else(|| appearance.theme().foreground().into_solid());
            left_buttons.add_child(
                Container::new(
                    ConstrainedBox::new(icon.to_warpui_icon(Fill::Solid(icon_color)).finish())
                        .with_width(cli_icon_size)
                        .with_height(cli_icon_size)
                        .finish(),
                )
                .with_padding_right(8.)
                .finish(),
            );
        }

        if let Some(chip_kind) = self.plugin_chip_kind(app) {
            let manual = self.should_use_manual_mode(app);
            let chip = match (chip_kind, manual) {
                (PluginChipKind::Install, false) => {
                    ChildView::new(&self.install_plugin_button).finish()
                }
                (PluginChipKind::Install, true) => {
                    ChildView::new(&self.plugin_instructions_button).finish()
                }
                (PluginChipKind::Update, false) => {
                    ChildView::new(&self.update_plugin_button).finish()
                }
                (PluginChipKind::Update, true) => {
                    ChildView::new(&self.update_instructions_button).finish()
                }
            };
            let chip_with_dismiss = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(chip)
                .with_child(ChildView::new(&self.dismiss_plugin_chip_button).finish())
                .finish();
            left_buttons.add_child(chip_with_dismiss);
        }

        for item in &left_items {
            if let Some(element) = self.render_toolbar_item(item, app) {
                left_buttons.add_child(element);
            }
        }

        let mut right_buttons = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min)
            .with_spacing(4.);

        for item in &right_items {
            if let Some(element) = self.render_toolbar_item(item, app) {
                right_buttons.add_child(element);
            }
        }

        let content = Wrap::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(WrapFillEntireRun::new(left_buttons.finish()).finish())
            .with_child(WrapFill::new(0., right_buttons.finish()).finish())
            .with_run_spacing(context_chips::spacing::UDI_ROW_RUN_SPACING)
            .finish();
        let content = EventHandler::new(content)
            .on_right_mouse_down(|ctx, _, position, _| {
                ctx.dispatch_typed_action(CLIAgentFooterAction::ShowContextMenu { position });
                DispatchEventResult::StopPropagation
            })
            .finish();

        Container::new(content).with_vertical_padding(4.).finish()
    }

    pub fn has_open_chip_menu(&self, app: &AppContext) -> bool {
        self.display_chips
            .iter()
            .any(|chip| chip.as_ref(app).display_chip_kind().has_open_menu())
    }

    #[cfg(test)]
    pub fn settings_button_id(&self) -> EntityId {
        self.settings_button.id()
    }

    #[cfg(test)]
    pub fn file_button_id(&self) -> EntityId {
        self.file_button.id()
    }

    #[cfg(test)]
    pub fn display_chip_kinds(&self, app: &AppContext) -> Vec<ContextChipKind> {
        self.display_chips
            .iter()
            .map(|chip| chip.as_ref(app).chip_kind().clone())
            .collect()
    }
}

impl View for CLIAgentFooter {
    fn ui_name() -> &'static str {
        "CLIAgentFooter"
    }

    fn render(&self, app: &warpui::AppContext) -> Box<dyn warpui::Element> {
        self.render_footer(app)
    }
}

#[derive(Debug, Clone)]
pub enum CLIAgentFooterAction {
    SelectFile,
    InsertFilePath(String),
    ToggleCodeReview,
    ToggleFileExplorer,
    ToggleRichInput,
    InstallPlugin,
    UpdatePlugin,
    OpenPluginInstallInstructionsPane,
    OpenPluginUpdateInstructionsPane,
    DismissPluginChip,
    OpenCodingAgentSettings,
    ShowContextMenu { position: Vector2F },
}

impl TypedActionView for CLIAgentFooter {
    type Action = CLIAgentFooterAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut warpui::ViewContext<Self>) {
        match action {
            CLIAgentFooterAction::SelectFile => {
                self.select_file(ctx);
            }
            CLIAgentFooterAction::InsertFilePath(path) => {
                let path_with_space = format!("{path} ");
                if self.has_active_cli_agent_input_session(ctx) {
                    ctx.emit(CLIAgentFooterEvent::InsertIntoCLIRichInput(path_with_space));
                } else {
                    ctx.emit(CLIAgentFooterEvent::WriteToPty(path_with_space));
                }
            }
            CLIAgentFooterAction::ToggleCodeReview => {
                if self.cli_agent(ctx).is_some() {
                    ctx.emit(CLIAgentFooterEvent::ToggleCodeReviewPane);
                }
            }
            CLIAgentFooterAction::ToggleFileExplorer => {
                ctx.emit(CLIAgentFooterEvent::ToggleFileExplorer);
            }
            CLIAgentFooterAction::ToggleRichInput => {
                if self.has_active_cli_agent_input_session(ctx) {
                    ctx.emit(CLIAgentFooterEvent::HideRichInput);
                } else {
                    ctx.emit(CLIAgentFooterEvent::OpenRichInput);
                }
            }
            CLIAgentFooterAction::InstallPlugin => {
                #[cfg(not(target_family = "wasm"))]
                {
                    if !self.handle_install_plugin(ctx) {
                        self.record_plugin_auto_failure_and_notify(ctx);
                    }
                }
            }
            CLIAgentFooterAction::UpdatePlugin => {
                #[cfg(not(target_family = "wasm"))]
                {
                    if !self.handle_update_plugin(ctx) {
                        self.record_plugin_auto_failure_and_notify(ctx);
                    }
                }
            }
            CLIAgentFooterAction::OpenPluginInstallInstructionsPane => {
                #[cfg(not(target_family = "wasm"))]
                if let Some(agent) = self.cli_agent(ctx) {
                    ctx.emit(CLIAgentFooterEvent::OpenPluginInstructionsPane(
                        agent,
                        PluginModalKind::Install,
                    ));
                }
            }
            CLIAgentFooterAction::OpenPluginUpdateInstructionsPane => {
                #[cfg(not(target_family = "wasm"))]
                if let Some(agent) = self.cli_agent(ctx) {
                    ctx.emit(CLIAgentFooterEvent::OpenPluginInstructionsPane(
                        agent,
                        PluginModalKind::Update,
                    ));
                }
            }
            CLIAgentFooterAction::DismissPluginChip => {
                let is_update = matches!(self.plugin_chip_kind(ctx), Some(PluginChipKind::Update));
                let session = CLIAgentSessionsModel::as_ref(ctx)
                    .session(self.terminal_view_id)
                    .cloned();
                if let Some(session) = session {
                    let chip_key =
                        plugin_chip_key(session.agent.command_prefix(), &session.remote_host);
                    if is_update {
                        #[cfg(not(target_family = "wasm"))]
                        if let Some(manager) = plugin_manager_for(session.agent) {
                            let version = manager.minimum_plugin_version().to_owned();
                            CLIAgentSettings::handle(ctx).update(ctx, |settings, ctx| {
                                settings.dismiss_plugin_update_chip(&chip_key, version, ctx);
                            });
                        }
                    } else {
                        CLIAgentSettings::handle(ctx).update(ctx, |settings, ctx| {
                            settings.dismiss_plugin_install_chip(&chip_key, ctx);
                        });
                    }
                }
                ctx.notify();
            }
            CLIAgentFooterAction::OpenCodingAgentSettings => {
                #[cfg(not(target_family = "wasm"))]
                ctx.dispatch_typed_action_deferred(WorkspaceAction::ScrollToSettingsWidget {
                    page: SettingsSection::ThirdPartyCLIAgents,
                    widget_id: crate::settings_view::cli_agent_settings_widget_id(),
                });
            }
            CLIAgentFooterAction::ShowContextMenu { position } => {
                ctx.emit(CLIAgentFooterEvent::ShowContextMenu {
                    position: *position,
                });
            }
        }
    }
}

pub enum CLIAgentFooterEvent {
    WriteToPty(String),
    /// Insert text into the CLI agent rich input.
    InsertIntoCLIRichInput(String),
    ToggleCodeReviewPane,
    /// Toggle the file explorer side panel.
    ToggleFileExplorer,
    OpenRichInput,
    HideRichInput,
    ToggledChipMenu {
        open: bool,
    },
    TryExecuteChipCommand(PromptChipShellCommand),
    OpenCodeReview,
    ShowContextMenu {
        position: Vector2F,
    },
    PluginInstalled(CLIAgent),
    #[cfg(not(target_family = "wasm"))]
    OpenPluginInstructionsPane(CLIAgent, PluginModalKind),
}

impl Entity for CLIAgentFooter {
    type Event = CLIAgentFooterEvent;
}

pub(crate) struct AgentInputButtonTheme;

impl ActionButtonTheme for AgentInputButtonTheme {
    fn background(&self, hovered: bool, appearance: &Appearance) -> Option<Fill> {
        // Solid surface fills keep the button readable even when its parent
        // isn't `theme.background()` (for example, over an alt-screen CLI agent).
        let theme = appearance.theme();
        Some(if hovered {
            theme.surface_2()
        } else {
            theme.surface_1()
        })
    }

    fn text_color(
        &self,
        _hovered: bool,
        background: Option<Fill>,
        appearance: &Appearance,
    ) -> ColorU {
        // If a caller overrides `background()` with a translucent fill, blend
        // it over `surface_1` so text contrast is computed against the actual
        // rendered color rather than the raw overlay.
        let base_bg = appearance.theme().surface_1();
        let effective_bg = background
            .map(|overlay| base_bg.blend(&overlay))
            .unwrap_or(base_bg);

        appearance.theme().sub_text_color(effective_bg).into_solid()
    }

    fn border(&self, appearance: &Appearance) -> Option<ColorU> {
        Some(internal_colors::neutral_3(appearance.theme()))
    }

    fn should_opt_out_of_contrast_adjustment(&self) -> bool {
        true
    }
}

/// Green-accented theme for the "Install Warp plugin" chip.
struct InstallPluginButtonTheme;

impl ActionButtonTheme for InstallPluginButtonTheme {
    fn background(&self, hovered: bool, appearance: &Appearance) -> Option<Fill> {
        let green = appearance.theme().ansi_fg_green();
        let base = appearance.theme().surface_1();
        Some(if hovered {
            base.blend(&Fill::Solid(green).with_opacity(30))
        } else {
            base.blend(&Fill::Solid(green).with_opacity(15))
        })
    }

    fn text_color(
        &self,
        _hovered: bool,
        _background: Option<Fill>,
        appearance: &Appearance,
    ) -> ColorU {
        appearance.theme().ansi_fg_green()
    }

    fn border(&self, appearance: &Appearance) -> Option<ColorU> {
        let green = appearance.theme().ansi_fg_green();
        Some(ColorU::new(green.r, green.g, green.b, 80))
    }

    fn should_opt_out_of_contrast_adjustment(&self) -> bool {
        true
    }
}

/// Writes the detailed plugin installation log to a temp file.
/// Returns the log file path on success, or `None` if writing failed.
#[cfg(not(target_family = "wasm"))]
async fn write_install_log(agent: CLIAgent, err: &PluginInstallError) -> Option<PathBuf> {
    let log_path = env::temp_dir().join("warp-plugin-install.log");
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    let contents = format!(
        "Warp plugin installation — {agent:?}\n\
         {now}\n\
         \n\
         {log}",
        log = err.log,
    );
    fs::write(&log_path, contents).await.ok()?;
    Some(log_path)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
