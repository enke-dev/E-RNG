/**
 * A stand-in for the Rust side, only used when the app runs in a plain browser (`vite dev`
 * without Tauri). It lets the UI be looked at and clicked through, none of the rules, rounding or
 * validation of the real backend lives here: totals are plain floating point.
 */
import { mockIPC } from '@tauri-apps/api/mocks';

import type {
  Customer,
  InvoiceDraft,
  PositionTemplate,
  Settings,
  StoredInvoice,
} from './api.utils.js';
import { today } from './form.utils.js';

const address = {
  street: 'Marktplatz 2',
  post_code: '54321',
  city: 'Beispielstadt',
  country: 'DE',
};

const settings: Settings = {
  seller: {
    name: 'Muster Software GmbH',
    address: { street: 'Hauptstraße 5', post_code: '50667', city: 'Köln', country: 'DE' },
    vat_id: 'DE123456789',
    tax_number: null,
    contact_name: 'Max Muster',
    phone: '+49 221 123456',
    email: 'max@muster.example',
    iban: 'DE02 1203 0000 0000 2020 51',
    bic: 'BYLADEM1001',
    account_holder: 'Muster Software GmbH',
    legal_info: null,
  },
  tax_rate: '19',
  payment_days: 14,
  number_prefix: 'RE-',
  footer: null,
  logo: null,
};

const customers: Customer[] = [
  {
    id: 1,
    name: 'Beispiel AG',
    address,
    vat_id: null,
    email: 'einkauf@beispiel.example',
    buyer_reference: null,
  },
  {
    id: 2,
    name: 'Stadt Beispielstadt',
    address,
    vat_id: null,
    email: 'rechnung@stadt.example',
    buyer_reference: '991-01234-56',
  },
];

const templates: PositionTemplate[] = [
  { id: 1, description: 'Beratung', quantity: '1', unit: 'HUR', price: '95,00' },
  { id: 2, description: 'Reisekosten', quantity: null, unit: null, price: '42,50' },
  { id: 3, description: 'Wartungsvertrag', quantity: '12', unit: 'MON', price: '49,00' },
];

const draftOf = (customer: Customer): InvoiceDraft => ({
  customer,
  issue_date: '2026-10-01',
  delivery_date: '2026-10-01',
  positions: [{ description: 'Beratung', quantity: '10', unit: 'HUR', price: '95,00' }],
  profile: 'en16931',
  buyer_reference: null,
  note: null,
  frozen: null,
});

const invoices: StoredInvoice[] = [
  {
    id: 1,
    number: 'RE-2026-001',
    status: 'exported',
    draft: { ...draftOf(customers[0] as Customer), frozen: settings },
    total: '1130.50',
    exported_at: '2026-10-02 08:00:00',
    updated_at: '2026-10-02 08:00:00',
  },
  {
    id: 2,
    number: null,
    status: 'draft',
    draft: draftOf(customers[1] as Customer),
    total: '1130.50',
    exported_at: null,
    updated_at: '2026-10-05 09:00:00',
  },
];

const number = (text: string | null | undefined) =>
  Number((text ?? '').replace(/\./g, '').replace(',', '.')) || 0;
const euro = (value: number) => (Math.round(value * 100) / 100).toFixed(2);

function totals(draft: InvoiceDraft) {
  const net = draft.positions.reduce(
    (sum, p) => sum + (number(p.quantity) || 1) * number(p.price),
    0
  );
  const vat = (net * Number(settings.tax_rate)) / 100;
  return { net: euro(net), vat: euro(vat), gross: euro(net + vat) };
}

function check(draft: InvoiceDraft) {
  const problems = [];
  if (draft.customer.name === '') {
    problems.push({ field: 'customer', message: 'Ein Kunde ist erforderlich' });
  }
  draft.positions.forEach((p, i) => {
    if (p.description.trim() === '') {
      problems.push({
        field: `positions[${i}].description`,
        message: 'Eine Beschreibung ist erforderlich',
      });
    }
    if (p.price.trim() === '') {
      problems.push({ field: `positions[${i}].price`, message: 'Ein Preis ist erforderlich' });
    }
  });
  const findings =
    draft.profile === 'xrechnung' && !draft.buyer_reference
      ? [
          {
            rule: 'BR-DE-15',
            severity: 'Fatal',
            message: 'Buyer reference (BT-10) shall be present',
          },
        ]
      : [];
  return { problems, findings, totals: problems.length === 0 ? totals(draft) : null };
}

const find = (id: number) => invoices.find(i => i.id === id) as StoredInvoice;

export function installMocks() {
  // `platform()` reads this synchronously
  (globalThis as unknown as Record<string, unknown>)['__TAURI_OS_PLUGIN_INTERNALS__'] = {
    platform: 'macos',
  };

  mockIPC(
    (command, payload) => {
      const args = (payload ?? {}) as Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
      switch (command) {
        case 'settings_get':
          return settings;
        case 'settings_save':
          Object.assign(settings, args['settings']);
          return null;
        case 'customers_list':
          return customers;
        case 'customer_save': {
          const customer = args['customer'] as Customer;
          if (customer.id === null) {
            customers.push({ ...customer, id: customers.length + 1 });
          }
          return customers.at(
            customer.id === null ? -1 : customers.findIndex(c => c.id === customer.id)
          );
        }
        case 'templates_list':
          return templates;
        case 'template_save': {
          const template = args['template'] as PositionTemplate;
          if (template.id === null) {
            templates.push({ ...template, id: templates.length + 1 });
          } else {
            templates[templates.findIndex(t => t.id === template.id)] = template;
          }
          return template.id === null ? templates.at(-1) : template;
        }
        case 'template_delete':
          templates.splice(
            templates.findIndex(t => t.id === args['id']),
            1
          );
          return null;
        case 'invoices_list':
          return invoices.map(i => ({
            id: i.id,
            number: i.number,
            status: i.status,
            customer_name: i.draft.customer.name,
            issue_date: i.draft.issue_date,
            total: i.total,
            updated_at: i.updated_at,
          }));
        case 'invoice_get':
          return find(args['id']);
        case 'invoice_blank':
          return {
            ...draftOf({ ...(customers[0] as Customer), id: null, name: '' }),
            issue_date: today(),
            delivery_date: today(),
          };
        case 'invoice_save': {
          const draft = args['draft'] as InvoiceDraft;
          const existing = args['id'] === null ? undefined : find(args['id']);
          const stored: StoredInvoice = existing ?? {
            id: invoices.length + 1,
            number: null,
            status: 'draft',
            draft,
            total: null,
            exported_at: null,
            updated_at: '',
          };
          stored.draft = draft;
          stored.total = check(draft).totals?.gross ?? null;
          if (!existing) {
            invoices.push(stored);
          }
          return stored;
        }
        case 'invoice_duplicate': {
          const copy: StoredInvoice = {
            ...structuredClone(find(args['id'])),
            id: invoices.length + 1,
            number: null,
            status: 'draft',
            exported_at: null,
          };
          copy.draft.issue_date = today();
          copy.draft.delivery_date = today();
          copy.draft.frozen = null;
          invoices.push(copy);
          return copy;
        }
        case 'invoice_delete':
          invoices.splice(
            invoices.findIndex(i => i.id === args['id']),
            1
          );
          return null;
        case 'check_invoice':
          return check(args['draft']);
        case 'produce_invoice':
          return {
            work_dir: '/tmp/erng-mock',
            number: find(args['id']).number ?? 'RE-2026-002',
            report: { valid: true, issues: [] },
          };
        case 'export_invoice': {
          const stored = find(args['id']);
          stored.status = 'exported';
          stored.number ??= 'RE-2026-002';
          stored.exported_at = `${today()} 12:00:00`;
          stored.draft.frozen = settings;
          return stored;
        }
        case 'discard_work_dir':
          return null;
        case 'plugin:dialog|save':
          return '/tmp/mock-export';
        case 'plugin:updater|check':
          return null;
        default:
          return null;
      }
    },
    { shouldMockEvents: true }
  );
}
