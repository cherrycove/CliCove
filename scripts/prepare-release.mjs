import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const targets = ['aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-pc-windows-msvc'];
export function verifyVersion(root, tag) {
  const pkg = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8'));
  const config = JSON.parse(readFileSync(path.join(root, 'src-tauri/tauri.conf.json'), 'utf8'));
  const cargo = readFileSync(path.join(root, 'src-tauri/Cargo.toml'), 'utf8').match(/^version = "([^"]+)"/m)?.[1];
  if (!/^\d+\.\d+\.\d+$/.test(pkg.version) || tag !== `v${pkg.version}` || config.version !== pkg.version || cargo !== pkg.version) {
    throw new Error('Release tag and package.json, Cargo.toml, tauri.conf.json versions must match (vX.Y.Z).');
  }
  return pkg.version;
}
function filesIn(root) {
  return readdirSync(root, { withFileTypes: true }).flatMap((item) => {
    const name = path.join(root, item.name);
    return item.isDirectory() ? filesIn(name) : [name];
  });
}
export function collectArtifacts(root, target, output, version) {
  if (!targets.includes(target)) throw new Error(`Unsupported target: ${target}`);
  mkdirSync(output, { recursive: true });
  const files = filesIn(root);
  const suffixes = target.includes('apple') ? ['.dmg', '.app.tar.gz', '.app.tar.gz.sig'] : ['.msi', '.msi.sig', '-setup.exe', '-setup.exe.sig'];
  for (const suffix of suffixes) {
    const matches = files.filter((file) => file.endsWith(suffix));
    if (matches.length !== 1) throw new Error(`Expected exactly one ${suffix} for ${target}, found ${matches.length}`);
    const name = `CliCove_${version}_${target}${suffix}`;
    copyFileSync(matches[0], path.join(output, name));
  }
}
export function generateManifest(root, version, repository, notes, publishedAt = new Date().toISOString()) {
  const platforms = {};
  const platformFiles = {
    'darwin-aarch64': `CliCove_${version}_aarch64-apple-darwin.app.tar.gz`,
    'darwin-x86_64': `CliCove_${version}_x86_64-apple-darwin.app.tar.gz`,
    'windows-x86_64-msi': `CliCove_${version}_x86_64-pc-windows-msvc.msi`,
    'windows-x86_64-nsis': `CliCove_${version}_x86_64-pc-windows-msvc-setup.exe`,
  };
  for (const [platform, name] of Object.entries(platformFiles)) {
    if (!existsSync(path.join(root, name))) throw new Error(`Missing update package: ${name}`);
    const signature = readFileSync(path.join(root, `${name}.sig`), 'utf8').trim();
    if (!signature || !Buffer.from(signature, 'base64').toString().startsWith('untrusted comment:')) throw new Error(`Invalid signature file: ${name}.sig`);
    platforms[platform] = { signature, url: `https://github.com/${repository}/releases/download/v${version}/${name}` };
  }
  // Fallback for clients that cannot determine their original installer type.
  platforms['windows-x86_64'] = platforms['windows-x86_64-msi'];
  return { version, notes, pub_date: publishedAt, platforms };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [mode, ...args] = process.argv.slice(2);
  if (mode === 'verify') {
    console.log(verifyVersion(process.cwd(), args[0]));
  } else if (mode === 'collect') {
    collectArtifacts(args[0], args[1], args[2], args[3]);
  } else if (mode === 'manifest') {
    const [root, version, repository, notesFile] = args;
    const manifest = generateManifest(root, version, repository, readFileSync(notesFile, 'utf8'));
    writeFileSync(path.join(root, 'latest.json'), JSON.stringify(manifest, null, 2) + '\n');
    const sums = filesIn(root).filter((file) => !file.endsWith('SHA256SUMS')).sort().map((file) => `${createHash('sha256').update(readFileSync(file)).digest('hex')}  ${path.basename(file)}`).join('\n');
    writeFileSync(path.join(root, 'SHA256SUMS'), sums + '\n');
  } else throw new Error('Usage: prepare-release.mjs verify TAG | collect BUNDLE TARGET OUTPUT VERSION | manifest ASSETS VERSION REPO NOTES');
}
