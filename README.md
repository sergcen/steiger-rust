<h1 align="center">Steiger for Rust</h1>

<p align="center">
  A fast, native architecture linter for Feature-Sliced Design projects.
</p>

<p align="center">
  <a href="https://github.com/sergcen/steiger-rust/actions/workflows/ci.yml">
    <img alt="CI" src="https://github.com/sergcen/steiger-rust/actions/workflows/ci.yml/badge.svg?branch=main">
  </a>
  <a href="https://www.npmjs.com/package/steiger-rust">
    <img alt="npm version" src="https://img.shields.io/npm/v/steiger-rust">
  </a>
  <a href="https://www.npmjs.com/package/steiger-rust">
    <img alt="npm downloads" src="https://img.shields.io/npm/dm/steiger-rust">
  </a>
  <a href="LICENSE">
    <img alt="MIT license" src="https://img.shields.io/github/license/sergcen/steiger-rust">
  </a>
</p>

Steiger for Rust is a Node.js-free rewrite of
[feature-sliced/steiger](https://github.com/feature-sliced/steiger). It checks
the architecture of JavaScript and TypeScript projects that use
[Feature-Sliced Design](https://feature-sliced.design/) while keeping startup
and analysis inside a native Rust executable.

- Compatible with Steiger 0.6 and `@feature-sliced/steiger-plugin` 0.7 behavior.
- Includes all 20 FSD rules and enables the same 17-rule recommended preset.
- Uses Oxc for JavaScript and TypeScript parsing and Rayon for parallel work.
- Supports checked-in diagnostic baselines for gradual adoption in CI and
  pre-push hooks.

## Quick start

Install the npm package and lint the directory that contains your FSD layers:

```bash
npm install --save-dev steiger-rust
npx steiger ./src
```

A project without diagnostics prints:

```text
✓ No problems found!
```

The `steiger-rust` meta-package installs the native package for the current
operating system and architecture. Its `steiger` entry points directly to the
Rust executable: npm installs and resolves the command, but the linter itself
does not start Node.js or a JavaScript launcher.

> [!IMPORTANT]
> Do not install with `--omit=optional`. The platform-specific native binary is
> delivered as an optional dependency.

## Performance

The following end-to-end CLI benchmark was measured on July 21, 2026, using
the Terminal PWA: a 64 MB source tree containing 10,378 JavaScript, TypeScript,
and component files. Both implementations ran only
`fsd/insignificant-slice`, used the same mock-widget exclusions, and returned
the same 87 diagnostics.

| Implementation | Version | Runs | Median wall time | Relative |
| --- | --- | ---: | ---: | ---: |
| JavaScript Steiger | `steiger 0.6.0` + plugin `0.7.0` | 3 | 200.73 s | 1× |
| Steiger for Rust | `0.1.0` release build | 10 | 0.855 s | ~235× faster |

Environment: Apple M2 Max, macOS, Node.js 22.14.0. Runs used a warm filesystem
cache, the JSON reporter, and discarded reporter output. The JavaScript process
was run with `ulimit -n 120000`; runs at 65,536 file descriptors stopped with
`EMFILE: too many open files`.

Raw wall times were `201.14, 200.73, 200.49 s` for JavaScript and
`0.85, 0.66, 0.86, 0.89, 0.86, 0.65, 0.86, 0.88, 0.66, 0.65 s` for Rust.
These are project-specific results rather than a universal performance
guarantee; filesystem, enabled rules, project shape, and cache state all affect
runtime.

## Applicability and compatibility

Use Steiger for Rust in local development, pre-push validation, or CI for FSD
projects whose source is JavaScript, TypeScript, Vue, Svelte, or Astro. Linting
does not require authentication or a network connection after installation.

| Area | Supported targets |
| --- | --- |
| Operating systems | macOS, Linux, Windows |
| Architectures | x64 and arm64 |
| npm Linux variants | glibc and musl |
| Source files | JS, JSX, TS, TSX, MJS, CJS, MTS, CTS |
| Embedded scripts | Vue, Svelte, Astro |
| Configuration | TOML and JSON |
| Source builds | Rust 1.89 or newer |

GitHub Releases and npm publish native binaries for these targets:

| Operating system | Architecture | Target family |
| --- | --- | --- |
| macOS | x64, arm64 | Darwin |
| Linux | x64, arm64 | GNU/glibc and musl |
| Windows | x64, arm64 | MSVC |

Other Rust targets can be built from source when all dependencies support the
target.

## Usage

When `PATH` is omitted, Steiger selects `./src`, then `./app`, then the current
directory.

```bash
steiger [OPTIONS] [PATH]
```

| Option | Purpose |
| --- | --- |
| `-w`, `--watch` | Re-run when project files change |
| `--fix` | Apply available automatic fixes |
| `--fail-on-warnings` | Return a failing status when warnings remain |
| `--reporter pretty\|json` | Select human-readable or JSON output |
| `--config FILE` | Use an explicit TOML or JSON configuration |
| `--baseline FILE` | Fail only on diagnostics absent from a baseline |
| `--fsd-errors FILE` | Alias for `--baseline` |
| `--list-rules` | Print every built-in rule and its default state |

Common commands:

```bash
steiger ./src --watch
steiger ./src --fix
steiger ./src --reporter json
steiger ./src --fail-on-warnings
steiger ./src --baseline ./fsd-errors.json
steiger --list-rules
```

### Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Validation passed |
| `1` | Errors remain, warnings fail, or new baseline diagnostics were found |
| `2` | The path, configuration, or another CLI input is invalid |

## Validate against existing FSD debt

`--baseline` lets a project commit its known FSD diagnostics and reject only
new `(rule, path)` pairs. The file is a JSON object keyed by rule name; every
path is resolved relative to the baseline file.

```json
{
  "fsd/insignificant-slice": [
    "src/entities/user"
  ]
}
```

Run the validation from a hook or CI job:

```bash
steiger ./src --baseline ./fsd-errors.json
```

Known diagnostics are hidden. The command returns `0` when there are no new
pairs and `1` when current output contains a pair absent from the baseline. A
reduction in existing diagnostics succeeds without modifying the committed
file.

For an npm project, the same validation can be exposed as a package script:

```json
{
  "scripts": {
    "fsd:check": "steiger ./src --baseline ./fsd-errors.json"
  }
}
```

```bash
npm run fsd:check
```

## Configuration

Without `--config`, Steiger searches the current directory and its parents for:

- `steiger.toml`
- `.steiger.toml`
- `steiger.config.toml`
- `steiger.json`
- `.steiger.json`
- `steiger.config.json`

> [!IMPORTANT]
> JavaScript and TypeScript configuration files such as `steiger.config.js`
> are not evaluated. Executing them would add a Node.js runtime dependency.
> Move the rule and override settings to TOML or JSON; plugin objects are not
> needed because every supported FSD rule is built into the binary.

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

JSON also accepts a Steiger-style flat array:

```json
[
  { "ignores": ["**/__mocks__/**"] },
  {
    "files": ["./src/shared/**"],
    "rules": { "fsd/public-api": "off" }
  }
]
```

Later matching overrides take precedence. A folder diagnostic receives the
highest effective severity of the files below that folder, matching upstream
Steiger's file-glob behavior.

## Language and module support

- JavaScript, JSX, TypeScript, TSX, MJS, CJS, MTS, and CTS through Oxc.
- `import`, `import()`, and `require()` dependencies.
- Vue and Svelte `<script>` blocks.
- Astro frontmatter.
- Relative imports, directory indexes, TypeScript `baseUrl`/`paths`, `extends`,
  and project references through `oxc_resolver`.
- `.gitignore`, `.ignore`, `.git`, `node_modules`, and Rust `target` exclusions.

## Rules

The recommended preset enables these 17 rules:

- `fsd/ambiguous-slice-names`
- `fsd/excessive-slicing`
- `fsd/forbidden-imports`
- `fsd/inconsistent-naming`
- `fsd/insignificant-slice`
- `fsd/no-layer-public-api`
- `fsd/no-processes`
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

The upstream-compatible split rules `fsd/no-cross-imports`,
`fsd/no-higher-level-imports`, and `fsd/import-locality` are built in but
disabled by default. Run `steiger --list-rules` for the machine-readable list.

## Install from source

```bash
cargo install --path . --locked
steiger ./src
```

Prebuilt archives and SHA-256 checksums are also available on the
[GitHub Releases page](https://github.com/sergcen/steiger-rust/releases).

## Development

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo build --locked --release
```

The npm integration test verifies that `.bin/steiger` resolves directly to a
native Mach-O, ELF, or PE executable:

```bash
node --test tests/npm_native_cli.mjs
node scripts/prepare-npm-release.mjs --check v0.1.0
```

### Repository map

| Path | Purpose |
| --- | --- |
| `src/main.rs` | CLI arguments, reporters, exit codes, and watch mode |
| `src/config.rs` | TOML/JSON discovery, parsing, and override scoping |
| `src/rules/` | Built-in structural and import rules |
| `tests/` | CLI, configuration, compatibility, and npm integration tests |
| `npm/` | Meta-package and eight platform-specific packages |
| `.github/workflows/` | Multi-platform CI and tag-based release publishing |

See [CONTRIBUTING.md](CONTRIBUTING.md) for pull request guidance and
[RELEASING.md](RELEASING.md) for the tag-based GitHub and npm release process.

## License and upstream relationship

This project is an independent Rust implementation derived from the behavior
and tests of the MIT-licensed upstream Steiger project. It is released under
the [MIT License](LICENSE).
