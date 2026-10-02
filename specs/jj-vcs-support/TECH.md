# jj VCS support: technical design

See `PRODUCT.md` for behavior. The existing integration is git-hardcoded: there is no VCS trait, and
git runs through `warp_util::git::run_git_command` plus ad-hoc `Command::new("git")` calls. The plan
adds a VCS kind and routes to jj at the existing seams. Each phase ships on its own behind
`FeatureFlag::JjSupport`.

## Phase 1: Detection

- `crates/repo_metadata/src/repositories.rs`: `find_git_repo` (:338) checks `.jj/` before `.git` at
  each ancestor. `GitRepoInfo` gains `vcs: VcsKind`, where `VcsKind = Git | Jj { colocated: bool }`.
  For a secondary workspace, `.jj/repo` is a file whose contents point at the main repo's `.jj/repo`.
- `crates/repo_metadata/src/repository.rs`: `Repository` stores the kind, and `git_watch_paths`
  (:350) gains a jj counterpart that watches `.jj/repo/op_heads/heads` (a new operation means
  changed state) and `.jj/working_copy`.
- `crates/repo_metadata/src/entry.rs` / `watcher.rs`: add `.jj` classifiers next to the git ones,
  and ignore `.jj/` in `path_is_ignored` (`local_model.rs:1808`).
- `DetectedRepositoriesEvent::DetectedGitRepo` carries the kind. The name stays for now, to limit
  churn.
- New `crates/warp_util/src/jj.rs`: `run_jj_command(repo, args)`, which always passes
  `--no-pager --color=never` and optionally `--ignore-working-copy`.

## Phase 2: Status model and chips

- `app/src/code_review/git_repo_model/`: add a `LocalJjRepoStatusModel` and route to it from
  `GitRepoStatusModel` (either a new variant or a backend inside `Local`). It produces the same
  `GitStatusMetadata`, so `CurrentPrompt::apply_git_repo_metadata` (`current_prompt.rs:1588`) needs
  no chip changes:
  - `current_branch_name`: `jj log -r @ --no-graph -T 'bookmarks'`, then `@-`, then
    `change_id.shortest()`.
  - `main_branch_name`: `jj log -r 'trunk()' --no-graph -T 'bookmarks'`.
  - `stats_against_head`: `jj diff -r @ --stat`.
  - `branch_tracking_status`: ahead/behind of the bookmark against `<bookmark>@origin`.
- `app/src/context_chips/git_branch_on_click.rs` / `display_chip.rs`: a jj menu source (bookmarks +
  `jj workspace list`) and a `JjNew`/`ChangeDirectory` chip command in place of `GitCheckout`.
- `ShellCommandGenerator` invalidation lists add `"jj"` for the remote-fallback generators.

## Phase 3: Code review

- `app/src/code_review/diff_state/local.rs`: split the git calls behind a backend so
  `LocalDiffStateModel` can drive jj:

  | Purpose | Git | jj |
  |---|---|---|
  | File status vs head | `status --porcelain=2` | `diff -r @ --summary` |
  | File status vs main | `diff --name-status <merge-base>` | `diff --from 'fork_point(trunk() \| @)' --to @ --summary` |
  | Per-file diff | `diff --patch-with-raw -z HEAD -- path` | `diff --git -r @ -- path` |
  | Base content | `show HEAD:path` | `file show -r @- path` |
  | Numstat | `diff --numstat` | `diff --stat` (parsed) |
  | Discard | stash/restore/clean | `restore <paths>` |

- Git ops (`app/src/code_review/git_actions.rs`, `util/git.rs`): a `run_jj_commit` runs
  `jj commit -m`; push resolves a bookmark on `@-`, or `jj bookmark create <name> -r @-`, then
  `jj git push -b <name>`. `CommitState.include_unstaged` is ignored and its toggle hidden for jj
  (`git_dialog/commit.rs:554`). The push dialog gains a bookmark-name field for jj.
- `is_git_operation_blocked` returns `false` for jj workspaces; see PRODUCT.md #15.
- `get_pr_for_branch` (`util/git.rs:851`) takes the bookmark rather than `symbolic-ref HEAD`, which
  fails under jj's detached HEAD.

## Phase 4: Workspaces

- `app/src/tab_configs/tab_config.rs`: `build_worktree_config_toml` (:419) emits
  `jj workspace add "<path>" --name {{branch}} -r <base>` for jj repos. `is_worktree()` (:179) also
  matches `jj workspace`. Paths reuse `generated_worktree_path` (:42-58).
- Name generation: feed `generate_worktree_branch_name` with `jj workspace list` names instead of
  `list_local_branches_sync`.
- `app/src/tab_configs/branch_picker.rs`: list bookmarks + `trunk()` for jj repos.
- `app/src/workspace/view.rs`: `should_include_worktree_sidecar_repo` (:10578) excludes secondary jj
  workspaces the same way it excludes linked git worktrees.
- New "Forget workspace" action: `jj workspace forget <name>`, then delete the directory after
  confirmation.

## Phase 5: Other consumers

- `app/src/ai/blocklist/handoff/touched_repos.rs` `find_git_root` (:125).
- `app/src/search/files/model.rs:686` `get_git_changed_files`.
- `app/src/tab_configs/session_config.rs:190` `is_git_repo`.

## Testing

- Unit tests per phase with temp repos created by `jj git init` (colocated and not) plus
  `jj workspace add`. Tests skip when `jj` isn't on PATH.
- Parser tests for `--summary`/`--stat`/template output use fixed fixtures and need no jj binary.
