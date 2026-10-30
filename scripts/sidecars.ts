/**
 * Fetches the tools the app ships next to its binary into `src-tauri/sidecars/`:
 *
 * - `mustang.jar`  attaches the XML (Factur-X) and validates PDF/A, XMP and the XML rules; what the
 *                  app never runs (rendering with FOP and Batik, test libraries) is taken out
 * - `jre/`         a minimal Java runtime for Mustang, linked from a Temurin JDK
 *
 * The tools are those of the target being built: `--target <triple>`, else the one Tauri builds for
 * (`TAURI_ENV_TARGET_TRIPLE`), else the host. The runtime is linked by the host's `jlink` from the
 * target's modules, so Windows can be built on Linux or macOS; both JDKs are the same pinned version,
 * which jlink requires.
 *
 * Versions and checksums are pinned in `sidecars.json`, a download that does not match is refused.
 * `--stub` only creates empty placeholders, enough for `tauri-build` (lint, typecheck) to pass.
 */
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  chmodSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { arch, platform } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { argv, env, stdout } from 'node:process';
import { fileURLToPath } from 'node:url';

import { unzipSync, zipSync } from 'fflate';

interface Download {
  url: string;
  sha256: string;
}
interface Pins {
  /** `drop`: packages the app never runs (only `combine` and `validate`), as paths in the jar */
  mustang: Download & { version: string; drop: string[] };
  /** One JDK per target, and per host that builds: the host's `jlink`, the target's modules */
  jdk: { version: string; modules: string; targets: Record<string, Download> };
}

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pinsFile = join(root, 'scripts', 'sidecars.json');
const cache = join(root, '.cache', 'sidecars');
const out = join(root, 'src-tauri', 'sidecars');
const stamp = join(out, '.pins');

const pins = JSON.parse(readFileSync(pinsFile, 'utf8')) as Pins;

const log = (message: string) => stdout.write(`${message}\n`);

const HOSTS: Record<string, string> = {
  'darwin-arm64': 'aarch64-apple-darwin',
  'win32-x64': 'x86_64-pc-windows-msvc',
  'linux-x64': 'x86_64-unknown-linux-gnu',
};

const option = (name: string) => {
  const index = argv.indexOf(name);
  return index === -1 ? undefined : argv[index + 1];
};

const windows = (triple: string) => triple.includes('windows');

function run(command: string, args: string[], cwd?: string) {
  const result = spawnSync(command, args, { cwd, stdio: 'inherit' });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} failed`);
  }
}

async function download({ url, sha256 }: Download, name: string): Promise<string> {
  const target = join(cache, name);
  const hash = (file: string) => createHash('sha256').update(readFileSync(file)).digest('hex');
  if (existsSync(target) && hash(target) === sha256) {
    return target;
  }

  log(`downloading ${url}`);
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`${url}: ${response.status}`);
  }
  mkdirSync(cache, { recursive: true });
  writeFileSync(target, Buffer.from(await response.arrayBuffer()));
  const actual = hash(target);
  if (actual !== sha256) {
    rmSync(target);
    throw new Error(`${name}: checksum ${actual} does not match the pinned ${sha256}`);
  }
  return target;
}

/** Unpacks a zip (Windows JDK) itself, a tarball with `tar`, which every host has */
function unpack(archive: string, dir: string) {
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  if (!archive.endsWith('.zip')) {
    run('tar', ['xzf', archive, '-C', dir]);
    return;
  }
  Object.entries(unzipSync(readFileSync(archive)))
    .filter(([name]) => !name.endsWith('/'))
    .forEach(([name, data]) => {
      const file = join(dir, name);
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(file, data);
    });
}

/** The JDK of a triple, unpacked; its home is inside the bundle on macOS */
async function jdk(triple: string) {
  const pinned = pins.jdk.targets[triple];
  if (!pinned) {
    throw new Error(`no JDK pinned for ${triple} in sidecars.json`);
  }
  const archive = await download(
    pinned,
    `jdk-${triple}${pinned.url.endsWith('.zip') ? '.zip' : '.tar.gz'}`
  );
  const dir = join(cache, `jdk-${triple}`);
  unpack(archive, dir);
  const home = join(dir, `jdk-${pins.jdk.version}`);
  return triple.includes('apple') ? join(home, 'Contents', 'Home') : home;
}

/**
 * The jar without the packages in `drop`. Service registrations that name a class that is gone
 * are taken out too, the service loader would fail on them. Whether the rest still works shows the
 * validator: the reports of both jars are the same for valid and broken files.
 */
function slim(jar: string, target: string, drop: string[]) {
  const entries = unzipSync(readFileSync(jar));
  const dropped = (name: string) => drop.some(path => name.startsWith(`${path}/`));
  const present = (line: string) => {
    const name = line.split('#')[0]?.trim() ?? '';
    const file = `${name.replaceAll('.', '/')}.class`;
    return name === '' || (file in entries && !dropped(file));
  };
  const kept = Object.fromEntries(
    Object.entries(entries)
      .filter(([name]) => !name.endsWith('/') && !dropped(name))
      .map(([name, data]): [string, Uint8Array | null] => {
        if (!name.startsWith('META-INF/services/')) {
          return [name, data];
        }
        const lines = new TextDecoder().decode(data).split('\n').filter(present);
        return lines.some(line => line.split('#')[0]?.trim())
          ? [name, new TextEncoder().encode(lines.join('\n'))]
          : [name, null];
      })
      .filter((entry): entry is [string, Uint8Array] => entry[1] !== null)
  );
  // a fixed time, so the same pins make the same jar
  writeFileSync(target, zipSync(kept, { level: 9, mtime: new Date(1980, 0, 2) }));
}

/** jlink keeps the JDK's read-only modes, which makes tauri's resource copy fail on the second build */
function writable(dir: string) {
  readdirSync(dir, { recursive: true, encoding: 'utf8' })
    .map(name => join(dir, name))
    .forEach(path => chmodSync(path, statSync(path).mode | 0o200));
}

function stub() {
  rmSync(out, { recursive: true, force: true });
  mkdirSync(join(out, 'jre'), { recursive: true });
  writeFileSync(join(out, 'mustang.jar'), '');
  writeFileSync(join(out, 'jre', 'release'), '');
  writeFileSync(stamp, 'stub');
}

async function fetchAll() {
  const host = HOSTS[`${platform()}-${arch()}`];
  if (!host) {
    throw new Error(`no sidecars for building on ${platform()} ${arch()}`);
  }
  const target = option('--target') ?? env['TAURI_ENV_TARGET_TRIPLE'] ?? host;
  const wanted = createHash('sha256').update(readFileSync(pinsFile)).update(target).digest('hex');
  if (existsSync(stamp) && readFileSync(stamp, 'utf8') === wanted) {
    log(`sidecars for ${target} up to date`);
    return;
  }

  const mustangJar = await download(pins.mustang, 'mustang.jar');
  const hostHome = await jdk(host);
  const targetHome = target === host ? hostHome : await jdk(target);

  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });

  // mustang
  slim(mustangJar, join(out, 'mustang.jar'), pins.mustang.drop);

  // jre, linked to the modules mustang needs (verified by running the validator against it)
  run(join(hostHome, 'bin', windows(host) ? 'jlink.exe' : 'jlink'), [
    '--module-path',
    join(targetHome, 'jmods'),
    '--add-modules',
    pins.jdk.modules,
    '--strip-debug',
    '--no-header-files',
    '--no-man-pages',
    '--compress',
    'zip-9',
    '--output',
    join(out, 'jre'),
  ]);
  writable(join(out, 'jre'));

  writeFileSync(stamp, wanted);
  log(`sidecars for ${target} ready`);
}

await (argv.includes('--stub') ? Promise.resolve(stub()) : fetchAll());
