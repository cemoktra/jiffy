use pathfinder_color::ColorU;
use warp_core::ui::appearance::Appearance;
use warpui::keymap::Keystroke;
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::{AppContext, Element, SingletonEntity};

use crate::ui_components::blended_colors;

fn font_size(appearance: &Appearance) -> f32 {
    appearance.monospace_font_size() - 2.
}

fn keystroke_size(appearance: &Appearance) -> f32 {
    font_size(appearance) + 2.
}

/// Renders `keystroke` as a compact keyboard-shortcut badge, optionally overriding the default
/// text and background colors.
pub fn render_keystroke_with_color_overrides(
    keystroke: &Keystroke,
    color: Option<ColorU>,
    background_color: Option<ColorU>,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    appearance
        .ui_builder()
        .keyboard_shortcut(keystroke)
        .lowercase_modifier()
        .with_space_between_keys(2.)
        .with_style(UiComponentStyles {
            margin: Some(Coords::default()),
            padding: Some(Coords::default()),
            border_width: Some(1.),
            background: Some(
                background_color
                    .unwrap_or_else(|| blended_colors::neutral_3(theme))
                    .into(),
            ),
            font_color: Some(color.unwrap_or_else(|| theme.foreground().into_solid())),
            font_family_id: Some(appearance.ui_font_family()),
            font_size: Some(font_size(appearance)),
            width: Some(keystroke_size(appearance)),
            height: Some(keystroke_size(appearance)),
            ..Default::default()
        })
        .with_line_height_ratio(1.0)
        .build()
        .finish()
}
