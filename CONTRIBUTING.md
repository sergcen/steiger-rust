# Contributing

Bug fixes, compatibility improvements, performance work, documentation, and tests are welcome. For large behavioral changes, open an issue first so the expected compatibility with upstream Steiger can be agreed on before implementation.

## Development setup

Steiger requires Rust 1.89 or newer. Node.js is used only by npm packaging and release tooling; the installed `steiger` command is the native Rust executable.

```bash
cargo build
cargo test --all-features
cargo build --release
node --test tests/npm_native_cli.mjs
```

Before opening a pull request, run:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo build --locked --release
node --test tests/npm_native_cli.mjs
```

## Pull requests

- Explain the user-visible behavior and why the change is needed.
- Add or update tests for behavioral changes.
- Preserve compatibility with Steiger 0.6 and `@feature-sliced/steiger-plugin` 0.7 unless the pull request explicitly proposes a compatibility update.
- Update the README when flags, configuration, supported targets, or installation behavior changes.
- Keep unrelated refactors out of focused fixes.

Maintainers may request changes before merging. By contributing, you agree that your contribution is licensed under the repository's MIT License.
