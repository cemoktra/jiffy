//! ContextFlag flags are for behaviors that need to be conditionally enabled or disabled based
//! on where the app is being run and are a permanent part of the app.

use std::sync::atomic::{AtomicBool, Ordering};

use enum_iterator::{Sequence, cardinality};
use warp_errors::report_error;

use crate::channel::ChannelState;

/// All ContextFlag flags are enabled by default and can only be toggled manually in dogfood
/// contexts.
#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug, Sequence)]
pub enum ContextFlag {
    CreateNewSession,
    CloseWindow,
    LaunchConfigurations,
    ShowSlowShellStartupBanner,
}

/// The enablement states for context flags.  As mentioned in the documentation
/// for [`ContextFlag`], these are enabled by default.
static FLAG_STATES: [AtomicBool; cardinality::<ContextFlag>()] =
    [const { AtomicBool::new(true) }; { cardinality::<ContextFlag>() }];

impl ContextFlag {
    pub fn is_enabled(&self) -> bool {
        FLAG_STATES[*self as usize].load(Ordering::Relaxed)
    }

    /// Sets a ContextFlag flag. FOR DEBUG USE ONLY.
    pub fn set(&self, value: bool) {
        if !ChannelState::enable_debug_features() {
            report_error!(
                "Tried to set value of ContextFlag in non-dogfood context",
                extra: { "flag" => ?self }
            );
        }

        FLAG_STATES[*self as usize].store(value, Ordering::Relaxed);
    }
}
