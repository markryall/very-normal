# Brief

`very-normal plan | head -3` panics with "failed printing to stdout: Broken pipe". Rust ignores SIGPIPE, so writing after `head` exits returns EPIPE and `println!` panics. Restore the default SIGPIPE handling so it exits quietly like other Unix tools.

---

## Goal

Commands whose output is cut off by a closed pipe exit quietly instead of panicking.

## Acceptance criteria

1. On Unix, `main` resets SIGPIPE to `SIG_DFL` before doing anything else.
2. `very-normal plan | head -1` prints one line and nothing on stderr.
3. `very-normal --help | head -1` prints one line and nothing on stderr.
4. The TUI and every other command behave as before; `cargo test` and `cargo clippy --all-targets -- -D warnings` pass.

## Out of scope

- Replacing `println!` with fallible writes.
- Windows, which has no SIGPIPE.

## Open questions

None.
