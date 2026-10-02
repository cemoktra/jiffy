use std::path::PathBuf;

use futures::future::BoxFuture;
pub use warp_terminal::local_tty::docker_sandbox::*;
use warpui::AppContext;

use crate::terminal::local_shell::resolve_executable_from_user_shell;
/// Resolves `sbx` using the PATH captured from the user's interactive login
/// shell, matching how MCP servers and LSP find binaries.
///
/// Falls back to the process's `PATH` if the interactive PATH capture
/// fails.
#[cfg(feature = "local_tty")]
pub fn resolve_sbx_path_from_user_shell(
    ctx: &mut AppContext,
) -> BoxFuture<'static, Option<PathBuf>> {
    resolve_executable_from_user_shell("sbx", ctx)
}
