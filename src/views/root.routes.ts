import type { RouteConfig } from '@lit-labs/router';
import { html } from 'lit';

import type { Kind } from '../common/utils/api.utils.js';
import { activeContext, blockedAt, ensureContexts } from '../common/utils/context.utils.js';
import { confirmLeave } from '../common/utils/leave.utils.js';
import { redirect } from '../common/utils/routing.utils.js';
import { currentRoute } from '../state.js';

/**
 * Invoices, customers and templates need a context that can issue invoices, offers one that has a
 * seller with an address. Without one the user is sent to the form of the context (or of a new one)
 * and comes back here when it is complete. The export gate in the backend stays the real check, this
 * only keeps the way clear.
 */
async function guard(kind: Kind): Promise<boolean> {
  try {
    await ensureContexts();
  } catch {
    // the views show what went wrong when they load
    return true;
  }
  const active = activeContext.get();
  if (active && (kind === 'offer' ? active.missing_for_offers : active.missing).length === 0) {
    return true;
  }
  blockedAt.set(location.pathname);
  redirect(active ? `/settings/context-${active.id}-company` : '/settings/context-new-company');
  return false;
}

/** The address of the view on show, where to go back to if the user decides to stay */
let shown = location.pathname;

/**
 * Leaving a view with unsaved edits is the user's call. The address has changed already when a route
 * is entered (link, back button), staying puts it back.
 */
async function leave(): Promise<boolean> {
  const target = location.pathname;
  if (target === shown || (await confirmLeave(target))) {
    return true;
  }
  history.replaceState({}, '', shown);
  currentRoute.set(shown);
  return false;
}

/** Enters a route: the user may stay where they are, the view is loaded, then the context guard of its kind */
const entering = (load: () => Promise<unknown>, guarded: Kind | false = 'invoice') => {
  return async () => {
    if (!(await leave())) {
      return false;
    }
    await load();
    if (guarded && !(await guard(guarded))) {
      return false;
    }
    shown = location.pathname;
    return true;
  };
};

/** Both address forms show the same element: it stays and only its selection changes */
const invoices = (selected = '') => html`<erng-invoices selected="${selected}"></erng-invoices>`;

/** A literal of its own: the offers are another element, not the invoice list with another kind */
const offers = (selected = '') =>
  html`<erng-invoices kind="offer" selected="${selected}"></erng-invoices>`;

const settings = (section = '') => html`<erng-settings selected="${section}"></erng-settings>`;

/** Views are loaded on first visit, the shell stays small */
export const routes: RouteConfig[] = [
  {
    path: '/offers',
    enter: entering(() => import('./invoices/invoices.view.js'), 'offer'),
    render: () => offers(),
  },
  {
    path: '/offers/:id',
    enter: entering(() => import('./invoices/invoices.view.js'), 'offer'),
    render: ({ id }) => offers(id),
  },
  {
    path: '/',
    enter: entering(() => import('./invoices/invoices.view.js')),
    render: () => invoices(),
  },
  {
    path: '/invoices/:id',
    enter: entering(() => import('./invoices/invoices.view.js')),
    render: ({ id }) => invoices(id),
  },
  {
    path: '/customers',
    enter: entering(() => import('./customers/customers.view.js')),
    render: () => html`<erng-customers></erng-customers>`,
  },
  {
    path: '/templates',
    enter: entering(() => import('./templates/templates.view.js')),
    render: () => html`<erng-templates></erng-templates>`,
  },
  {
    path: '/settings',
    enter: entering(() => import('./settings/settings.view.js'), false),
    render: () => settings(),
  },
  {
    path: '/settings/:section',
    enter: entering(() => import('./settings/settings.view.js'), false),
    render: ({ section }) => settings(section),
  },
];
