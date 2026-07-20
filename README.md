# Steiger for Rust

A native, Node.js-free rewrite of [feature-sliced/steiger](https://github.com/feature-sliced/steiger): a project-architecture linter for [Feature-Sliced Design](https://feature-sliced.design/).

The current implementation follows Steiger 0.6.0 and `@feature-sliced/steiger-plugin` 0.7.0. It contains the same 20 FSD rules, enables the same 17-rule recommended preset, keeps the familiar CLI flags, and uses Oxc to analyze JavaScript and TypeScript imports.

## Quick start

```bash
npm install --save-dev steiger-rust
npx steiger ./src
```

The npm launcher downloads only the native binary for the current operating system and architecture. It requires Node.js 18.18 or newer to launch the binary; Steiger itself does not embed Node.js.

To build from source instead:

```bash
cargo install --path . --locked
steiger ./src
```

When no path is provided, Steiger selects `./src`, then `./app`, then the current folder.

```bash
steiger ./src --watch
steiger ./src --fix
steiger ./src --reporter json
steiger ./src --fail-on-warnings
steiger ./src --baseline ./fsd-errors.json
steiger --list-rules
```

The exit code is `1` when errors remain, or when warnings remain together with `--fail-on-warnings`. Invalid input or configuration returns `2`.

## Supported platforms

GitHub Releases and npm provide native binaries for:

| Operating system | Architectures | Runtime variant |
| --- | --- | --- |
| macOS | x64, arm64 | Darwin |
| Linux | x64, arm64 | glibc and musl |
| Windows | x64, arm64 | MSVC |

Other Rust targets can still be built from source when the dependencies support them.

### Validate against existing FSD debt

For gradual migrations, `--baseline` accepts the checked-in `fsd-errors.json` format used by Terminal: a JSON object whose keys are rule names and whose values are arrays of paths. Paths are resolved relative to the baseline file.

```bash
steiger ./src --baseline ./fsd-errors.json
# --fsd-errors is an alias for --baseline
```

Known `(rule, path)` diagnostics are hidden. The command exits with `0` when no new diagnostics appear and `1` when the current result contains a pair absent from the baseline, making it suitable for pre-push hooks and CI. A reduction in existing diagnostics succeeds without rewriting the committed file.

## Configuration

The native binary works without a configuration file. It searches the current folder and its parents for `steiger.toml`, `.steiger.toml`, `steiger.config.toml`, `steiger.json`, `.steiger.json`, or `steiger.config.json`.

Example `steiger.toml`:

```toml
global_ignores = ["**/__mocks__/**"]

[rules]
"fsd/no-processes" = "warn"

[[overrides]]
files = ["./src/shared/**"]

[overrides.rules]
"fsd/public-api" = "off"

[[overrides]]
files = ["./src/widgets/**"]
ignores = ["**/discount-offers/**"]

[overrides.rules]
"fsd/no-segmentless-slices" = "off"
```

JSON also accepts a Steiger-style flat array. Rules are built into the binary, so plugin objects are omitted:

```json
[
  { "ignores": ["**/__mocks__/**"] },
  {
    "files": ["./src/shared/**"],
    "rules": { "fsd/public-api": "off" }
  }
]
```

Later matching overrides take precedence. A folder diagnostic receives the highest effective severity of the files below that folder, matching upstream Steiger's file-glob behavior.

JavaScript and TypeScript configuration files are intentionally not evaluated: doing that would make the native binary depend on Node.js. Use TOML or JSON, or pass a file explicitly with `--config`.

## Language and module support

- JavaScript, JSX, TypeScript, TSX, MJS, CJS, MTS, and CTS through Oxc
- `import`, `import()`, and `require()` dependencies
- Vue and Svelte `<script>` blocks
- Astro frontmatter
- relative imports, directory indexes, TypeScript `baseUrl`/`paths`, `extends`, and project references through `oxc_resolver`
- `.gitignore`, `.ignore`, `.git`, `node_modules`, and Rust `target` exclusions

## Rules

Run `steiger --list-rules` for the machine-readable list. The recommended preset includes:

- `fsd/ambiguous-slice-names`
- `fsd/excessive-slicing`
- `fsd/forbidden-imports`
- `fsd/inconsistent-naming`
- `fsd/insignificant-slice`
- `fsd/no-layer-public-api`
- `fsd/no-public-api-sidestep`
- `fsd/no-reserved-folder-names`
- `fsd/no-segmentless-slices`
- `fsd/no-segments-on-sliced-layers`
- `fsd/no-ui-in-app`
- `fsd/public-api`
- `fsd/repetitive-naming`
- `fsd/segments-by-purpose`
- `fsd/shared-lib-grouping`
- `fsd/typo-in-layer-name`
- `fsd/no-processes`

The upstream-compatible split rules `fsd/no-cross-imports`, `fsd/no-higher-level-imports`, and `fsd/import-locality` are available but disabled by default.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

The npm launcher has a separate test suite:

```bash
npm --prefix npm/steiger-rust test
node scripts/prepare-npm-release.mjs --check v0.1.0
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for pull request guidance and [RELEASING.md](RELEASING.md) for the tag-based GitHub/npm release process.

## License and upstream relationship

This project is an independent Rust implementation derived from the behavior and tests of the MIT-licensed upstream Steiger project.

Released under the [MIT License](LICENSE).
