use settings::Setting as _;
use warp_core::features::FeatureFlag;
use warp_errors::report_if_error;
use warpui::{App, EntityId, ModelHandle, SingletonEntity};

use super::AgentNotificationsModel;
use crate::agent_notifications::{NotificationCategory, NotificationFilter};
use crate::settings::CLIAgentSettings;
use crate::terminal::CLIAgent;
use crate::terminal::cli_agent_sessions::{
    CLIAgentSessionContext, CLIAgentSessionStatus, CLIAgentSessionsModel,
    CLIAgentSessionsModelEvent,
};
use crate::test_util::settings::initialize_settings_for_tests;
use crate::workspace::WorkspaceRegistry;

fn setup_app(
    app: &mut App,
) -> (
    ModelHandle<CLIAgentSessionsModel>,
    ModelHandle<AgentNotificationsModel>,
) {
    initialize_settings_for_tests(app);
    app.add_singleton_model(|_| WorkspaceRegistry::new());
    let sessions = app.add_singleton_model(|_| CLIAgentSessionsModel::new());
    let notifications = app.add_singleton_model(AgentNotificationsModel::new);
    // Hidden in-app notifications skip the toast telemetry path, which needs a telemetry context
    // provider the test app doesn't register.
    CLIAgentSettings::handle(app).update(app, |settings, ctx| {
        report_if_error!(settings.show_agent_notifications.set_value(false, ctx));
    });
    (sessions, notifications)
}

fn emit_status(
    sessions: &ModelHandle<CLIAgentSessionsModel>,
    app: &mut App,
    terminal_view_id: EntityId,
    status: CLIAgentSessionStatus,
) {
    sessions.update(app, |_, ctx| {
        ctx.emit(CLIAgentSessionsModelEvent::StatusChanged {
            terminal_view_id,
            agent: CLIAgent::Claude,
            status,
            session_context: Box::new(CLIAgentSessionContext::default()),
        });
    });
}

fn notification_count(notifications: &ModelHandle<AgentNotificationsModel>, app: &App) -> usize {
    notifications.read(app, |model, _| {
        model
            .notifications()
            .filtered_count(NotificationFilter::All)
    })
}

#[test]
fn add_notification_tracks_unread_activity_when_in_app_notifications_are_hidden() {
    App::test((), |mut app| async move {
        let _guard = FeatureFlag::HOANotifications.override_enabled(true);
        let (_sessions, notifications) = setup_app(&mut app);

        let terminal_view_id = EntityId::new();
        notifications.update(&mut app, |model, ctx| {
            model.add_notification(
                "Claude completed".to_owned(),
                "Task completed.".to_owned(),
                NotificationCategory::Complete,
                CLIAgent::Claude,
                terminal_view_id,
                None,
                ctx,
            );
        });

        notifications.read(&app, |model, _| {
            assert_eq!(
                model
                    .notifications()
                    .filtered_count(NotificationFilter::All),
                1
            );
            assert!(
                model
                    .notifications()
                    .has_unread_for_terminal_view(terminal_view_id)
            );
        });
    });
}

#[test]
fn blocked_status_adds_request_notification() {
    App::test((), |mut app| async move {
        let _guard = FeatureFlag::HOANotifications.override_enabled(true);
        let (sessions, notifications) = setup_app(&mut app);

        let terminal_view_id = EntityId::new();
        emit_status(
            &sessions,
            &mut app,
            terminal_view_id,
            CLIAgentSessionStatus::Blocked {
                message: Some("Approve edit?".to_owned()),
            },
        );

        notifications.read(&app, |model, _| {
            let item = model
                .notifications()
                .items_filtered(NotificationFilter::All)
                .next()
                .expect("blocked status should add a notification");
            assert_eq!(item.category, NotificationCategory::Request);
            assert_eq!(item.message, "Approve edit?");
            assert_eq!(item.terminal_view_id, terminal_view_id);
        });
    });
}

#[test]
fn failed_status_adds_error_notification() {
    App::test((), |mut app| async move {
        let _guard = FeatureFlag::HOANotifications.override_enabled(true);
        let (sessions, notifications) = setup_app(&mut app);

        emit_status(
            &sessions,
            &mut app,
            EntityId::new(),
            CLIAgentSessionStatus::Failed {
                error_type: Some("rate_limit".to_owned()),
                message: Some("Slow down".to_owned()),
            },
        );

        notifications.read(&app, |model, _| {
            assert_eq!(
                model
                    .notifications()
                    .filtered_count(NotificationFilter::Errors),
                1
            );
            let item = model
                .notifications()
                .items_filtered(NotificationFilter::Errors)
                .next()
                .unwrap();
            assert_eq!(item.message, "rate_limit: Slow down");
        });
    });
}

#[test]
fn in_progress_resume_clears_stale_notification_and_adds_none() {
    App::test((), |mut app| async move {
        let _guard = FeatureFlag::HOANotifications.override_enabled(true);
        let (sessions, notifications) = setup_app(&mut app);

        let terminal_view_id = EntityId::new();
        emit_status(
            &sessions,
            &mut app,
            terminal_view_id,
            CLIAgentSessionStatus::Success,
        );
        assert_eq!(notification_count(&notifications, &app), 1);

        emit_status(
            &sessions,
            &mut app,
            terminal_view_id,
            CLIAgentSessionStatus::InProgress,
        );
        assert_eq!(notification_count(&notifications, &app), 0);
    });
}

#[test]
fn session_end_removes_notification() {
    App::test((), |mut app| async move {
        let _guard = FeatureFlag::HOANotifications.override_enabled(true);
        let (sessions, notifications) = setup_app(&mut app);

        let terminal_view_id = EntityId::new();
        emit_status(
            &sessions,
            &mut app,
            terminal_view_id,
            CLIAgentSessionStatus::Cancelled,
        );
        assert_eq!(notification_count(&notifications, &app), 1);

        sessions.update(&mut app, |_, ctx| {
            ctx.emit(CLIAgentSessionsModelEvent::Ended {
                terminal_view_id,
                agent: CLIAgent::Claude,
            });
        });
        assert_eq!(notification_count(&notifications, &app), 0);
    });
}

#[test]
fn notifications_are_ignored_when_feature_is_disabled() {
    App::test((), |mut app| async move {
        let _guard = FeatureFlag::HOANotifications.override_enabled(false);
        let (sessions, notifications) = setup_app(&mut app);

        emit_status(
            &sessions,
            &mut app,
            EntityId::new(),
            CLIAgentSessionStatus::Success,
        );
        assert_eq!(notification_count(&notifications, &app), 0);
    });
}
