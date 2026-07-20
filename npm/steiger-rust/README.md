# steiger-rust

Native npm distribution of the Steiger Feature-Sliced Design architecture linter.

```bash
npm install --save-dev steiger-rust
npx steiger ./src
```

The package installs a platform-specific Rust binary through an optional dependency. Its `steiger` command points directly to that executable and does not start Node.js. Do not install it with `--omit=optional`.
