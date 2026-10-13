import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { resolve, relative } from 'node:path';
import { generate } from './generate-contracts.mjs';
const root = resolve(import.meta.dirname, '..');
const read = path => readFile(resolve(root, path), 'utf8');
async function files(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  return (await Promise.all(entries.map(entry => entry.isDirectory() ? files(resolve(directory, entry.name)) : [resolve(directory, entry.name)]))).flat();
}
test('Rust contract is the sole DTO source and unsupported schema fails closed', async () => {
  const source = await read('crates/monitor-contracts/src/lib.rs');
  assert.equal(generate(source), await read('apps/dashboard/src/api/generated/monitor.ts'));
  assert.throws(() => generate(source.replace('scenario: DemoScenario', 'scenario: f64')));
});
test('P0 backend has no credential reader, network client, scheduler or sidecar', async () => {
  const sourceFiles = [...await files(resolve(root, 'crates')), ...await files(resolve(root, 'apps/desktop/src-tauri/src'))].filter(path => path.endsWith('.rs') || path.endsWith('Cargo.toml'));
  for (const path of sourceFiles) assert.doesNotMatch((await readFile(path, 'utf8')).split('#[cfg(test)]')[0], /reqwest|ureq|auth\.json|refresh_token|Authorization|std::process::Command|tokio::time::interval/i, relative(root, path));
  const config = JSON.parse(await read('apps/desktop/src-tauri/tauri.conf.json'));
  assert.equal(config.identifier, 'dev.agentusage.monitor');
  assert.equal(config.bundle.externalBin, undefined);
  assert.doesNotMatch(config.app.security.csp, /https:|https?:\/\/(?!ipc\.localhost)/);
  assert.equal(config.build.devUrl, 'http://127.0.0.1:1430');
});
test('frontend Tauri imports stay within the transport and preferences are independent', async () => {
  const sourceFiles = (await files(resolve(root, 'apps/dashboard/src'))).filter(path => /\.(ts|tsx)$/.test(path));
  for (const path of sourceFiles) {
    const source = await readFile(path, 'utf8');
    if (!path.replaceAll('\\', '/').includes('/transports/tauri/')) assert.doesNotMatch(source, /from ['"]@tauri-apps\//, relative(root, path));
  }
  assert.match(await read('apps/dashboard/src/i18n/messages.ts'), /agent-usage-monitor\.language/);
  assert.match(await read('apps/dashboard/src/app/App.tsx'), /agent-usage-monitor\./);
});
