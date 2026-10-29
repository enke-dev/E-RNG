import { redirect } from './routing.utils.js';

/**
 * What the app remembers between runs (where the dividers are, the view that was open). It is a
 * convenience of this machine, not a setting: the storage may be gone or refuse, nothing depends on it.
 */
const PREFIX = 'erng:';

export function recall(key: string): number | undefined {
  try {
    const value = Number(localStorage.getItem(PREFIX + key));
    return Number.isFinite(value) && value > 0 ? value : undefined;
  } catch {
    return undefined;
  }
}

export function remember(key: string, value: number) {
  try {
    localStorage.setItem(PREFIX + key, String(value));
  } catch {
    // not remembered, nothing breaks
  }
}

/** A short text, e.g. an id */
export function recallText(key: string): string | undefined {
  try {
    return localStorage.getItem(PREFIX + key) ?? undefined;
  } catch {
    return undefined;
  }
}

export function rememberText(key: string, value: string) {
  try {
    localStorage.setItem(PREFIX + key, value);
  } catch {
    // not remembered, nothing breaks
  }
}

const ROUTE = `${PREFIX}route`;

/** Views worth coming back to; a blank invoice or offer is not stored yet, there is nothing to come back to */
const restorable = (path: string) =>
  !/^\/(invoices|offers)\//.test(path) || /^\/(invoices|offers)\/\d+$/.test(path);

/** Notes the view that is open, call it on every navigation */
export function rememberRoute(path: string) {
  try {
    if (restorable(path)) {
      localStorage.setItem(ROUTE, path);
    }
  } catch {
    // not remembered, nothing breaks
  }
}

/** Opens the view that was open last, if the app was started without a place of its own */
export function restoreRoute() {
  try {
    const path = localStorage.getItem(ROUTE);
    if (location.pathname === '/' && path && path !== '/' && restorable(path)) {
      redirect(path);
    }
  } catch {
    // start where the app starts
  }
}
