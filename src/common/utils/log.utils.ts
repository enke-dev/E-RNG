import { attachConsole } from '@tauri-apps/plugin-log';

/** Forwards the webview console to the tauri log, so packaged failures can be read */
export async function initLog() {
  await attachConsole();
}
