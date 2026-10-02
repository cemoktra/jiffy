use warp_core::ui::theme::Fill;
use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, Empty, Flex, MainAxisAlignment,
    MainAxisSize, MouseStateHandle, ParentElement, Radius, Text,
};
use warpui::ui_components::components::UiComponent;
use warpui::{AppContext, Element, EventContext, SingletonEntity};

use crate::appearance::Appearance;
use crate::ui_components::blended_colors;
use crate::ui_components::buttons::{icon_button, icon_button_with_color};
use crate::ui_components::icons::Icon;

const CODE_BLOCK_CORNER_RADIUS: f32 = 8.0;
const CODE_BLOCK_VERTICAL_PADDING: f32 = 10.0;
const CODE_BLOCK_HORIZONTAL_PADDING: f32 = 16.0;

#[derive(Default, Clone)]
pub struct CodeSnippetButtonHandles {
    pub copy_button: MouseStateHandle,
    pub execute_button: MouseStateHandle,
}

pub type HandleCode = Box<dyn FnMut(String, &mut EventContext)>;

pub struct CodeBlockOptions {
    pub on_execute: Option<HandleCode>,
    pub on_copy: Option<HandleCode>,
    pub footer_element: Option<Box<dyn Element>>,
    pub mouse_handles: Option<CodeSnippetButtonHandles>,
}

fn render_button(
    appearance: &Appearance,
    icon: Icon,
    tooltip_text: &str,
    mouse_handle: MouseStateHandle,
    code: String,
    mut on_click: HandleCode,
    color: Option<Fill>,
) -> Container {
    let ui_builder = appearance.ui_builder().clone();
    let tooltip_text = tooltip_text.to_owned();
    let button_element = if let Some(color) = color {
        icon_button_with_color(appearance, icon, false, mouse_handle, color)
    } else {
        icon_button(appearance, icon, false, mouse_handle)
    };

    Container::new(
        button_element
            .with_tooltip(move || ui_builder.tool_tip(tooltip_text.clone()).build().finish())
            .build()
            .on_click(move |ctx, _, _| {
                on_click(code.clone(), ctx);
            })
            .finish(),
    )
}

/// Renders `code` as plain monospace text in a bordered block, with an optional footer and
/// copy / run-in-terminal buttons.
pub fn render_code_block_plain(
    code: &str,
    CodeBlockOptions {
        on_execute,
        on_copy,
        footer_element,
        mouse_handles,
    }: CodeBlockOptions,
    selectable: bool,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();

    let code_element = Text::new(
        code.to_owned(),
        appearance.monospace_font_family(),
        appearance.monospace_font_size(),
    )
    .with_color(blended_colors::text_main(theme, theme.surface_1()))
    .with_selectable(selectable)
    .finish();

    let mut footer_row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
        .with_main_axis_size(MainAxisSize::Max)
        .with_child(footer_element.unwrap_or_else(|| Empty::new().finish()));

    if let Some(mouse_handles) = mouse_handles {
        let mut action_row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);

        if let Some(on_copy) = on_copy {
            action_row.add_child(
                render_button(
                    appearance,
                    Icon::Copy,
                    "Copy",
                    mouse_handles.copy_button,
                    code.to_owned(),
                    on_copy,
                    None,
                )
                .finish(),
            );
        }

        if let Some(on_execute) = on_execute {
            action_row.add_child(
                render_button(
                    appearance,
                    Icon::TerminalInput,
                    "Run in terminal",
                    mouse_handles.execute_button,
                    code.to_owned(),
                    on_execute,
                    None,
                )
                .with_margin_left(8.)
                .finish(),
            );
        }

        footer_row.add_child(action_row.finish());
    }

    let content = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(
            Container::new(code_element)
                .with_background(theme.surface_2())
                .with_padding_top(CODE_BLOCK_VERTICAL_PADDING)
                .with_padding_bottom(CODE_BLOCK_VERTICAL_PADDING)
                .with_horizontal_padding(CODE_BLOCK_HORIZONTAL_PADDING)
                .with_corner_radius(CornerRadius::with_top(Radius::Pixels(
                    CODE_BLOCK_CORNER_RADIUS,
                )))
                .finish(),
        )
        .with_child(
            Container::new(footer_row.finish())
                .with_background(theme.surface_2())
                .with_padding_top(6.)
                .with_padding_bottom(12.)
                .with_horizontal_padding(CODE_BLOCK_HORIZONTAL_PADDING)
                .with_corner_radius(CornerRadius::with_bottom(Radius::Pixels(
                    CODE_BLOCK_CORNER_RADIUS,
                )))
                .finish(),
        );

    Container::new(content.finish())
        .with_border(Border::all(1.).with_border_fill(theme.outline()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
            CODE_BLOCK_CORNER_RADIUS,
        )))
        .finish()
}

/// Renders a runnable shell snippet with an optional language label in the footer.
pub fn render_runnable_code_snippet(
    code_snippet: &str,
    language_label: Option<&str>,
    on_execute: Option<HandleCode>,
    on_copy: Option<HandleCode>,
    mouse_handles: Option<CodeSnippetButtonHandles>,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let footer_element = language_label.map(|label| {
        Text::new_inline(
            label.to_owned(),
            appearance.monospace_font_family(),
            appearance.monospace_font_size(),
        )
        .with_color(blended_colors::text_sub(theme, theme.surface_3()))
        .finish()
    });
    render_code_block_plain(
        code_snippet,
        CodeBlockOptions {
            on_execute,
            on_copy,
            footer_element,
            mouse_handles,
        },
        true,
        app,
    )
}
