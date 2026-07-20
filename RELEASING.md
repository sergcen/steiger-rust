# Releasing Steiger

The `Release` workflow builds eight native binaries, creates a GitHub Release with SHA-256 checksums, publishes eight platform packages to npm, and then publishes the `steiger-rust` meta-package.

## One-time repository setup

1. Create the public GitHub repository and push the default branch.
2. Enable GitHub Actions with read/write workflow permissions for releases.
3. Enable two-factor authentication on the npm maintainer account.
4. For the first release, create a short-lived granular npm token with read/write access to all packages and bypass 2FA enabled, then save it as the GitHub Actions secret `NPM_TOKEN`.

After the first release creates all nine npm packages, configure a GitHub Actions trusted publisher for each package:

- owner and repository: the GitHub repository that contains this code;
- workflow file: `release.yml`;
- environment: leave empty.

Once trusted publishing succeeds, remove the `NPM_TOKEN` secret. The workflow grants only `id-token: write` and uses GitHub-hosted runners, so npm can attach provenance to public packages.

## Publish a version

1. Update the Rust and npm manifests with the version helper.
2. Regenerate `Cargo.lock` and run all checks.
3. Verify that the release tag and every manifest agree.
4. Create and push the tag.

```bash
node scripts/set-version.mjs 0.1.0
cargo check
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo build --locked --release
node --test tests/npm_native_cli.mjs
node scripts/prepare-npm-release.mjs --check v0.1.0
git tag v0.1.0
git push origin v0.1.0
```

The workflow is idempotent for npm: packages already published at the requested version are skipped. Re-running it also replaces matching GitHub Release assets.

## Published npm packages

- `steiger-rust`
- `steiger-rust-darwin-arm64`
- `steiger-rust-darwin-x64`
- `steiger-rust-linux-arm64-gnu`
- `steiger-rust-linux-x64-gnu`
- `steiger-rust-linux-arm64-musl`
- `steiger-rust-linux-x64-musl`
- `steiger-rust-win32-arm64-msvc`
- `steiger-rust-win32-x64-msvc`
