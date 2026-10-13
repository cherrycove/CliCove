import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// The app and platform assets share this one drawing. Edit the SVG, then regenerate.
const root = dirname(dirname(fileURLToPath(import.meta.url)));
const source = readFileSync(join(root, 'src/assets/clicove-mark.svg'), 'utf8');
const body = source.slice(source.indexOf('  <g id="tile">'), source.lastIndexOf('</svg>')).trim();
const symbol = source.match(/<g id="symbol">([\s\S]*?)<\/g>/)?.[1];
if (!symbol) throw new Error('The brand master must contain a symbol group.');
const svg = (size, viewBox, content) => `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="${viewBox}">\n${content}\n</svg>\n`;
// Windows/Linux use the mark directly. macOS needs transparent canvas margins,
// but the tile itself must stay opaque so Tahoe does not add a second backing.
const icon = svg(512, '0 0 72 72', body);
const macIcon = svg(512, '0 0 512 512', `<g transform="translate(50 50) scale(${412 / 72})">${body}</g>`);
// macOS uses the alpha channel of this asset as a system-colored template.
const mono = svg(18, '12 12 48 48', symbol.replace(/fill="#[0-9a-f]+"/g, 'fill="currentColor"'));
const brandDir = join(root, 'docs/brand/clicove');
const iconsDir = join(root, 'src-tauri/icons');
mkdirSync(brandDir, { recursive: true });
mkdirSync(iconsDir, { recursive: true });

const sizes = [18, 24, 36, 48, 64, 96];
const samples = sizes.map((size, index) => `<g transform="translate(${70 + index * 132} ${662 - size}) scale(${size / 72})">${body}</g><text x="${70 + index * 132}" y="700" font-size="16">${size}px</text>`).join('\n');
const preview = svg(960, '0 0 960 960', `<rect width="960" height="960" fill="#f4f3ed"/>
<g font-family="system-ui, sans-serif" fill="#232722">
<text x="64" y="78" font-size="34" font-weight="700">CliCove</text>
<text x="64" y="112" font-size="18" fill="#66705d">One mark, inside and outside the app.</text>
<g transform="translate(52 150) scale(.82)">${macIcon.replace(/<svg[^>]*>|<\/svg>/g, '')}</g>
<g transform="translate(574 250) scale(2.5)">${body}</g>
<text x="570" y="474" font-size="18">Windows / interface</text>
<text x="88" y="560" font-size="18">macOS · transparent canvas, no backing</text>
${samples}
<text x="64" y="785" font-size="18">Aligned C segments · centered prompt · fixed corner join</text>
<rect x="64" y="824" width="136" height="44" fill="#305ae8"/>
<rect x="200" y="824" width="136" height="44" fill="#f2cb4b"/>
<rect x="336" y="824" width="136" height="44" fill="#e9644b"/>
<rect x="472" y="824" width="136" height="44" fill="#fffef9"/>
</g>`);

const temp = mkdtempSync(join(tmpdir(), 'clicove-brand-render-'));
const tauri = (...args) => execFileSync('npm', ['run', 'tauri', '--', 'icon', ...args], { cwd: root, stdio: 'inherit' });
try {
  writeFileSync(join(temp, 'icon.svg'), icon);
  writeFileSync(join(temp, 'icon-macos.svg'), macIcon);
  writeFileSync(join(temp, 'tray.svg'), mono);
  writeFileSync(join(temp, 'preview.svg'), preview);
  tauri(join(temp, 'icon.svg'), '--output', join(temp, 'desktop'));
  for (const name of ['32x32.png', '128x128.png', '128x128@2x.png', 'icon.ico']) {
    copyFileSync(join(temp, 'desktop', name), join(iconsDir, name));
  }
  tauri(join(temp, 'icon-macos.svg'), '--output', join(temp, 'macos'));
  copyFileSync(join(temp, 'macos/icon.icns'), join(iconsDir, 'icon.icns'));
  tauri(join(temp, 'tray.svg'), '--output', join(temp, 'tray'), '--png', '36');
  copyFileSync(join(temp, 'tray/36x36.png'), join(iconsDir, 'tray-template.png'));
  tauri(join(temp, 'preview.svg'), '--output', join(temp, 'preview'), '--png', '960');
  copyFileSync(join(temp, 'preview/960x960.png'), join(brandDir, 'clicove-preview.png'));
} finally {
  rmSync(temp, { recursive: true, force: true });
}
