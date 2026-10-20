import 'element-internals-polyfill';
import 'urlpattern-polyfill';

import { registerIcons } from './common/utils/icon.utils.js';
import { initLog } from './common/utils/log.utils.js';
import { restoreRoute } from './common/utils/memory.utils.js';
import { errors, followSystemScheme } from './state.js';

followSystemScheme();
registerIcons();
restoreRoute();

/**
 * First set up logging, then load the app
 * @todo use global await once safari supports it properly
 * @see https://webkit.org/b/242740
 */
// outside of tauri (`vite dev` in a browser) a mock stands in for the rust side
const ready =
  import.meta.env.DEV && !('__TAURI_INTERNALS__' in globalThis)
    ? import('./common/utils/mock.utils.js').then(({ installMocks }) => installMocks())
    : Promise.resolve();

ready
  .then(() => initLog())
  .catch((error: unknown) => {
    // a rejected init would leave the boot spinner up for ever with nothing rendered
    errors.set([error instanceof Error ? error.message : String(error)]);
  })
  .then(() => import('./views/root.view.js'));
