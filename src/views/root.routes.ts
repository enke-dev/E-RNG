import type { RouteConfig } from '@lit-labs/router';
import { html } from 'lit';

/** Views are loaded on first visit, the shell stays small */
export const routes: RouteConfig[] = [
  {
    path: '/',
    enter: async () => {
      await import('./invoices/invoices.view.js');
      return true;
    },
    render: () => html`<erng-invoices></erng-invoices>`,
  },
  {
    path: '/invoices/:id',
    enter: async () => {
      await import('./invoice-edit/invoice-edit.view.js');
      return true;
    },
    render: ({ id }) => html`<erng-invoice-edit invoice-id="${id ?? 'new'}"></erng-invoice-edit>`,
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
    path: '/settings',
    enter: async () => {
      await import('./settings/settings.view.js');
      return true;
    },
    render: () => html`<erng-settings></erng-settings>`,
  },
];
