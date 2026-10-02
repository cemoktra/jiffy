use warp_core::features::FeatureFlag;
use warpui::{AppContext, Entity, EntityId, ModelContext, SingletonEntity, ViewHandle};

use super::{NotificationCategory, NotificationId, NotificationItem, NotificationItems};
use crate::terminal::cli_agent_sessions::{
    CLIAgentSessionStatus, CLIAgentSessionsModel, CLIAgentSessionsModelEvent,
};
use crate::terminal::{CLIAgent, TerminalView};
use crate::workspace::util::is_terminal_view_in_same_tab;
use crate::workspace::{Workspace, WorkspaceRegistry};

/// Singleton model responsible for triggering in-app notifications on CLI agent session status
/// updates and tracking/storing these notifications for the notifications mailbox.
pub struct AgentNotificationsModel {
    notifications: NotificationItems,
}

impl Entity for AgentNotificationsModel {
    type Event = AgentManagementEvent;
}

impl SingletonEntity for AgentNotificationsModel {}

impl AgentNotificationsModel {
    pub(crate) fn new(ctx: &mut ModelContext<Self>) -> Self {
        let cli_sessions_model = CLIAgentSessionsModel::handle(ctx);
        ctx.subscribe_to_model(&cli_sessions_model, |me, _, event, ctx| {
            me.handle_cli_agent_session_event(event, ctx);
        });

        Self {
            notifications: NotificationItems::default(),
        }
    }

    pub(crate) fn notifications(&self) -> &NotificationItems {
        &self.notifications
    }

    pub(crate) fn mark_item_read(&mut self, id: NotificationId, ctx: &mut ModelContext<Self>) {
        if self.notifications.mark_item_read(id) {
            ctx.emit(AgentManagementEvent::NotificationUpdated);
        }
    }

    pub(crate) fn mark_all_items_read(&mut self, ctx: &mut ModelContext<Self>) {
        if self.notifications.mark_all_items_read() {
            ctx.emit(AgentManagementEvent::AllNotificationsMarkedRead);
        }
    }

    /// Marks all notifications from the given terminal view as read.
    pub(crate) fn mark_items_from_terminal_view_read(
        &mut self,
        terminal_view_id: EntityId,
        ctx: &mut ModelContext<Self>,
    ) {
        if !FeatureFlag::HOANotifications.is_enabled() {
            return;
        }
        if self
            .notifications
            .mark_all_terminal_view_items_as_read(terminal_view_id)
        {
            ctx.emit(AgentManagementEvent::NotificationUpdated);
        }
    }

    fn handle_cli_agent_session_event(
        &mut self,
        event: &CLIAgentSessionsModelEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        if !FeatureFlag::HOANotifications.is_enabled() {
            return;
        }

        match event {
            CLIAgentSessionsModelEvent::Ended {
                terminal_view_id, ..
            } => {
                self.remove_notifications_for_terminal_view(*terminal_view_id, ctx);
            }
            CLIAgentSessionsModelEvent::Started { .. }
            | CLIAgentSessionsModelEvent::InputSessionChanged { .. }
            | CLIAgentSessionsModelEvent::SessionUpdated { .. } => {}
            CLIAgentSessionsModelEvent::StatusChanged {
                terminal_view_id,
                agent,
                status,
                session_context,
            } => match status {
                // When the agent resumes its work we can assume that the previous notification is stale.
                CLIAgentSessionStatus::InProgress => {
                    self.remove_notifications_for_terminal_view(*terminal_view_id, ctx);
                }
                CLIAgentSessionStatus::Success => {
                    let title = session_context
                        .display_title()
                        .unwrap_or_else(|| format!("{} completed", agent.display_name()));
                    let message = match agent {
                        CLIAgent::Codex => "Notification from Codex",
                        _ => "Task completed.",
                    };
                    let metadata = TerminalViewMetadata::lookup(*terminal_view_id, ctx);
                    self.add_notification(
                        title,
                        message.to_owned(),
                        NotificationCategory::Complete,
                        *agent,
                        *terminal_view_id,
                        metadata.branch,
                        ctx,
                    );
                }
                CLIAgentSessionStatus::Failed {
                    error_type,
                    message,
                } => {
                    let title = session_context
                        .display_title()
                        .unwrap_or_else(|| format!("{} failed", agent.display_name()));
                    let body = match (message.as_deref(), error_type.as_deref()) {
                        (Some(msg), Some(kind)) => format!("{kind}: {msg}"),
                        (Some(msg), None) => msg.to_owned(),
                        (None, Some(kind)) => kind.to_owned(),
                        (None, None) => "The agent encountered an error.".to_owned(),
                    };
                    let metadata = TerminalViewMetadata::lookup(*terminal_view_id, ctx);
                    self.add_notification(
                        title,
                        body,
                        NotificationCategory::Error,
                        *agent,
                        *terminal_view_id,
                        metadata.branch,
                        ctx,
                    );
                }
                CLIAgentSessionStatus::Blocked { message } => {
                    let title = session_context
                        .display_title()
                        .unwrap_or_else(|| format!("{} needs attention", agent.display_name()));
                    let metadata = TerminalViewMetadata::lookup(*terminal_view_id, ctx);
                    self.add_notification(
                        title,
                        message
                            .clone()
                            .unwrap_or_else(|| "Waiting for input.".to_owned()),
                        NotificationCategory::Request,
                        *agent,
                        *terminal_view_id,
                        metadata.branch,
                        ctx,
                    );
                }
                CLIAgentSessionStatus::Cancelled => {
                    let title = session_context
                        .display_title()
                        .unwrap_or_else(|| format!("{} cancelled", agent.display_name()));
                    let metadata = TerminalViewMetadata::lookup(*terminal_view_id, ctx);
                    self.add_notification(
                        title,
                        "Cancelled by user.".to_owned(),
                        NotificationCategory::Complete,
                        *agent,
                        *terminal_view_id,
                        metadata.branch,
                        ctx,
                    );
                }
            },
        }
    }

    /// Removes the existing notification for the given terminal view (if any) and emits an update
    /// event.
    fn remove_notifications_for_terminal_view(
        &mut self,
        terminal_view_id: EntityId,
        ctx: &mut ModelContext<Self>,
    ) {
        if self
            .notifications
            .remove_for_terminal_view(terminal_view_id)
        {
            ctx.emit(AgentManagementEvent::NotificationUpdated);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_notification(
        &mut self,
        title: String,
        message: String,
        category: NotificationCategory,
        agent: CLIAgent,
        terminal_view_id: EntityId,
        branch: Option<String>,
        ctx: &mut ModelContext<Self>,
    ) {
        let is_visible = is_terminal_view_visible(terminal_view_id, ctx);
        let item = NotificationItem::new(
            title,
            message,
            category,
            agent,
            is_visible,
            terminal_view_id,
            branch,
        );

        let id = item.id;
        self.notifications.push(item);
        ctx.emit(AgentManagementEvent::NotificationAdded { id });
    }
}

#[derive(Clone, Debug)]
pub enum AgentManagementEvent {
    /// A new notification was added to the persistent notification center.
    NotificationAdded { id: NotificationId },
    /// A notification's read state changed.
    NotificationUpdated,
    /// All notifications were marked as read.
    AllNotificationsMarkedRead,
}

fn is_terminal_view_visible(terminal_view_id: EntityId, app: &AppContext) -> bool {
    let Some(active_id) = active_focused_terminal_id(app) else {
        return false;
    };
    active_id == terminal_view_id
        || is_terminal_view_in_same_tab(&active_id, &terminal_view_id, app)
}

/// Per-notification metadata derived from a single [`TerminalView`] lookup.
struct TerminalViewMetadata {
    branch: Option<String>,
}

impl TerminalViewMetadata {
    fn lookup(terminal_view_id: EntityId, app: &AppContext) -> Self {
        let branch = find_terminal_view_by_id(terminal_view_id, app)
            .and_then(|terminal_view| terminal_view.as_ref(app).current_git_branch(app));
        Self { branch }
    }
}

fn find_terminal_view_by_id(
    terminal_view_id: EntityId,
    app: &AppContext,
) -> Option<ViewHandle<TerminalView>> {
    for (_, workspace_handle) in WorkspaceRegistry::as_ref(app).all_workspaces(app) {
        for pane_group in workspace_handle.as_ref(app).tab_views() {
            let pane_group = pane_group.as_ref(app);
            for pane_id in pane_group.terminal_pane_ids() {
                if let Some(terminal_view) = pane_group.terminal_view_from_pane_id(pane_id, app)
                    && terminal_view.id() == terminal_view_id
                {
                    return Some(terminal_view);
                }
            }
        }
    }
    None
}

fn active_focused_terminal_id(app: &AppContext) -> Option<EntityId> {
    let active_window = app.windows().active_window()?;
    let workspace = app
        .views_of_type::<Workspace>(active_window)
        .and_then(|views| views.first().cloned())?;

    let workspace = workspace.as_ref(app);
    workspace.active_terminal_id(app)
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
