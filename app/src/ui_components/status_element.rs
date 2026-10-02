use warp_core::ui::appearance::Appearance;
use warp_core::ui::color::coloru_with_opacity;
use warp_core::ui::theme::Fill;
use warpui::Element;
use warpui::elements::{ConstrainedBox, Container, CornerRadius, Radius};

use crate::terminal::cli_agent_sessions::CLIAgentSessionStatus;

/// Padding around the status icon
pub const STATUS_ELEMENT_PADDING: f32 = 2.;

/// Render the status element used by CLI agent session surfaces.
pub fn render_status_element(
    status: &CLIAgentSessionStatus,
    icon_size: f32,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let (icon, color) = status.status_icon_and_color(theme);

    Container::new(
        ConstrainedBox::new(icon.to_warpui_icon(Fill::from(color)).finish())
            .with_width(icon_size)
            .with_height(icon_size)
            .finish(),
    )
    .with_uniform_padding(STATUS_ELEMENT_PADDING)
    .with_background(coloru_with_opacity(color, 10))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
    .finish()
}
