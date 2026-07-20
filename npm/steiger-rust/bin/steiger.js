#!/usr/bin/env node

'use strict';

const { spawnSync } = require('node:child_process');

const { packageNameFor } = require('../lib/platform.js');

const packageName = packageNameFor();
if (!packageName) {
  console.error(`steiger-rust does not provide a binary for ${process.platform}/${process.arch}`);
  process.exit(1);
}

const binaryName = process.platform === 'win32' ? 'steiger.exe' : 'steiger';
let binaryPath;
try {
  binaryPath = require.resolve(`${packageName}/bin/${binaryName}`);
} catch {
  console.error(
    `steiger-rust could not load ${packageName}. ` +
      'Reinstall without --omit=optional and make sure your npm registry serves optional dependencies.',
  );
  process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), { stdio: 'inherit' });
if (result.error) {
  console.error(`steiger-rust failed to start the native binary: ${result.error.message}`);
  process.exit(1);
}
if (result.signal) {
  process.kill(process.pid, result.signal);
}
process.exit(result.status ?? 1);
