use clap::Parser;

use super::*;

#[test]
fn identifies_worker_subcommands() {
    assert!(is_worker_invocation("ripgrep-search"));
    #[cfg(unix)]
    assert!(is_worker_invocation(&terminal_server_subcommand()));
    assert!(!is_worker_invocation("--prompt"));
}

#[test]
fn parses_without_subcommand() {
    let args = Args::try_parse_from(["jiffy"]).expect("bare invocation should parse");
    assert!(args.command().is_none());
}
