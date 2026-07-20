import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import {
  chmod,
  cp,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  realpath,
  rm,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');

function currentPlatform() {
  if (process.platform === 'darwin' && ['arm64', 'x64'].includes(process.arch)) {
    return { packageName: `steiger-rust-darwin-${process.arch}`, binary: 'steiger' };
  }
  if (process.platform === 'linux' && ['arm64', 'x64'].includes(process.arch)) {
    const libc = process.report.getReport().header.glibcVersionRuntime ? 'gnu' : 'musl';
    return { packageName: `steiger-rust-linux-${process.arch}-${libc}`, binary: 'steiger' };
  }
  if (process.platform === 'win32' && ['arm64', 'x64'].includes(process.arch)) {
    return { packageName: `steiger-rust-win32-${process.arch}-msvc`, binary: 'steiger.exe' };
  }
  return undefined;
}

function npmPack(directory, destination) {
  const output = execFileSync(
    'npm',
    ['pack', directory, '--json', '--pack-destination', destination],
    { encoding: 'utf8' },
  );
  const [result] = JSON.parse(output);
  return { path: join(destination, result.filename), files: result.files.map(({ path }) => path) };
}

async function writeJson(path, value) {
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`);
}

test('the npm command resolves directly to a native executable', async (context) => {
  const platform = currentPlatform();
  if (!platform) {
    context.skip(`unsupported test platform: ${process.platform}/${process.arch}`);
    return;
  }

  const sourceBinary = resolve(
    process.env.STEIGER_NATIVE_BINARY ??
      join(repositoryRoot, 'target', 'release', platform.binary),
  );
  await lstat(sourceBinary);

  const temporaryRoot = await mkdtemp(join(tmpdir(), 'steiger-npm-native-'));
  context.after(() => rm(temporaryRoot, { recursive: true, force: true }));

  const platformDirectory = join(temporaryRoot, 'platform');
  const metaDirectory = join(temporaryRoot, 'meta');
  await cp(
    join(repositoryRoot, 'npm', 'platforms', platform.packageName),
    platformDirectory,
    { recursive: true },
  );
  await cp(join(repositoryRoot, 'npm', 'steiger-rust'), metaDirectory, { recursive: true });

  const packagedBinary = join(platformDirectory, 'bin', platform.binary);
  await mkdir(dirname(packagedBinary), { recursive: true });
  await cp(sourceBinary, packagedBinary);
  if (process.platform !== 'win32') await chmod(packagedBinary, 0o755);

  const platformTarball = npmPack(platformDirectory, temporaryRoot);
  assert.deepEqual(
    platformTarball.files.filter((path) => path.startsWith('bin/')),
    [`bin/${platform.binary}`],
  );

  const metaManifestPath = join(metaDirectory, 'package.json');
  const metaManifest = JSON.parse(await readFile(metaManifestPath, 'utf8'));
  assert.equal(metaManifest.bin, undefined);
  metaManifest.optionalDependencies = {
    [platform.packageName]: `file:${platformTarball.path}`,
  };
  await writeJson(metaManifestPath, metaManifest);

  const metaTarball = npmPack(metaDirectory, temporaryRoot);
  assert.equal(metaTarball.files.some((path) => path.endsWith('.js')), false);

  const consumerDirectory = join(temporaryRoot, 'consumer');
  await mkdir(consumerDirectory);
  await writeJson(join(consumerDirectory, 'package.json'), {
    private: true,
    devDependencies: { 'steiger-rust': `file:${metaTarball.path}` },
  });
  execFileSync(
    'npm',
    ['install', '--offline', '--ignore-scripts', '--no-audit', '--no-fund', '--no-package-lock'],
    { cwd: consumerDirectory, stdio: 'pipe' },
  );

  const command = join(consumerDirectory, 'node_modules', '.bin', 'steiger');
  const commandTarget = await realpath(command);
  assert.equal(basename(commandTarget), platform.binary);
  assert.match(commandTarget, new RegExp(`${platform.packageName}.+bin.+${platform.binary}$`));

  if (process.platform !== 'win32') {
    const commandStat = await lstat(command);
    assert.equal(commandStat.isSymbolicLink(), true);
  }

  const magic = (await readFile(commandTarget)).subarray(0, 4);
  const nativeMagic =
    magic.equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46])) ||
    magic.equals(Buffer.from([0xcf, 0xfa, 0xed, 0xfe])) ||
    magic.subarray(0, 2).equals(Buffer.from('MZ'));
  assert.equal(nativeMagic, true, 'steiger must be an ELF, Mach-O, or PE executable');

  const result = spawnSync(command, ['--version'], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^steiger \d+\.\d+\.\d+/);
});
