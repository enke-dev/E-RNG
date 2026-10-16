import { env } from 'node:process';

import { defineConfig } from 'vite';
import litCss from 'vite-plugin-lit-css';
import { hmrPlugin, presets } from 'vite-plugin-web-components-hmr';

import pkg from '../package.json' with { type: 'json' };
import { litCssWatch } from './vite-watch-lit-css.plugin.js';

const host = env['TAURI_DEV_HOST'];

// set the application version as env var
env['APP_VERSION'] = pkg.version;

// https://vite.dev/config/
export default defineConfig({
  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    // 3. tell Vite to ignore watching `src-tauri`
    watch: { ignored: ['**/src-tauri/**'] },
  },
  // env variables with these prefixes are exposed to the frontend through `import.meta.env`
  envPrefix: ['APP_', 'VITE_', 'TAURI_ENV_*'],
  build: {
    // tauri uses WebKit on macOS
    target: 'safari13',
    minify: !env['TAURI_ENV_DEBUG'],
    sourcemap: !!env['TAURI_ENV_DEBUG'],
  },
  plugins: [
    litCss({ exclude: ['src/styles/**/*.css'] }),
    litCssWatch({ exclude: ['src/styles/**/*.css'] }),
    hmrPlugin({ include: ['./src/**/*.ts'], presets: [presets.lit] }),
  ],
});
