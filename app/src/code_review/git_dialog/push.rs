//! Push / publish mode for [`GitDialog`].
//!
//! Renders the branch's unpushed commit list with per-commit file expansion.
//! Each commit's file list is captured up front on `Commit.files`, so
//! expansion is a pure toggle (no per-commit fetch). A single `publish: bool`
//! flag toggles between pushing an existing branch and publishing a new one
//! (setting upstream). On confirm, spawns `run_push`.

use std::collections::HashMap;

use warp_core::ui::appearance::Appearance;
use warp_errors::report_error;
use warpui::elements::{
    Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, Element, Flex, Hoverable, MainAxisAlignment, MainAxisSize,
    MouseStateHandle, ParentElement, Radius, ScrollbarWidth, Text,
};
use warpui::platform::Cursor;
use warpui::ui_components::components::{UiComponent, UiComponentStyles};
use warpui::{AppContext, SingletonEntity, ViewContext, ViewHandle};

use crate::code::editor::{add_color, remove_color};
use crate::code_review::git_dialog::{
    GitDialog, GitDialogAction, GitDialogEvent, GitDialogMode, render_branch_section,
    render_chevron_icon, render_file_list, show_toast, user_facing_git_error,
};
use crate::editor::{EditorOptions, EditorView, Event as EditorEvent, TextOptions};
use crate::ui_components::icons::Icon;
use crate::util::git::Commit;

/// Push-specific sub-actions, dispatched wrapped in `GitDialogAction::Push`.
#[derive(Clone, Debug, PartialEq)]
pub enum PushSubAction {
    ToggleCommit(String),
}

pub struct PushState {
    pub(super) publish: bool,
    commits: Vec<Commit>,
    expanded: HashMap<String, bool>,
    commit_mouse_states: HashMap<String, MouseStateHandle>,
    commits_scroll_state: ClippedScrollStateHandle,
    /// Name for the jj bookmark to create and push, shown when the committed change has none.
    bookmark_editor: Option<ViewHandle<EditorView>>,
}

pub(super) fn new_state(publish: bool, commits: Vec<Commit>) -> PushState {
    let commit_mouse_states = commits
        .iter()
        .map(|c| (c.hash.clone(), MouseStateHandle::default()))
        .collect();
    PushState {
        publish,
        commits,
        expanded: HashMap::new(),
        commit_mouse_states,
        commits_scroll_state: ClippedScrollStateHandle::default(),
        bookmark_editor: None,
    }
}

/// In a jj workspace whose committed change has no bookmark yet, asks for the bookmark name to
/// push, prefilled with jj's own `push-<change id>` convention.
#[cfg(feature = "local_fs")]
pub(super) fn maybe_request_bookmark_name(me: &GitDialog, ctx: &mut ViewContext<GitDialog>) {
    use warp_util::jj::JjSnapshot;

    let Some(repo_path) = me
        .repo_location()
        .to_local_path()
        .filter(|path| crate::util::jj::is_jj_workspace(path))
        .map(std::path::Path::to_path_buf)
    else {
        return;
    };
    ctx.spawn(
        async move { crate::util::jj::working_copy_info(&repo_path, JjSnapshot::Skip).await },
        |me, result, ctx| {
            let Ok(info) = result else {
                return;
            };
            if info.bookmark.is_some() {
                return;
            }
            let editor = ctx.add_typed_action_view(|ctx| {
                let mut editor = EditorView::new(
                    EditorOptions {
                        text: TextOptions {
                            font_family_override: Some(Appearance::as_ref(ctx).ui_font_family()),
                            ..Default::default()
                        },
                        single_line: true,
                        ..Default::default()
                    },
                    ctx,
                );
                editor.set_placeholder_text("Bookmark name", ctx);
                editor.system_reset_buffer_text(&format!("push-{}", info.change_id), ctx);
                editor
            });
            ctx.subscribe_to_view(&editor, |me, _, event, ctx| match event {
                EditorEvent::Escape => {
                    if !me.loading() {
                        ctx.emit(GitDialogEvent::Cancelled);
                    }
                }
                EditorEvent::Edited(_) => {
                    me.refresh_confirm_enabled(ctx);
                    ctx.notify();
                }
                _ => {}
            });
            if let GitDialogMode::Push(state) = me.mode_mut() {
                state.bookmark_editor = Some(editor);
            }
            me.refresh_confirm_enabled(ctx);
            ctx.notify();
        },
    );
}

/// The bookmark name typed into the push dialog, if it asks for one.
fn requested_bookmark_name(state: &PushState, app: &AppContext) -> Option<String> {
    let editor = state.bookmark_editor.as_ref()?;
    Some(editor.as_ref(app).buffer_text(app).trim().to_string())
}

pub(super) fn is_ready_to_confirm(state: &PushState, app: &AppContext) -> bool {
    requested_bookmark_name(state, app).is_none_or(|name| !name.is_empty())
}

pub(super) fn confirm_label(publish: bool) -> &'static str {
    if publish { "Publish" } else { "Push" }
}

pub(super) fn confirm_icon(publish: bool) -> Icon {
    if publish {
        Icon::UploadCloud
    } else {
        Icon::ArrowUp
    }
}

fn loading_label(publish: bool) -> &'static str {
    if publish {
        "Publishing…"
    } else {
        "Pushing…"
    }
}

pub(super) fn handle_sub_action(
    me: &mut GitDialog,
    action: &PushSubAction,
    ctx: &mut ViewContext<GitDialog>,
) {
    match action {
        PushSubAction::ToggleCommit(hash) => {
            // File lists are captured up front on `Commit.files`, so expansion
            // is a pure toggle.
            if let GitDialogMode::Push(state) = me.mode_mut() {
                let is_expanded = state.expanded.entry(hash.clone()).or_insert(false);
                *is_expanded = !*is_expanded;
            }
            ctx.notify();
        }
    }
}

pub(super) fn start_confirm(me: &mut GitDialog, ctx: &mut ViewContext<GitDialog>) {
    let (publish, requested_bookmark) = match me.mode() {
        GitDialogMode::Push(state) => (state.publish, requested_bookmark_name(state, ctx)),
        _ => return,
    };
    let branch = match requested_bookmark {
        Some(name) if name.is_empty() => return,
        Some(name) => name,
        None => me.branch_name().to_string(),
    };

    me.set_loading(loading_label(publish), ctx);

    me.diff_state_model().update(ctx, |m, ctx| {
        m.git_push(branch, ctx);
    });
}

/// Shared push completion: toast + close.
pub(super) fn finish_push(
    publish: bool,
    result: anyhow::Result<()>,
    ctx: &mut ViewContext<GitDialog>,
) {
    match result {
        Ok(_) => {
            let toast_msg = if publish {
                "Branch successfully published."
            } else {
                "Changes successfully pushed."
            };
            show_toast(toast_msg, ctx);
        }
        Err(e) => {
            report_error!(&e);
            show_toast(user_facing_git_error(&e.to_string()), ctx);
        }
    }
    ctx.emit(GitDialogEvent::Completed);
}

pub(super) fn render_body(
    state: &PushState,
    branch_name: &str,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let branch_section = match &state.bookmark_editor {
        Some(editor) => render_bookmark_editor(editor, appearance),
        None => render_branch_section(branch_name, appearance),
    };
    let mut body = Flex::column().with_child(
        Container::new(branch_section)
            .with_margin_bottom(16.)
            .finish(),
    );

    if !state.commits.is_empty() {
        body.add_child(render_commits_section(state, appearance));
    }

    body.finish()
}

fn render_bookmark_editor(
    editor: &ViewHandle<EditorView>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let label = Text::new(
        "Bookmark",
        appearance.ui_font_family(),
        appearance.ui_font_size(),
    )
    .with_color(theme.main_text_color(theme.surface_1()).into_solid())
    .finish();
    let input = appearance
        .ui_builder()
        .text_input(editor.clone())
        .with_style(UiComponentStyles {
            border_color: Some(theme.surface_3().into()),
            border_radius: Some(CornerRadius::with_all(Radius::Pixels(6.))),
            ..Default::default()
        })
        .build()
        .finish();
    Flex::column()
        .with_child(Container::new(label).with_margin_bottom(8.).finish())
        .with_child(input)
        .finish()
}

fn render_commits_section(state: &PushState, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let main_color = theme.main_text_color(theme.surface_1()).into_solid();
    let sub_color = theme.sub_text_color(theme.surface_1()).into_solid();

    let label = Text::new(
        "Included commits",
        appearance.ui_font_family(),
        appearance.ui_font_size(),
    )
    .with_color(main_color)
    .finish();

    let mut commit_list = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

    for commit in &state.commits {
        let is_expanded = state.expanded.get(&commit.hash).copied().unwrap_or(false);
        let hash = commit.hash.clone();

        let subject = Text::new(
            commit.subject.clone(),
            appearance.ui_font_family(),
            appearance.ui_font_size(),
        )
        .with_color(main_color)
        .soft_wrap(false)
        .finish();

        let stats_text = format!(
            "{} {}",
            commit.files_changed,
            if commit.files_changed == 1 {
                "file"
            } else {
                "files"
            },
        );

        let mut stats_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Text::new(
                    stats_text,
                    appearance.ui_font_family(),
                    appearance.ui_font_size(),
                )
                .with_color(sub_color)
                .finish(),
            );

        if commit.additions > 0 {
            stats_row.add_child(
                Container::new(
                    Text::new(
                        format!("+{}", commit.additions),
                        appearance.ui_font_family(),
                        appearance.ui_font_size(),
                    )
                    .with_color(add_color(appearance))
                    .finish(),
                )
                .with_margin_left(4.)
                .finish(),
            );
        }

        if commit.deletions > 0 {
            stats_row.add_child(
                Container::new(
                    Text::new(
                        format!("-{}", commit.deletions),
                        appearance.ui_font_family(),
                        appearance.ui_font_size(),
                    )
                    .with_color(remove_color(appearance))
                    .finish(),
                )
                .with_margin_left(4.)
                .finish(),
            );
        }

        let info_col = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(subject)
            .with_child(
                Container::new(stats_row.finish())
                    .with_margin_top(2.)
                    .finish(),
            )
            .finish();

        let chevron = render_chevron_icon(is_expanded, appearance);

        let summary_row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(info_col)
            .with_child(chevron)
            .finish();

        let mouse_state = state
            .commit_mouse_states
            .get(&commit.hash)
            .cloned()
            .unwrap_or_default();

        let clickable_summary = Hoverable::new(mouse_state, |_| {
            Container::new(summary_row)
                .with_padding_top(6.)
                .with_padding_bottom(6.)
                .with_padding_left(12.)
                .with_padding_right(8.)
                .finish()
        })
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(GitDialogAction::Push(PushSubAction::ToggleCommit(
                hash.clone(),
            )));
        })
        .with_cursor(Cursor::PointingHand)
        .finish();

        let mut commit_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        commit_col.add_child(clickable_summary);

        if is_expanded {
            commit_col.add_child(render_file_list(&commit.files, appearance));
        }

        let bordered_commit = Container::new(commit_col.finish())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
            .with_border(Border::all(1.).with_border_fill(theme.surface_3()))
            .finish();

        commit_list.add_child(
            Container::new(bordered_commit)
                .with_margin_bottom(4.)
                .finish(),
        );
    }

    const MAX_COMMITS_HEIGHT: f32 = 300.;

    let commit_content = commit_list.finish();
    let commits_element = ConstrainedBox::new(
        ClippedScrollable::vertical(
            state.commits_scroll_state.clone(),
            commit_content,
            ScrollbarWidth::Auto,
            theme.nonactive_ui_detail().into(),
            theme.active_ui_detail().into(),
            warpui::elements::Fill::None,
        )
        .finish(),
    )
    .with_max_height(MAX_COMMITS_HEIGHT)
    .finish();

    Flex::column()
        .with_child(Container::new(label).with_margin_bottom(8.).finish())
        .with_child(commits_element)
        .finish()
}
