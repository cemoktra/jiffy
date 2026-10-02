use warpui::EntityId;

use super::*;
use crate::terminal::CLIAgent;

fn make_cli_session_notification(
    terminal_view_id: EntityId,
    category: NotificationCategory,
) -> NotificationItem {
    NotificationItem::new(
        "cli test".to_owned(),
        "cli msg".to_owned(),
        category,
        CLIAgent::Claude,
        false,
        terminal_view_id,
        None,
    )
}

#[test]
fn remove_for_terminal_view_cleans_up_cli_session_notification() {
    let mut items = NotificationItems::default();
    let terminal_view_id = EntityId::new();

    items.push(make_cli_session_notification(
        terminal_view_id,
        NotificationCategory::Complete,
    ));
    assert_eq!(items.filtered_count(NotificationFilter::All), 1);

    let removed = items.remove_for_terminal_view(terminal_view_id);
    assert!(removed);
    assert_eq!(items.filtered_count(NotificationFilter::All), 0);
}

#[test]
fn remove_for_terminal_view_leaves_unrelated_notifications() {
    let mut items = NotificationItems::default();
    let terminal_a = EntityId::new();
    let terminal_b = EntityId::new();

    items.push(make_cli_session_notification(
        terminal_a,
        NotificationCategory::Complete,
    ));
    items.push(make_cli_session_notification(
        terminal_b,
        NotificationCategory::Request,
    ));
    assert_eq!(items.filtered_count(NotificationFilter::All), 2);

    let removed = items.remove_for_terminal_view(terminal_a);
    assert!(removed);
    assert_eq!(items.filtered_count(NotificationFilter::All), 1);

    let remaining = items
        .items_filtered(NotificationFilter::All)
        .next()
        .unwrap();
    assert_eq!(remaining.terminal_view_id, terminal_b);
}

#[test]
fn remove_for_terminal_view_returns_false_when_nothing_to_remove() {
    let mut items = NotificationItems::default();
    assert!(!items.remove_for_terminal_view(EntityId::new()));
}

#[test]
fn push_replaces_stale_notification_for_same_terminal_view() {
    let mut items = NotificationItems::default();
    let terminal_view_id = EntityId::new();

    items.push(make_cli_session_notification(
        terminal_view_id,
        NotificationCategory::Request,
    ));
    items.push(make_cli_session_notification(
        terminal_view_id,
        NotificationCategory::Error,
    ));

    assert_eq!(items.filtered_count(NotificationFilter::All), 1);
    assert_eq!(items.filtered_count(NotificationFilter::Errors), 1);
}

#[test]
fn mark_all_terminal_view_items_as_read_only_affects_that_view() {
    let mut items = NotificationItems::default();
    let terminal_a = EntityId::new();
    let terminal_b = EntityId::new();

    items.push(make_cli_session_notification(
        terminal_a,
        NotificationCategory::Complete,
    ));
    items.push(make_cli_session_notification(
        terminal_b,
        NotificationCategory::Complete,
    ));

    assert!(items.mark_all_terminal_view_items_as_read(terminal_a));
    assert!(!items.has_unread_for_terminal_view(terminal_a));
    assert!(items.has_unread_for_terminal_view(terminal_b));
    assert_eq!(items.filtered_count(NotificationFilter::Unread), 1);
}
