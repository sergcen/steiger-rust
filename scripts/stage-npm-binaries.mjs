import { chmod, copyFile, mkdir, stat } from 'node:fs/promises';
import { resolve } from 'node:path';

import { platforms } from './platforms.mjs';

const [artifactRoot = 'artifacts'] = process.argv.slice(2);

for (const { packageName, binary } of platforms) {
  const source = resolve(artifactRoot, packageName, binary);
  const destinationDirectory = resolve('npm/platforms', packageName, 'bin');
  const destination = resolve(destinationDirectory, binary);
  await stat(source).catch(() => {
    throw new Error(`missing release binary: ${source}`);
  });
  await mkdir(destinationDirectory, { recursive: true });
  await copyFile(source, destination);
  await copyFile(resolve('LICENSE'), resolve('npm/platforms', packageName, 'LICENSE'));
  if (binary !== 'steiger.exe') {
    await chmod(destination, 0o755);
  }
}

await copyFile(resolve('LICENSE'), resolve('npm/steiger-rust/LICENSE'));
await copyFile(resolve('README.md'), resolve('npm/steiger-rust/README.md'));

console.log(`staged ${platforms.length} npm platform binaries`);
