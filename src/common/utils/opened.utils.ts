import { listen } from '@tauri-apps/api/event';

import { ask } from '../components/ask/ask.component.js';
import { api } from './api.utils.js';
import { goto } from './routing.utils.js';

/**
 * Layout and backup files the system opened with the app (double click in Finder). Importing or
 * restoring is up to the settings, which may not be on screen yet: a file waits for them to say
 * they are ready.
 */
type Kind = 'layout' | 'backup';

const waiting = new Map<Kind, string>();
const takers = new Map<Kind, (path: string) => void>();

const kindOf = (path: string): Kind => (/\.erng-backup$/i.test(path) ? 'backup' : 'layout');

/** The settings take the files of a kind from now on, call the returned function when they are gone */
export function takeOpened(kind: Kind, handler: (path: string) => void) {
  takers.set(kind, handler);
  const path = waiting.get(kind);
  if (path) {
    waiting.delete(kind);
    handler(path);
  }
  return () => {
    if (takers.get(kind) === handler) {
      takers.delete(kind);
    }
  };
}

/** The name of a file without its folders, on either system */
export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

const handOver = (kind: Kind, path: string) => {
  const taker = takers.get(kind);
  if (taker) {
    taker(path);
  } else {
    waiting.set(kind, path);
  }
};

/**
 * A file from outside is not stored on a whim: the user says yes first. A layout is asked for here,
 * a backup on its page, where restoring asks anyway.
 */
async function offer(path: string) {
  if (kindOf(path) === 'backup') {
    goto('/settings/backup');
    handOver('backup', path);
    return;
  }
  const yes = await ask(`Die Datei „${fileName(path)}“ als Layout importieren?`, {
    title: 'Layout importieren',
    okLabel: 'Importieren',
    cancelLabel: 'Abbrechen',
  });
  if (!yes) {
    return;
  }
  goto('/settings');
  handOver('layout', path);
}

/** Follows what the system opens with the app, also what came in before the window was ready */
export async function followOpenedFiles() {
  const drain = async () => {
    for (const path of await api.openedFiles()) {
      await offer(path);
    }
  };
  try {
    await listen('file-opened', () => void drain());
    await drain();
  } catch {
    // no backend (the page in a plain browser), nothing can be opened
  }
}
