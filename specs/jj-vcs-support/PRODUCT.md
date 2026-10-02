# jj (Jujutsu) VCS support

## Summary

Warp recognizes jj repositories and uses jj instead of git for its VCS surfaces: prompt chips, tab
titles, the code review panel, and the worktree flow (which becomes jj workspaces). When a
directory is both a jj and a git repository (colocated), jj wins.

## Problem

Warp's VCS features are git-only. In a colocated jj repo git sees a detached HEAD, so the branch
chip shows a commit hash, the commit dialog offers staging that jj doesn't have, and new worktrees
bypass jj entirely. In a non-colocated jj repo, or a secondary jj workspace, Warp shows nothing.

## Non-goals

- Remote/SSH sessions (repo detection runs daemon-side there).
- The TUI front-end's statusline.
- Warp's built-in agent features.
- Automatic cleanup of workspaces when a tab closes.

## Behavior

### Detection

1. A directory is a jj repository when it or an ancestor (up to `$HOME`) contains a `.jj/`
   directory. Detection prefers jj: at each ancestor level `.jj/` is checked before `.git`.
2. All three layouts are recognized: colocated (`.jj/` and `.git` side by side), non-colocated
   (git store in `.jj/repo/store/git`), and secondary workspaces (`.jj/repo` is a file pointing to
   the main repo).
3. `.jj/` is hidden from the file tree and file search, like `.git/`.
4. Everything is gated behind the `JjSupport` feature flag. With the flag off, behavior is
   unchanged.

### Prompt chips and tab title

5. The branch chip shows the bookmarks on `@`, falling back to bookmarks on `@-`, then to the
   shortest unique change id of `@`. A conflicted `@` shows a conflict marker.
6. The diff-stats chip shows the working-copy change (`@`) stats.
7. The "main branch" is jj's `trunk()`.
8. Clicking the branch chip lists bookmarks and workspaces. Choosing a bookmark inserts
   `jj new <bookmark>`; choosing a workspace inserts `cd <workspace path>`.
9. Rendering the prompt never runs jj. Chip data is refreshed in the background (throttled to at
   most once every 5 seconds) when files or jj's operation log change, or after a command
   finishes. Each refresh snapshots the working copy once, so stats include unrecorded edits. A
   snapshot records a jj operation only when files actually changed. Every other query uses
   `--ignore-working-copy`. The dropdown's bookmark and workspace listing never snapshots.

### Code review panel

10. Changed files are those in `@` (against `@-`), or against the fork point with `trunk()` in
    "main branch" mode. There is no staged/unstaged distinction.
11. Discarding files runs `jj restore <paths>`.
12. **Commit** describes the current change and starts a new one (`jj commit -m <message>`): the
    change is accepted and work moves on to a fresh, empty change. The "Include unstaged" toggle
    is not shown. The AI commit-message generator works on the `@` diff.
13. **Push**: if `@` or `@-` has a bookmark, it is pushed (`jj git push -b <bookmark>`). Otherwise
    the push dialog asks for a bookmark name, prefilled with jj's `push-<change id>` convention,
    creates the bookmark on `@-`, and pushes it. "Commit and push" without a bookmark uses that
    same default name.
14. **Create PR** uses `gh pr create --head <bookmark>`.
15. Git operations are never blocked in jj workspaces: jj has no merge/rebase-in-progress state,
    conflicts are recorded in commits, and every operation can be undone with `jj undo`.

### Workspaces (jj's worktrees)

16. For a jj repo, "New worktree" creates a jj workspace:
    `jj workspace add "<path>" --name <name> -r <base>`, then `cd`s into it. The new terminal or
    agent pane starts inside the workspace.
17. Workspaces live in Warp's data dir, like git worktrees: `<data_dir>/worktrees/<repo>/<name>`.
18. Generated names avoid collisions with existing workspace names (`jj workspace list`). The base
    picker lists bookmarks and `trunk()`.
19. A "Forget workspace" action runs `jj workspace forget <name>` and deletes the workspace
    directory after confirmation.
