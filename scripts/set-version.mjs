import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import { platforms } from './platforms.mjs';

const [version] = process.argv.slice(2);
if (!version || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
  throw new Error('usage: node scripts/set-version.mjs <VERSION>');
}

const cargoPath = resolve('Cargo.toml');
const cargoSource = await readFile(cargoPath, 'utf8');
const packageVersionPattern = /(\[package\][\s\S]*?^version\s*=\s*)"[^"]+"/m;
if (!packageVersionPattern.test(cargoSource)) {
  throw new Error('could not find the Cargo.toml package version');
}
const updatedCargo = cargoSource.replace(packageVersionPattern, `$1"${version}"`);
await writeFile(cargoPath, updatedCargo);

const packagePaths = [
  resolve('npm/steiger-rust/package.json'),
  ...platforms.map(({ packageName }) => resolve('npm/platforms', packageName, 'package.json')),
];
for (const path of packagePaths) {
  const manifest = JSON.parse(await readFile(path, 'utf8'));
  manifest.version = version;
  delete manifest.repository;
  delete manifest.homepage;
  delete manifest.bugs;
  if (manifest.name === 'steiger-rust') {
    manifest.optionalDependencies = Object.fromEntries(
      platforms.map(({ packageName }) => [packageName, version]),
    );
  }
  await writeFile(path, `${JSON.stringify(manifest, null, 2)}\n`);
}

console.log(`updated Cargo.toml and npm manifests to ${version}`);
console.log('run cargo check to update Cargo.lock');
