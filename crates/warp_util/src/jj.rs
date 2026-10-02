use std::path::{Path, PathBuf};
use std::sync::RwLock;

use anyhow::{Result, anyhow};

/// The `jj` executable resolved from the user's interactive shell `PATH`. GUI launches often
/// don't inherit that `PATH` (e.g. Homebrew or `~/.local/bin` installs), so bare `jj` may not be
/// found by the app process itself.
static JJ_EXECUTABLE: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Records the `jj` executable to use for subsequent [`run_jj_command`] calls.
pub fn set_jj_executable(path: PathBuf) {
    if let Ok(mut executable) = JJ_EXECUTABLE.write() {
        *executable = Some(path);
    }
}

/// Whether [`set_jj_executable`] has been called.
pub fn is_jj_executable_resolved() -> bool {
    JJ_EXECUTABLE
        .read()
        .is_ok_and(|executable| executable.is_some())
}

/// Controls whether jj snapshots the working copy before running a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JjSnapshot {
    /// Record working-copy edits into `@` first. Creates a jj operation only if files changed.
    Snapshot,
    /// Read the repository as of the last snapshot (`--ignore-working-copy`).
    Skip,
}

/// Runs a jj command in `repo_path` and returns its stdout.
#[cfg(not(target_family = "wasm"))]
pub async fn run_jj_command(
    repo_path: &Path,
    args: &[&str],
    snapshot: JjSnapshot,
) -> Result<String> {
    run_jj_command_with_env(repo_path, args, snapshot, None).await
}

/// Like [`run_jj_command`] but sets `PATH` on the child when `path_env` is `Some`, so git hooks
/// and credential helpers jj invokes resolve like in an interactive shell.
#[cfg(not(target_family = "wasm"))]
pub async fn run_jj_command_with_env(
    repo_path: &Path,
    args: &[&str],
    snapshot: JjSnapshot,
    path_env: Option<&str>,
) -> Result<String> {
    use command::Stdio;
    use command::r#async::Command;

    log::debug!("[JJ OPERATION] jj {}", args.join(" "));
    let mut cmd = Command::new(jj_executable());
    cmd.args(["--no-pager", "--color=never"]);
    if snapshot == JjSnapshot::Skip {
        cmd.arg("--ignore-working-copy");
    }
    if let Some(path_env) = path_env {
        cmd.env("PATH", path_env);
    }
    cmd.args(args)
        .current_dir(repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let output = cmd
        .output()
        .await
        .map_err(|e| anyhow!("Failed to execute jj command: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow!("jj command failed: {stderr}"))
    }
}

/// Blocking variant of [`run_jj_command`] for synchronous call sites.
#[cfg(not(target_family = "wasm"))]
pub fn run_jj_command_blocking(
    repo_path: &Path,
    args: &[&str],
    snapshot: JjSnapshot,
) -> Result<String> {
    use command::Stdio;
    use command::blocking::Command;

    let mut cmd = Command::new(jj_executable());
    cmd.args(["--no-pager", "--color=never"]);
    if snapshot == JjSnapshot::Skip {
        cmd.arg("--ignore-working-copy");
    }
    let output = cmd
        .args(args)
        .current_dir(repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| anyhow!("Failed to execute jj command: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow!("jj command failed: {stderr}"))
    }
}

fn jj_executable() -> PathBuf {
    JJ_EXECUTABLE
        .read()
        .ok()
        .and_then(|executable| executable.clone())
        .unwrap_or_else(|| PathBuf::from("jj"))
}
