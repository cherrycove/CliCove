import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { collectArtifacts, generateManifest, verifyVersion } from './prepare-release.mjs';

function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), 'clicove-release-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  return root;
}
test('requires every platform and maps MSI and NSIS to their original installer types', (t) => {
  const root = fixture(t);
  const names = ['aarch64-apple-darwin.app.tar.gz', 'x86_64-apple-darwin.app.tar.gz', 'x86_64-pc-windows-msvc.msi', 'x86_64-pc-windows-msvc-setup.exe'];
  assert.throws(() => generateManifest(root, '0.6.12', 'cherrycove/CliCove', ''), /Missing update package/);
  const signature = Buffer.from('untrusted comment: test signature fixture').toString('base64');
  for (const name of names) {
    writeFileSync(path.join(root, `CliCove_0.6.12_${name}`), 'package');
    writeFileSync(path.join(root, `CliCove_0.6.12_${name}.sig`), signature);
  }
  const manifest = generateManifest(root, '0.6.12', 'cherrycove/CliCove', '更新说明');
  assert.equal(Object.keys(manifest.platforms).length, 5);
  assert.match(manifest.platforms['windows-x86_64-msi'].url, /\.msi$/);
  assert.match(manifest.platforms['windows-x86_64-nsis'].url, /-setup\.exe$/);
  assert.equal(manifest.platforms['darwin-aarch64'].signature, signature);
  writeFileSync(path.join(root, 'CliCove_0.6.12_x86_64-pc-windows-msvc.msi.sig'), '');
  assert.throws(() => generateManifest(root, '0.6.12', 'cherrycove/CliCove', ''), /Invalid signature/);
});
test('collects architecture-qualified names and rejects incomplete builds', (t) => {
  const root = fixture(t);
  const bundle = path.join(root, 'bundle');
  const out = path.join(root, 'out');
  mkdirSync(bundle);
  for (const name of ['CliCove.dmg', 'CliCove.app.tar.gz', 'CliCove.app.tar.gz.sig']) writeFileSync(path.join(bundle, name), name);
  collectArtifacts(bundle, 'aarch64-apple-darwin', out, '0.6.12');
  assert.equal(readFileSync(path.join(out, 'CliCove_0.6.12_aarch64-apple-darwin.app.tar.gz'), 'utf8'), 'CliCove.app.tar.gz');
  rmSync(path.join(bundle, 'CliCove.app.tar.gz.sig'));
  assert.throws(() => collectArtifacts(bundle, 'aarch64-apple-darwin', out, '0.6.12'), /Expected exactly one/);
});
test('rejects inconsistent versions and non-stable release tags', (t) => {
  const root = fixture(t);
  mkdirSync(path.join(root, 'src-tauri'));
  writeFileSync(path.join(root, 'package.json'), JSON.stringify({ version: '0.6.12' }));
  writeFileSync(path.join(root, 'src-tauri/tauri.conf.json'), JSON.stringify({ version: '0.6.12' }));
  writeFileSync(path.join(root, 'src-tauri/Cargo.toml'), '[package]\nversion = "0.6.12"\n');
  assert.equal(verifyVersion(root, 'v0.6.12'), '0.6.12');
  assert.throws(() => verifyVersion(root, 'v0.6.13'));
  assert.throws(() => verifyVersion(root, 'v0.6.12-beta'));
});
