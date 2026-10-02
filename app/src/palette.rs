use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum PaletteMode {
    Command,
    Navigation,
    LaunchConfig,
    Files,
}

/// Where a palette was opened from, for sources that affect palette behavior.
#[derive(Clone, Copy, Debug, Default)]
pub enum PaletteSource {
    #[default]
    Other,
    CtrlTab {
        shift_pressed_initially: bool,
    },
    TitleBarSearchBar,
}
