# Jiffy

Jiffy is a local-only terminal, forked from the open-source [Warp](https://github.com/warpdotdev/warp)
client. It keeps Warp's fast GPU-rendered terminal, blocks, editor, and code review panel, and
removes everything that depends on Warp's cloud: there is no account, sign-in, Warp Drive, Warp AI
agent, session sharing, autoupdate, crash reporting, or telemetry.

On top of that, Jiffy adds:

- **Jujutsu (jj) support**: jj repositories are detected alongside git, with jj-aware diffs in
  code review and a jj workspace tab config.
- **Third-party CLI agent integration**: first-class support for running
  [Claude Code](https://www.anthropic.com/claude-code) (and other CLI agents) in the terminal,
  including rich input and agent notifications.

Jiffy targets Linux. The macOS and Windows code paths inherited from Warp are still in the tree but
are not actively maintained.

## Building and running

Install the build dependencies once (on Debian/Ubuntu this installs the required system packages,
the Rust toolchain, and cargo tools):

```bash
./script/bootstrap
```

Then build and run Jiffy:

```bash
./script/run        # builds and runs the `jiffy` binary with the `gui` feature
# or, equivalently:
cargo run -p jiffy --bin jiffy --features gui
```

On Linux, `.cargo/config.toml` links with `clang` and the [mold](https://github.com/rui314/mold)
linker (`-fuse-ld=mold`), so both must be installed and on your `PATH`. `./script/bootstrap`
installs them on Debian-based distributions; on other distributions install them with your package
manager.

Logs are written to `~/.local/state/jiffy/jiffy.log`.

See [AGENTS.md](AGENTS.md) for the development guide (testing, linting, and code style).

## License

Jiffy is a derivative of the Warp client and keeps its licensing:

- The UI framework (the `warpui_core` and `warpui` crates) is licensed under the
  [MIT license](LICENSE-MIT).
- The rest of the code in this repository is licensed under the [AGPL v3](LICENSE-AGPL).

Jiffy is not affiliated with or endorsed by Warp. All credit for the upstream codebase goes to
[Warp](https://github.com/warpdotdev/warp) and its contributors.
