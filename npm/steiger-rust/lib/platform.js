'use strict';

const PACKAGES = Object.freeze({
  'darwin-arm64': 'steiger-rust-darwin-arm64',
  'darwin-x64': 'steiger-rust-darwin-x64',
  'linux-arm64-gnu': 'steiger-rust-linux-arm64-gnu',
  'linux-x64-gnu': 'steiger-rust-linux-x64-gnu',
  'linux-arm64-musl': 'steiger-rust-linux-arm64-musl',
  'linux-x64-musl': 'steiger-rust-linux-x64-musl',
  'win32-arm64-msvc': 'steiger-rust-win32-arm64-msvc',
  'win32-x64-msvc': 'steiger-rust-win32-x64-msvc',
});

function usesMusl(report = process.report) {
  if (!report || typeof report.getReport !== 'function') {
    return false;
  }
  return !report.getReport().header.glibcVersionRuntime;
}

function packageNameFor(
  platform = process.platform,
  arch = process.arch,
  musl = platform === 'linux' && usesMusl(),
) {
  const suffix = platform === 'linux' ? (musl ? '-musl' : '-gnu') : platform === 'win32' ? '-msvc' : '';
  return PACKAGES[`${platform}-${arch}${suffix}`] ?? null;
}

module.exports = { packageNameFor, usesMusl };
