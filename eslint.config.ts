import { cwd } from 'node:process';

import config, { setTsConfigRootDir } from '@enke.dev/lint/eslint.config.js';
import { defineConfig } from 'eslint/config';

export default defineConfig([
  config,
  setTsConfigRootDir(cwd()),
  { ignores: ['dist/**', 'src-tauri/target/**', 'src-tauri/gen/**'] },
]);
