import { listen } from '@tauri-apps/api/event';

import { ask } from '../components/ask/ask.component.js';
import { api } from './api.utils.js';
import { goto } from './routing.utils.js';

/**
 * Layout files the system opened with the app (double click in Finder). Importing is up to the
 * settings, which may not be on screen yet: a file waits for them to say they are ready.
 */
let waiting: string | undefined;
let importer: ((path: string) => void) | undefined;

/** The settings take the files from now on, call the returned function when they are gone */
export function takeOpenedLayouts(handler: (path: string) => void) {
  importer = handler;
  if (waiting) {
    handler(waiting);
    waiting = undefined;
  }
  return () => {
    importer = undefined;
  };
}

const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** A file from outside is not stored on a whim: the user says yes first */
async function offer(path: string) {
  const yes = await ask(`Die Datei „${fileName(path)}“ als Layout importieren?`, {
    title: 'Layout importieren',
    okLabel: 'Importieren',
    cancelLabel: 'Abbrechen',
  });
  if (!yes) {
    return;
  }
  goto('/settings');
  if (importer) {
    importer(path);
  } else {
    waiting = path;
  }
}

/** Follows what the system opens with the app, also what came in before the window was ready */
export async function followOpenedFiles() {
  const drain = async () => {
    for (const path of await api.openedFiles()) {
      await offer(path);
    }
  };
  try {
    await listen('layout-file-opened', () => void drain());
    await drain();
  } catch {
    // no backend (the page in a plain browser), nothing can be opened
  }
}
