# team

- CI runs only on tag push, so local gates are the only gates: `cargo clippy --features gui -- -D warnings`, `cargo fmt --check`, `cargo test --lib` before reporting any change as done.
- Commits follow Conventional Commits (`type(scope): description`); CONTRIBUTING.md is the source. The "Not observed" lines in older AGENTS.md copies are stale.
- Never bypass hooks. `--no-verify` is not an option.
- Never `#[allow(...)]`; `#[expect(...)]` only with a tracking-issue comment.

## spec
- Every spec names which interfaces are affected (CLI, TUI, GUI) and whether the change sits behind the `gui` feature gate.
