/**
 * Generates the app icons (`src-tauri/icons/`) with `tauri icon`.
 *
 * The design is `src/assets/icon-default.icon` (Icon Composer), `icon-default.png` is its export.
 * The export fills the whole 1024 px canvas, while a macOS icon in an `.icns` is expected to sit on
 * an 824 px body with room around it for the shadow, otherwise it looks too big in the Dock next
 * to other apps. The export is therefore placed on a transparent canvas first.
 *
 * Tauri's bundler could compile the `.icon` itself for macOS 26, that needs a working `actool`
 * (Xcode 26) and is not wired up yet.
 */
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { stdout } from 'node:process';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const exported = join(root, 'src', 'assets', 'icon-default.png');
const output = join(root, '.cache', 'icon-source.svg');

const canvas = 1024;
const body = 824;
const margin = (canvas - body) / 2;

const png = readFileSync(exported).toString('base64');
const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${canvas}" height="${canvas}" viewBox="0 0 ${canvas} ${canvas}">
  <image x="${margin}" y="${margin}" width="${body}" height="${body}" href="data:image/png;base64,${png}"/>
</svg>
`;

mkdirSync(dirname(output), { recursive: true });
writeFileSync(output, svg);

const result = spawnSync('bunx', ['tauri', 'icon', output], { cwd: root, stdio: 'inherit' });
if (result.status !== 0) {
  throw new Error('tauri icon failed');
}
stdout.write('icons ready\n');
