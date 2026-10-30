/**
 * Fetches the tools the app ships next to its binary into `src-tauri/sidecars/`:
 *
 * - `mustang.jar`  attaches the XML (Factur-X) and validates PDF/A, XMP and the XML rules; what the
 *                  app never runs (rendering with FOP and Batik, test libraries) is taken out
 * - `jre/`         a minimal Java runtime for Mustang, linked from a Temurin JDK
 *
 * Versions and checksums are pinned in `sidecars.json`, a download that does not match is refused.
 * `--stub` only creates empty placeholders, enough for `tauri-build` (lint, typecheck) to pass.
 */
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { arch, platform } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { argv, stdout } from 'node:process';
import { fileURLToPath } from 'node:url';

interface Download {
  version: string;
  url: string;
  sha256: string;
}
interface Pins {
  /** `drop`: packages the app never runs (only `combine` and `validate`), as paths in the jar */
  mustang: Download & { drop: string[] };
  jdk: Download & { modules: string };
}

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pinsFile = join(root, 'scripts', 'sidecars.json');
const cache = join(root, '.cache', 'sidecars');
const out = join(root, 'src-tauri', 'sidecars');
const stamp = join(out, '.pins');

const pins = JSON.parse(readFileSync(pinsFile, 'utf8')) as Pins;

const log = (message: string) => stdout.write(`${message}\n`);

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

/**
 * The jar without the packages in `drop`. Service registrations that name a class that is gone
 * are taken out too, the service loader would fail on them. Whether the rest still works shows the
 * validator: the reports of both jars are the same for valid and broken files.
 */
function slim(jar: string, target: string, drop: string[]) {
  const dir = join(cache, 'mustang');
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  run('unzip', ['-q', jar, '-d', dir]);
  drop.forEach(path => rmSync(join(dir, path), { recursive: true, force: true }));
  const services = join(dir, 'META-INF', 'services');
  const present = (line: string) => {
    const name = line.split('#')[0]?.trim() ?? '';
    return name === '' || existsSync(join(dir, `${name.replaceAll('.', '/')}.class`));
  };
  readdirSync(services).forEach(name => {
    const file = join(services, name);
    const lines = readFileSync(file, 'utf8').split('\n').filter(present);
    if (lines.some(line => line.split('#')[0]?.trim())) {
      writeFileSync(file, lines.join('\n'));
    } else {
      rmSync(file);
    }
  });
  run('zip', ['-q', '-r', '-9', '-X', target, '.'], dir);
}

function stub() {
  rmSync(out, { recursive: true, force: true });
  mkdirSync(join(out, 'jre'), { recursive: true });
  writeFileSync(join(out, 'mustang.jar'), '');
  writeFileSync(join(out, 'jre', 'release'), '');
  writeFileSync(stamp, 'stub');
}

async function fetchAll() {
  if (platform() !== 'darwin' || arch() !== 'arm64') {
    throw new Error('the pinned sidecars are for macOS arm64 only');
  }
  const wanted = createHash('sha256').update(readFileSync(pinsFile)).digest('hex');
  if (existsSync(stamp) && readFileSync(stamp, 'utf8') === wanted) {
    log('sidecars up to date');
    return;
  }

  const [mustangJar, jdkArchive] = await Promise.all([
    download(pins.mustang, 'mustang.jar'),
    download(pins.jdk, 'jdk.tar.gz'),
  ]);

  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });

  // mustang
  slim(mustangJar, join(out, 'mustang.jar'), pins.mustang.drop);

  // jre, linked to the modules mustang needs (verified by running the validator against it)
  const jdkDir = join(cache, 'jdk');
  rmSync(jdkDir, { recursive: true, force: true });
  mkdirSync(jdkDir, { recursive: true });
  run('tar', ['xzf', jdkArchive, '-C', jdkDir]);
  const home = join(jdkDir, `jdk-${pins.jdk.version}`, 'Contents', 'Home');
  run(join(home, 'bin', 'jlink'), [
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

  // jlink keeps the JDK's read-only modes, which makes tauri's resource copy fail on the second build
  run('chmod', ['-R', 'u+w', join(out, 'jre')]);

  writeFileSync(stamp, wanted);
  log('sidecars ready');
}

await (argv.includes('--stub') ? Promise.resolve(stub()) : fetchAll());
