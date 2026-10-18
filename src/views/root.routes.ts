import type { RouteConfig } from '@lit-labs/router';
import { html } from 'lit';

/** Both address forms show the same element: it stays and only its selection changes */
const invoices = (selected = '') => html`<erng-invoices selected="${selected}"></erng-invoices>`;

/** Views are loaded on first visit, the shell stays small */
export const routes: RouteConfig[] = [
  {
    path: '/',
    enter: async () => {
      await import('./invoices/invoices.view.js');
      return true;
    },
    render: () => invoices(),
  },
  {
    path: '/invoices/:id',
    enter: async () => {
      await import('./invoices/invoices.view.js');
      return true;
    },
    render: ({ id }) => invoices(id),
  },
  {
    path: '/customers',
    enter: async () => {
      await import('./customers/customers.view.js');
      return true;
    },
    render: () => html`<erng-customers></erng-customers>`,
  },
  {
    path: '/templates',
    enter: async () => {
      await import('./templates/templates.view.js');
      return true;
    },
    render: () => html`<erng-templates></erng-templates>`,
  },
  {
    path: '/settings',
    enter: async () => {
      await import('./settings/settings.view.js');
      return true;
    },
    render: () => html`<erng-settings></erng-settings>`,
  },
];
