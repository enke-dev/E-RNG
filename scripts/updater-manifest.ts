/**
 * Writes the updater's `latest.json` for a release from its uploaded assets, once every platform is
 * built: tauri-action merges its own platform into the release's manifest without a lock, so parallel
 * builds could lose one. Keys are the ones tauri-action wrote, `{os}-{arch}` and `{os}-{arch}-{bundle}`.
 * URLs are the plain download links, not the API ones tauri-action writes (for private repos): the
 * API allows 60 requests an hour per address without a login, an office or VPN sharing one used
 * them up and the update failed with 403.
 *
 * `bun scripts/updater-manifest.ts <release id> <out file>`, with `GITHUB_REPOSITORY` and a token
 * for `gh`. Refuses a release that lacks a platform's bundle or signature.
 */
import { spawnSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { argv, env, stdout } from 'node:process';

interface Asset {
  id: number;
  name: string;
  browser_download_url: string;
}

/** What each platform ships for the updater, by the ending of its asset */
const PLATFORMS = [
  { ending: '_aarch64.app.tar.gz', keys: ['darwin-aarch64', 'darwin-aarch64-app'] },
  { ending: '_x64-setup.exe', keys: ['windows-x86_64', 'windows-x86_64-nsis'] },
];

const NOTES = 'See the assets to download this version and install.';

const gh = (args: string[]) => {
  const result = spawnSync('gh', args, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  if (result.status !== 0) {
    throw new Error(`gh ${args.join(' ')} failed: ${result.stderr}`);
  }
  return result.stdout;
};

const [id, outFile] = argv.slice(2);
const repo = env['GITHUB_REPOSITORY'];
if (!id || !outFile || !repo) {
  throw new Error('usage: updater-manifest.ts <release id> <out file>, with GITHUB_REPOSITORY');
}

const release = JSON.parse(gh(['api', `repos/${repo}/releases/${id}`])) as {
  tag_name: string;
  assets: Asset[];
};
const download = (asset: Asset) =>
  gh([
    'api',
    '-H',
    'Accept: application/octet-stream',
    `repos/${repo}/releases/assets/${asset.id}`,
  ]);

const platforms = Object.fromEntries(
  PLATFORMS.flatMap(({ ending, keys }) => {
    const bundle = release.assets.find(asset => asset.name.endsWith(ending));
    const signature = release.assets.find(asset => asset.name.endsWith(`${ending}.sig`));
    if (!bundle || !signature) {
      throw new Error(`${release.tag_name} has no ${ending} with its signature`);
    }
    const entry = { signature: download(signature).trim(), url: bundle.browser_download_url };
    return keys.map(key => [key, entry]);
  })
);

writeFileSync(
  outFile,
  `${JSON.stringify(
    {
      version: release.tag_name.replace(/^v/, ''),
      notes: NOTES,
      pub_date: new Date().toISOString(),
      platforms,
    },
    null,
    2
  )}\n`
);
stdout.write(`${outFile}: ${Object.keys(platforms).join(', ')}\n`);
