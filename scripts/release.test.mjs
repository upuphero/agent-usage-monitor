import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, mkdtempSync, writeFileSync, mkdirSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve, join } from 'node:path';
import { spawnSync } from 'node:child_process';
const root = resolve(import.meta.dirname, '..');
const version = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
const fixtureRoot = join(root, '.local-data');
mkdirSync(fixtureRoot, { recursive: true });
const hash = value => createHash('sha256').update(value).digest('hex');
const names = [`Agent Usage Monitor_${version}_x64-setup.exe`, `Agent-Usage-Monitor_${version}_x64-portable.zip`];
const run = directory => spawnSync('pwsh', ['-NoProfile','-File',join(root,'scripts/publish-release.ps1'),'-ArtifactDirectory',directory,'-Commit','a'.repeat(40),'-Repository','fixture/repository','-VerifyOnly'], { cwd: root, encoding:'utf8' });

test('release verification accepts separate correctly hashed packages without GitHub requests', () => {
  const directory = mkdtempSync(join(fixtureRoot, 'release-valid-'));
  const rows = names.map((name, index) => { const body = Buffer.from(`synthetic-package-${index}`); writeFileSync(join(directory,name),body); return `${hash(body)}  ${name}`; });
  writeFileSync(join(directory,'SHA256SUMS.txt'),rows.join('\n'));
  const result = run(directory);
  assert.equal(result.status,0, result.stderr); assert.match(result.stdout, new RegExp(`Verified release packages for v${version.replaceAll('.','\\.')}`));
});
test('release verification rejects corruption and unexpected files before publication', () => {
  const directory = mkdtempSync(join(fixtureRoot,'release-invalid-'));
  for (const name of names) writeFileSync(join(directory,name),'synthetic-package');
  writeFileSync(join(directory,'SHA256SUMS.txt'),names.map(name => `${'0'.repeat(64)}  ${name}`).join('\n'));
  assert.notEqual(run(directory).status,0);
  writeFileSync(join(directory,'SHA256SUMS.txt'),`${hash('synthetic-package')}  unexpected-file.exe\n`);
  assert.notEqual(run(directory).status,0);
});
