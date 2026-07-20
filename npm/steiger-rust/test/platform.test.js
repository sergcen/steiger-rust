'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { packageNameFor, usesMusl } = require('../lib/platform.js');

test('selects every published platform package', () => {
  assert.equal(packageNameFor('darwin', 'arm64'), 'steiger-rust-darwin-arm64');
  assert.equal(packageNameFor('darwin', 'x64'), 'steiger-rust-darwin-x64');
  assert.equal(packageNameFor('linux', 'arm64', false), 'steiger-rust-linux-arm64-gnu');
  assert.equal(packageNameFor('linux', 'x64', false), 'steiger-rust-linux-x64-gnu');
  assert.equal(packageNameFor('linux', 'arm64', true), 'steiger-rust-linux-arm64-musl');
  assert.equal(packageNameFor('linux', 'x64', true), 'steiger-rust-linux-x64-musl');
  assert.equal(packageNameFor('win32', 'arm64'), 'steiger-rust-win32-arm64-msvc');
  assert.equal(packageNameFor('win32', 'x64'), 'steiger-rust-win32-x64-msvc');
});

test('rejects unsupported operating systems and architectures', () => {
  assert.equal(packageNameFor('freebsd', 'x64'), null);
  assert.equal(packageNameFor('darwin', 'ia32'), null);
});

test('detects musl from the Node diagnostic report', () => {
  assert.equal(usesMusl({ getReport: () => ({ header: { glibcVersionRuntime: '2.39' } }) }), false);
  assert.equal(usesMusl({ getReport: () => ({ header: {} }) }), true);
});
