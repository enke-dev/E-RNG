import type { RouteConfig } from '@lit-labs/router';
import { html } from 'lit';

import type { Kind } from '../common/utils/api.utils.js';
import {
  activeContext,
  blockedAt,
  ensureContexts,
  missingFor,
} from '../common/utils/context.utils.js';
import { confirmLeave } from '../common/utils/leave.utils.js';
import { redirect } from '../common/utils/routing.utils.js';
import { currentRoute } from '../state.js';

/**
 * Customers, templates and a new invoice need a context that can issue invoices, the Vorgänge one
 * that has a seller with an address (`neededFor`). Without one the user is sent to the form of the
 * context (or of a new one) and comes back here when it is complete. The export gate in the backend
 * stays the real check, this only keeps the way clear.
 */
async function guard(): Promise<boolean> {
  try {
    await ensureContexts();
  } catch {
    // the views show what went wrong when they load
    return true;
  }
  const active = activeContext.get();
  if (active && missingFor(active, location.pathname).length === 0) {
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

/** Enters a route: the user may stay where they are, the view is loaded, then the context guard */
const entering = (load: () => Promise<unknown>, guarded = true) => {
  return async () => {
    if (!(await leave())) {
      return false;
    }
    await load();
    if (guarded && !(await guard())) {
      return false;
    }
    shown = location.pathname;
    return true;
  };
};

/** The Vorgänge show both kinds; a new document is `new-offer` or `new-invoice` */
const cases = (selected = '') => html`<erng-cases selected="${selected}"></erng-cases>`;

const settings = (section = '') => html`<erng-settings selected="${section}"></erng-settings>`;

/** Goes on to where a view moved: nothing is shown on the way */
const moved = (path: string) => () => {
  redirect(path);
  return Promise.resolve(false);
};

/** Where a document of a former list is now: found by its id in its Vorgang, a new one there too */
const formerly = (kind: Kind, id = '') =>
  id === 'new' ? `/cases/new-${kind}` : /^\d+$/.test(id) ? `/cases/${id}` : '/cases';

/** Views are loaded on first visit, the shell stays small */
export const routes: RouteConfig[] = [
  {
    path: '/cases',
    enter: entering(() => import('./cases/cases.view.js')),
    render: () => cases(),
  },
  {
    path: '/cases/:id',
    enter: entering(() => import('./cases/cases.view.js')),
    render: ({ id }) => cases(id),
  },
  {
    path: '/cases/:vorgang/:id',
    enter: entering(() => import('./cases/cases.view.js')),
    render: ({ vorgang, id }) => cases(`${vorgang}/${id}`),
  },
  // the start and the addresses of the lists before the Vorgänge (remembered, in old links)
  { path: '/', enter: moved('/cases') },
  { path: '/offers', enter: moved('/cases') },
  { path: '/offers/:id', enter: ({ id }) => moved(formerly('offer', id))() },
  { path: '/invoices/:id', enter: ({ id }) => moved(formerly('invoice', id))() },
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
