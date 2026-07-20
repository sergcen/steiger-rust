import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import { platforms } from './platforms.mjs';

const [mode = '--check', rawTag = process.env.GITHUB_REF_NAME] = process.argv.slice(2);

if (!['--check', '--write'].includes(mode) || !rawTag) {
  throw new Error('usage: node scripts/prepare-npm-release.mjs <--check|--write> <vVERSION>');
}

const version = rawTag.startsWith('v') ? rawTag.slice(1) : rawTag;
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
  throw new Error(`invalid release tag: ${rawTag}`);
}

const cargoSource = await readFile(resolve('Cargo.toml'), 'utf8');
const cargoVersion = cargoSource.match(/\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m)?.[1];
if (cargoVersion !== version) {
  throw new Error(`Cargo.toml version ${cargoVersion ?? '<missing>'} does not match ${version}`);
}
const lockSource = await readFile(resolve('Cargo.lock'), 'utf8');
const lockVersion = lockSource.match(/\[\[package\]\]\nname = "steiger"\nversion = "([^"]+)"/)?.[1];
if (lockVersion !== version) {
  throw new Error(`Cargo.lock version ${lockVersion ?? '<missing>'} does not match ${version}`);
}

const packagePaths = [
  resolve('npm/steiger-rust/package.json'),
  ...platforms.map(({ packageName }) => resolve('npm/platforms', packageName, 'package.json')),
];
const manifests = await Promise.all(
  packagePaths.map(async (path) => ({ path, value: JSON.parse(await readFile(path, 'utf8')) })),
);
const expectedDependencies = Object.fromEntries(
  platforms.map(({ packageName }) => [packageName, version]),
);
const releaseWorkflow = await readFile(resolve('.github/workflows/release.yml'), 'utf8');
const workflowPairs = [...releaseWorkflow.matchAll(/target: ([^\s]+)\n\s+package: ([^\s]+)/g)]
  .map(([, target, packageName]) => ({ packageName, target }))
  .sort((left, right) => left.packageName.localeCompare(right.packageName));
const expectedPairs = platforms
  .map(({ packageName, target }) => ({ packageName, target }))
  .sort((left, right) => left.packageName.localeCompare(right.packageName));
if (JSON.stringify(workflowPairs) !== JSON.stringify(expectedPairs)) {
  throw new Error('release workflow matrix does not match scripts/platforms.mjs');
}

for (const { path, value } of manifests) {
  if (value.version !== version) {
    throw new Error(`${path} version ${value.version ?? '<missing>'} does not match ${version}`);
  }
}

const rootManifest = manifests[0].value;
if (JSON.stringify(rootManifest.optionalDependencies) !== JSON.stringify(expectedDependencies)) {
  throw new Error('npm/steiger-rust optionalDependencies do not match the platform package list');
}

if (mode === '--write') {
  const server = process.env.GITHUB_SERVER_URL;
  const repository = process.env.GITHUB_REPOSITORY;
  if (!server || !repository) {
    throw new Error('GITHUB_SERVER_URL and GITHUB_REPOSITORY are required in --write mode');
  }
  const repositoryUrl = `git+${server}/${repository}.git`;
  for (const manifest of manifests) {
    manifest.value.repository = { type: 'git', url: repositoryUrl };
    manifest.value.homepage = `${server}/${repository}#readme`;
    manifest.value.bugs = { url: `${server}/${repository}/issues` };
    await writeFile(manifest.path, `${JSON.stringify(manifest.value, null, 2)}\n`);
  }
}

console.log(`npm release manifests are ready for ${version}`);
