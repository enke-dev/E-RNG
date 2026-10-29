import { computed, signal } from '@lit-labs/signals';

import type { Context } from './api.utils.js';
import { api } from './api.utils.js';
import { confirmLeave } from './leave.utils.js';
import { recallText, rememberText } from './memory.utils.js';
import { goto } from './routing.utils.js';

/**
 * The contexts (issuers) and which one is active. Which one is active is a convenience of this
 * machine, like the last view: it is remembered here and never stored with the data.
 */
const ACTIVE = 'context';

/** Every context, archived ones included, `undefined` until they were loaded */
export const contexts = signal<Context[] | undefined>(undefined);

const chosen = signal<number | undefined>(Number(recallText(ACTIVE)) || undefined);

/** The contexts the switch offers */
export const available = computed(() => (contexts.get() ?? []).filter(c => !c.archived_at));

/** The chosen context if it still exists, else the first one that is in use; none on a fresh start */
export const activeContext = computed<Context | undefined>(() => {
  const list = available.get();
  return list.find(c => c.id === chosen.get()) ?? list[0];
});

/** Whether the active context can issue invoices: it is there and nothing blocking is missing */
export const ready = computed(() => {
  const active = activeContext.get();
  return active !== undefined && active.missing.length === 0;
});

/** Whether the active context can issue offers, which asks for less: a seller with an address */
export const readyForOffers = computed(() => {
  const active = activeContext.get();
  return active !== undefined && active.missing_for_offers.length === 0;
});

/** Where the user wanted to go when the guard sent them to a context's form, to go back there */
export const blockedAt = signal<string | undefined>(undefined);

export async function loadContexts(): Promise<Context[]> {
  const list = await api.contextsList();
  contexts.set(list);
  return list;
}

/** The contexts once they are loaded, loads them on the first call */
export async function ensureContexts(): Promise<Context[]> {
  return contexts.get() ?? loadContexts();
}

export function activate(id: number) {
  chosen.set(id);
  rememberText(ACTIVE, String(id));
}

/** The seller name labels a context everywhere, a new one has none yet */
export const label = (context: Pick<Context, 'settings'> | undefined) =>
  context?.settings.seller.name.trim() || 'Unbenannt';

export const contextById = (id: number | null | undefined) =>
  id == null ? undefined : (contexts.get() ?? []).find(c => c.id === id);

/**
 * The contexts an entry is for, to show as tags: the one it is scoped to, or all of them when it is
 * shared. Nothing with one context, there is nothing to tell apart.
 */
export function contextsOf(scope: number | null): Context[] {
  const list = available.get();
  if (list.length < 2) {
    return [];
  }
  const own = contextById(scope);
  return scope === null ? list : own ? [own] : [];
}

/** Tints the UI with the brand color: one class re-derives every brand token of the UI library */
export function applyBrand(brand: string | undefined) {
  const classes = document.documentElement.classList;
  [...classes].filter(c => c.startsWith('wa-brand-')).forEach(c => classes.remove(c));
  classes.add(`wa-brand-${brand || 'blue'}`);
}

/**
 * Makes another context the active one. The views of the old context go away with the switch, so
 * unsaved edits are the user's call. The settings stay where they are (the same page of the other
 * context, a layout stays a layout) unless the new context cannot issue invoices yet, then its form
 * opens; an invoice or offer stays with the context it was made under, so the list of the new one
 * opens instead. Returns whether the switch happened.
 */
export async function switchContext(id: number): Promise<boolean> {
  const next = contextById(id);
  const path = location.pathname;
  // offers ask less of a context than invoices do
  const missing = path.startsWith('/offers') ? next?.missing_for_offers : next?.missing;
  const incomplete = (missing?.length ?? 0) > 0;
  // in the settings the page stays: the same page of the other context, a layout stays a layout
  const page = /^\/settings\/context-(?:new|\d+)-(company|bank|offers|invoices)$/.exec(path)?.[1];
  const inSettings = path.startsWith('/settings');
  const target = incomplete
    ? `/settings/context-${id}-company`
    : page
      ? `/settings/context-${id}-${page}`
      : inSettings
        ? path
        : path.startsWith('/invoices/')
          ? '/'
          : path.startsWith('/offers/')
            ? '/offers'
            : path;
  if (!(await confirmLeave(target))) {
    return false;
  }
  activate(id);
  if (target !== path) {
    goto(target);
  }
  return true;
}
