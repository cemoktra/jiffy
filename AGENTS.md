# AGENTS.md

This file provides guidance when working with code in this repository.

Jiffy is a local-only terminal forked from the Warp client: no account, cloud sync, Warp AI agent,
session sharing, autoupdate, crash reporting, or telemetry. It adds Jujutsu (jj) support and
integration with third-party CLI agents such as Claude Code. Jiffy targets Linux; macOS and Windows
code paths are kept but not actively maintained, and wasm is not a supported target.

## Development Commands

### Build and Run
- `./script/run` - Build and run the app locally (the `jiffy` binary with the `gui` feature)
- `cargo run -p jiffy --bin jiffy --features gui` - Same, without the wrapper script
- `./script/bundle` - Build a release bundle (AppImage/deb/rpm/arch packages on Linux)

On Linux, `.cargo/config.toml` links with `clang` and `mold`; both must be on `PATH`
(`./script/bootstrap` installs them on Debian-based distributions).

Logs are written to `~/.local/state/jiffy/jiffy.log`.

### Testing
- `cargo nextest run --no-fail-fast --workspace` - Run tests with nextest
- `cargo nextest run -p <package>` - Run tests for an individual package (the app crate is `jiffy`)
- `cargo test --doc` - Run doc tests

### Linting and Formatting
- `./script/presubmit` - Run the full local presubmit only when explicitly requested
- `./script/format` - Format code
- `cargo clippy -p <package> --all-targets --tests -- -D warnings` - Run targeted Clippy
- `./script/run-clang-format.py -r --extensions 'c,h,cpp,m' ./crates/warpui/src/ ./app/src/` - Check C/C++/Obj-C formatting
- `find . -name "*.wgsl" -exec wgslfmt --check {} +` - Check WGSL shader formatting

### Implementation Validation Order
Optimize for fast delivery and let CI catch uncommon failures outside targeted local coverage.

1. While editing, run only the smallest targeted `cargo check` or `cargo nextest` command that gives useful feedback; defer checks and tests until after implementation where possible.
2. Once the code and self-review are complete, run the relevant tests and fix the code until they pass.
3. Run the relevant Clippy and other lint, typecheck, or build checks and fix their findings. Return to affected tests only when a fix materially changes behavior.
4. Run every applicable mutating formatter once, after all other code changes are complete.
5. Open or update the PR without rerunning tests or lint after formatting and without adding a full `./script/presubmit` run.

Run the full presubmit only when the user, task, or approved spec explicitly requires it. A later source, test, manifest, generated-code, or configuration change creates a new candidate: rerun the affected portion of the sequence and finish with the applicable formatter.

### Platform Setup
- `./script/bootstrap` - Platform-specific setup (system packages, Rust toolchain, cargo tools)
- `./script/install_cargo_build_deps` - Install Cargo build dependencies
- `./script/install_cargo_test_deps` - Install Cargo test dependencies

## Architecture Overview

Jiffy is a Rust terminal emulator built on a custom UI framework called **WarpUI**. The only binary
is `jiffy` (`app/src/bin/jiffy.rs`), a thin wrapper around `jiffy::run()` in the `app/` crate
(package and library name `jiffy`).

### Key Components

**UI framework** (`crates/warpui`, `crates/warpui_core`):
- Entity-Component-Handle pattern: a global `App` object owns all views/models (entities); views hold `ViewHandle<T>` references to other views; `AppContext` provides temporary access to handles during render/events.
- Actions system for event handling.
- `Element`s describe visual layout (Flutter-inspired) and are rendered on the GPU (WGSL).
- Mouse input uses `MouseStateHandle`: create it once during construction and reference/clone it wherever mouse input is tracked. An inline `MouseStateHandle::default()` while rendering means no mouse interactions work.

**Main app** (`app/`):
- Terminal emulation and shell management (`terminal/`)
- Third-party CLI agent integration, e.g. Claude Code (`terminal/cli_agent_sessions/`, `agent_notifications/`)
- Code editor, file tree, and code review panel with git and jj support (`code/`, `code_review/`)
- Settings and preferences (`settings/`, `settings_view/`)
- Workspace, tabs, and session management (`workspace/`, `pane_group/`)

**Core Libraries**:
- `crates/warp_core/` - Core utilities and platform abstractions
- `crates/warp_terminal/` - Terminal model and grid
- `crates/editor/` - Text editing functionality
- `crates/repo_metadata/` - Repository detection and metadata (git and jj)
- `crates/warp_features/` - Feature flag definitions
- `crates/persistence/` - SQLite persistence
- `crates/ipc/` - Inter-process communication

### Key Architectural Patterns

1. **Entity-Handle System**: Views reference other views via handles, not direct ownership
2. **Modular Structure**: A workspace contains tabs, each with a pane group of terminals, editors, etc.
3. **Cross-Platform**: Linux is the target; macOS and Windows code is conditionally compiled and kept

### Development Guidelines

**Workspace Structure**:
- This is a Cargo workspace; the main crate is `app/` (package `jiffy`) and the UI framework is in `crates/warpui/`
- Most other crates keep their upstream `warp_*`/`warpui*` names
- Platform-specific code is conditionally compiled

**Coding Style Preferences**:
- Avoid unnecessary type annotations, especially in closure params.
- Avoid using too many Rust path qualifiers and use imports for concision. Place import statements at the top of the file as per convention.
  An exception to this is inside cfg-guarded code branches. In those cases, you can either embed the import into the relevant scope or just use an absolute path for one-offs.
- If a function takes a context parameter (`AppContext`, `ViewContext`, or `ModelContext`), it should be named `ctx` and go last. The one exception is for
  functions that take a closure parameter, in which case the closure should be last.
- Always remove unused parameters completely rather than prefixing them with `_`. Update the function signature and all call sites accordingly.
- Prefer inline format arguments in macros like `println!`, `eprintln!`, and `format!` (for example, `eprintln!("{message}")` instead of `eprintln!("{}", message)`) to satisfy Clippy's `uninlined_format_args` lint.
- Do not pass `Itertools::format` results directly to logging macros (`log::*`, `safe_*`, etc.). `Itertools::format` produces a single-use formatter, while logging implementations may format a message more than once. Use a reusable `String` such as `iter.join(", ")` for logging arguments instead. Direct use in `format!` or `write!` is fine.
- When adding a toggleable setting, also add the matching Command Palette enable/disable entry and any required context flags so the setting is discoverable outside Settings.

**Comments**:
Comments have a cost. They carry a maintenance burden, because they must be kept in sync
with the code they describe. It is tempting to assume that more comments is always better,
but be judicious about when a comment is actually necessary because the code cannot speak
for itself.
- **Minimalist Comments**: Assume the reader is a Senior Software Engineer. Never comment
  to explain WHAT or HOW code works if self-documenting names accomplish that.
- **Strictly "Why" Only**: Reserve inline comments strictly for non-obvious business
  rationale, workarounds for third-party bugs, complex algorithms, unidiomatic code, or
  unexpected edge cases.
- **No Line-by-Line Narrations**: Never add comments restating the syntax (e.g., omit
  `# Initialize array`, `# Loop over users`).
- **Clean Docstrings**: Keep doc comments concise. Document public APIs, arguments, types,
  and returns. Do not narrate the method's internal implementation steps.
- **Single-source of documentation**: For items/members that have a doc comment explaining
  their purpose, you do not need to repeat that explanation anywhere else. A good example
  is a float const specifying an amount of spacing. You may use a doc comment on the
  declaration if necessary, but do not repeat that where the const is *referenced*. Another
  example is function call sites. Function doc comments explain what they do. Do not repeat
  the explanation at the call site.
- **Container docs describe the whole, member docs describe the parts**: A field's, variant's,
  or parameter's own doc comment is where that member gets explained. A container's item-level
  doc comment (struct, enum, trait) describes the item as a whole and must not enumerate or
  re-explain its members. Do not name members in the container's doc comment just to describe
  them, and do not restate in a member's doc comment what the container's doc comment already
  said. Behavior genuinely shared by several members belongs in exactly one place, not both.
- **Don't enumerate function call sites in doc comments**: Function doc comments should
  document their behavior and NOT their callers, e.g. it should never say things like,
  "this is used by [certain callers]" or "this is used when...".
- **No "transformation comments"**: Do not add comments that explain *your edits*. Comments
  only need explain the *current state* of the code. Explanations of edits belong in pull
  request comments instead. You shouldn't add comments with phrases like, "this used to do
  so-and-so".
- Do not remove existing comments when making unrelated changes. Only remove or modify a
  comment if the logic it describes has changed.
- The formatter (`./script/format`) is configured with a `max_width` (max line length) of
  100. Flow (reflow) comment line-wrapping to fill that full width rather than wrapping
  early at a narrower column, so comments span as few lines as possible.

**Terminal Model Locking**:
- Be extremely careful when calling `model.lock()` on the terminal model (`TerminalModel`). Acquiring multiple locks on the same model from different call sites can cause a deadlock, resulting in a UI freeze.
- Before adding a new `model.lock()` call, verify that no caller in the current call stack already holds the lock.
- Prefer passing already-locked model references down the call stack rather than acquiring new locks.
- If you must lock the model, keep the lock scope as short as possible and avoid calling other functions that might also attempt to lock.

**Testing**:
- Use `cargo nextest` for parallel test execution
- Follow the Implementation Validation Order above; do not add a full presubmit run after targeted tests unless it was explicitly required.
- Unit tests should be placed in separate files using the naming convention `${filename}_tests.rs` or `mod_test.rs`
- Test files should be included at the end of their corresponding module with:
  ```rust
  #[cfg(test)]
  #[path = "filename_tests.rs"]  // or "mod_test.rs"
  mod tests;
  ```

**Pull Request Workflow**:
- Follow the Implementation Validation Order before opening a PR or pushing a code update. Do not repeat validation when the candidate has not changed.
- CI (`.github/workflows/ci.yml`) runs Linux tests, formatting, and Clippy; address a later CI failure as a new revision.
- When opening PRs, use the PR template at `.github/pull_request_template.md`.

**Database**:
- Uses Diesel ORM with SQLite
- Migrations in `crates/persistence/migrations/`
- Schema defined in `crates/persistence/src/schema.rs`

### Feature Flags

Jiffy uses compile-time feature flags with a small runtime plumbing layer.

How to add a feature flag:
- Add a new variant to the `FeatureFlag` enum in `crates/warp_features/src/lib.rs`
- Enable it by default by adding a matching Cargo feature to `app/Cargo.toml` (and its `default`
  list) and mapping it in `app/src/features.rs`
- Gate code paths with `FeatureFlag::YourFlag.is_enabled()`

Best practices:
- **Prefer runtime checks over cfg directives**: Prefer `FeatureFlag::YourFlag.is_enabled()` over `#[cfg(...)]` compile-time directives so flags can be toggled without recompilation and are easier to clean up later. Use `#[cfg(...)]` only when the code cannot compile without them (for example, platform-specific code or dependencies that do not exist when the feature is disabled).
- Keep flags high-level and product-focused rather than per-call-site
- Remove the flag and dead branches after launch has stabilized
- For UI sections that expose a new feature, hide the UI behind the same flag

Example:
```rust
#[derive(Sequence)]
pub enum FeatureFlag {
    YourNewFeature,
}

// Use in code
if FeatureFlag::YourNewFeature.is_enabled() {
    // gated behavior
}
```

### Exhaustive Matching

When adding/editing match statements, avoid using the wildcard _ when at all possible. Exhaustive matching is helpful for ensuring that all variants are handled, especially when adding new variants to enums in the future.
