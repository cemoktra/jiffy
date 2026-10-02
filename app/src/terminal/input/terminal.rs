use warp_core::settings::Setting;
use warpui::elements::{
    Border, Container, DropTarget, Element, Flex, Hoverable, ParentElement, SavePosition, Stack,
};
use warpui::presenter::ChildView;
use warpui::{AppContext, SingletonEntity};

use super::common::{
    add_command_xray_overlay, add_input_suggestions_overlays,
    wrap_input_with_terminal_padding_and_focus_handler,
};
use super::{Input, InputDropTargetData};
use crate::appearance::Appearance;
use crate::context_chips::spacing;
use crate::features::FeatureFlag;
use crate::settings::InputModeSettings;
use crate::terminal::block_list_settings::BlockListSettings;
use crate::terminal::block_list_viewport::InputMode;
use crate::terminal::settings::TerminalSettings;
use crate::terminal::view::TerminalAction;

impl Input {
    /// Renders the terminal mode input used when the prompt isn't rendered from the user's PS1.
    pub(super) fn render_terminal_input(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let menu_positioning = self.menu_positioning(app);

        let model = self.model.lock();

        // We should likely rework this stack to not need to use `with_constrain_absolute_children`,
        // by reworking the positioning of the children to not depend on this.
        let mut stack = Stack::new().with_constrain_absolute_children();

        let input_mode = *InputModeSettings::as_ref(app).input_mode.value();

        let mut column = Flex::column();

        let prompt_elements = self
            .prompt_render_helper
            .render_universal_developer_input_prompt(&model, appearance, app);

        column.add_child(prompt_elements);

        let terminal_spacing = TerminalSettings::as_ref(app)
            .terminal_input_spacing(appearance.line_height_ratio(), app);
        column.add_child(
            Container::new(self.render_input_box(appearance, app))
                .with_margin_top(
                    terminal_spacing.prompt_to_editor_padding
                        * spacing::UDI_PROMPT_BOTTOM_PADDING_FACTOR,
                )
                .finish(),
        );

        if !(matches!(input_mode, InputMode::PinnedToTop)
            && self
                .suggestions_mode_model
                .as_ref(app)
                .is_inline_menu_open())
        {
            column.add_child(
                Container::new(Flex::row().finish())
                    .with_margin_bottom(8.)
                    .finish(),
            );
        }

        stack.add_child(wrap_input_with_terminal_padding_and_focus_handler(
            self.focus_handle
                .as_ref()
                .is_some_and(|h| h.is_active_session(app)),
            column.finish(),
        ));

        let is_focused = self.focus_handle.as_ref().is_none_or(|h| h.is_focused(app));

        if is_focused {
            add_input_suggestions_overlays(self, &mut stack, appearance, menu_positioning, app);
        }

        if let Some(token_description) = &self.command_x_ray_description {
            add_command_xray_overlay(
                self,
                &mut stack,
                token_description,
                appearance,
                menu_positioning,
                app,
            );
        }

        let drop_target = DropTarget::new(
            SavePosition::new(stack.finish(), &self.status_free_input_save_position_id()).finish(),
            InputDropTargetData::new(self.weak_view_handle.clone()),
        )
        .finish();

        let hoverable_input = Hoverable::new(self.hoverable_handle.clone(), |_| drop_target)
            .on_middle_click(|ctx, _app, _position| {
                ctx.dispatch_typed_action(TerminalAction::MiddleClickOnInput)
            })
            .finish();

        let show_block_dividers = *BlockListSettings::as_ref(app).show_block_dividers.value();

        let input = if show_block_dividers {
            Container::new(hoverable_input)
                .with_border(
                    Border::top(1.)
                        .with_border_color(styles::default_border_color(appearance.theme())),
                )
                .finish()
        } else {
            hoverable_input
        };

        let hide_menu = self
            .inline_terminal_menu_positioner
            .as_ref(app)
            .should_hide_inline_menu_for_pane_size(app);
        self.layout_input_with_inline_menu(input, input_mode, hide_menu, app)
    }

    /// Returns the inline menu view (slash commands, skills, history) that is currently open.
    fn render_open_inline_menu(&self, app: &AppContext) -> Option<Box<dyn Element>> {
        let suggestions_mode_model = self.suggestions_mode_model.as_ref(app);
        if suggestions_mode_model.is_slash_commands() {
            Some(ChildView::new(&self.inline_slash_commands_view).finish())
        } else if FeatureFlag::ListSkills.is_enabled() && suggestions_mode_model.is_skill_menu() {
            Some(ChildView::new(&self.inline_skill_selector_view).finish())
        } else if FeatureFlag::InlineHistoryMenu.is_enabled()
            && suggestions_mode_model.is_inline_history_menu()
        {
            Some(ChildView::new(&self.inline_history_menu_view).finish())
        } else {
            None
        }
    }

    /// Stacks `input` with the open inline menu, placing the menu on the side of the input that
    /// faces the block list.
    pub(super) fn layout_input_with_inline_menu(
        &self,
        input: Box<dyn Element>,
        input_mode: InputMode,
        hide_menu: bool,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let menu = if hide_menu {
            None
        } else {
            self.render_open_inline_menu(app)
        };
        let render_menu_below = match input_mode {
            InputMode::PinnedToBottom => false,
            InputMode::PinnedToTop => true,
            InputMode::Waterfall => self
                .inline_terminal_menu_positioner
                .as_ref(app)
                .should_render_inline_menu_below_input(),
        };

        let mut column = Flex::column();
        match menu {
            Some(menu) if render_menu_below => {
                column.add_children([input, menu]);
            }
            Some(menu) => {
                column.add_children([menu, input]);
            }
            None => column.add_child(input),
        }

        SavePosition::new(column.finish(), &self.save_position_id()).finish()
    }
}

pub mod styles {
    use pathfinder_color::ColorU;
    use warp_core::ui::theme::WarpTheme;

    pub fn default_border_color(theme: &WarpTheme) -> ColorU {
        theme.outline().into()
    }
}
