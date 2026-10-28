/**
 * A stand-in for the Rust side, only used when the app runs in a plain browser (`vite dev`
 * without Tauri). It lets the UI be looked at and clicked through, none of the rules, rounding or
 * validation of the real backend lives here: totals are plain floating point.
 */
import { mockIPC } from '@tauri-apps/api/mocks';

import type {
  Context,
  Customer,
  CustomLayout,
  InvoiceDraft,
  ListEntry,
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
  vat_note: 'Gemäß § 19 UStG wird keine Umsatzsteuer berechnet.',
  payment_days: 14,
  number_pattern: '[RE-]YYYY-NNN',
  layout: 'standard',
  intro: null,
  closing: null,
};

const second: Settings = {
  ...settings,
  seller: {
    ...settings.seller,
    name: 'Kinderverein e.V.',
    vat_id: null,
    tax_number: '123/456/78901',
    iban: 'DE89 3704 0044 0532 0130 00',
  },
  tax_rate: '',
  number_pattern: '[KV-]YYYY-NNN',
};

/** Mirrors `Settings::missing` in `context.rs`, roughly: only to see the guard and the form */
const missingOf = (s: Settings) =>
  [
    ['seller.name', s.seller.name.trim() === ''],
    ['seller.address.street', s.seller.address.street.trim() === ''],
    ['seller.address.post_code', s.seller.address.post_code.trim() === ''],
    ['seller.address.city', s.seller.address.city.trim() === ''],
    [
      'seller.vat_id',
      (s.seller.vat_id ?? '').trim() === '' && (s.seller.tax_number ?? '').trim() === '',
    ],
    ['seller.iban', s.seller.iban.trim() === ''],
  ]
    .filter(([, absent]) => absent)
    .map(([field]) => String(field));

const missingForXRechnung = (s: Settings) =>
  [
    ['seller.contact_name', s.seller.contact_name.trim() === ''],
    ['seller.phone', s.seller.phone.trim() === ''],
    ['seller.email', s.seller.email.trim() === ''],
  ]
    .filter(([, absent]) => absent)
    .map(([field]) => String(field));

const contexts: Context[] = [
  { id: 1, brand: 'blue', archived_at: null, settings },
  { id: 2, brand: 'green', archived_at: null, settings: second },
].map(c => ({
  ...c,
  missing: missingOf(c.settings),
  missing_for_xrechnung: missingForXRechnung(c.settings),
}));

const refreshed = (c: Context): Context => ({
  ...c,
  missing: missingOf(c.settings),
  missing_for_xrechnung: missingForXRechnung(c.settings),
});

const sellerOf = (contextId: number) =>
  contexts.find(c => c.id === contextId)?.settings.seller.name ?? '';

const customers: Customer[] = [
  {
    id: 1,
    context_id: null,
    name: 'Beispiel AG',
    address,
    vat_id: null,
    email: 'einkauf@beispiel.example',
    buyer_reference: null,
  },
  {
    id: 2,
    context_id: null,
    name: 'Stadt Beispielstadt',
    address,
    vat_id: null,
    email: 'rechnung@stadt.example',
    buyer_reference: '991-01234-56',
  },
];

const customLayouts: CustomLayout[] = [];

/** A small stand-in for the built-in layout the backend hands out */
const builtinLayout = (): CustomLayout => ({
  id: null,
  context_id: null,
  uid: '',
  updated_at: '',
  name: 'Klassisch',
  page: { margins: { top: 20, right: 20, bottom: 36, left: 25 }, parts: [] },
  first: {
    margins: { top: 98, right: 20, bottom: 36, left: 25 },
    parts: [
      {
        kind: 'address',
        anchor: 'top-left',
        x: 25,
        y: 50,
        width: 80,
        sender_line: true,
        size: 10,
      },
      {
        kind: 'infos',
        anchor: 'top-right',
        x: 20,
        y: 45,
        width: 75,
        fields: ['number', 'date', 'service-date', 'due-date'],
        labels: true,
        align: 'right',
        long_dates: false,
        size: 9,
      },
    ],
  },
  fonts: [],
  font: null,
  format: {
    decimal: ',',
    thousands: '.',
    currency_symbol: true,
    currency_after: true,
    currency_space: true,
    percent_space: true,
    true_minus: false,
    iban_groups: false,
    date: 'DD.MM.YYYY',
    date_long: 'D. MMMM YYYY',
  },
  archived: false,
});

const templates: PositionTemplate[] = [
  { id: 1, context_id: null, description: 'Beratung', quantity: '1', unit: 'HUR', price: '95,00' },
  {
    id: 2,
    context_id: null,
    description: 'Reisekosten',
    quantity: null,
    unit: null,
    price: '42,50',
  },
  {
    id: 3,
    context_id: null,
    description: 'Wartungsvertrag',
    quantity: '12',
    unit: 'MON',
    price: '49,00',
  },
];

const draftOf = (customer: Customer): InvoiceDraft => ({
  customer,
  issue_date: '2026-10-01',
  delivery_date: '2026-10-01',
  positions: [{ description: 'Beratung', quantity: '10', unit: 'HUR', price: '95,00' }],
  profile: 'en16931',
  buyer_reference: null,
  note: null,
  layout: null,
  intro: null,
  closing: null,
  frozen: null,
});

const invoices: StoredInvoice[] = [
  {
    id: 1,
    context_id: 1,
    number: 'RE-2026-001',
    status: 'exported',
    draft: { ...draftOf(customers[0] as Customer), frozen: settings },
    total: '1130.50',
    exported_at: '2026-10-02 08:00:00',
    paid_at: null,
    updated_at: '2026-10-02 08:00:00',
    archived_at: null,
    files: null,
  },
  {
    id: 2,
    context_id: 1,
    number: null,
    status: 'draft',
    draft: draftOf(customers[1] as Customer),
    total: '1130.50',
    exported_at: null,
    paid_at: null,
    updated_at: '2026-10-05 09:00:00',
    archived_at: null,
    files: null,
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
  const vat = (net * (Number(settings.tax_rate) || 0)) / 100;
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
        case 'number_example': {
          // a rough stand-in for the real pattern, enough to see the hint
          const pattern = String(args['pattern']);
          if (!/N/.test(pattern.replace(/\[[^\]]*\]/g, ''))) {
            throw 'Das Muster braucht einen Zähler: N, NNN für drei Stellen mit führenden Nullen.';
          }
          return pattern.replace(
            /\[([^\]]*)\]|YYYY|MM|DD|N+/g,
            (m, text?: string) =>
              text ??
              (m === 'YYYY'
                ? '2026'
                : m === 'MM'
                  ? '03'
                  : m === 'DD'
                    ? '07'
                    : '1'.padStart(m.length, '0'))
          );
        }
        case 'contexts_list':
          return contexts;
        case 'context_blank':
          return refreshed({
            id: null,
            brand: '',
            archived_at: null,
            missing: [],
            missing_for_xrechnung: [],
            settings: {
              ...settings,
              seller: {
                ...settings.seller,
                name: '',
                vat_id: null,
                iban: '',
                address: { street: '', post_code: '', city: '', country: 'DE' },
              },
            },
          });
        case 'context_save': {
          const context = args['context'] as Context;
          if (context.id === null) {
            const saved = refreshed({
              ...context,
              id: contexts.length + 1,
              brand: context.brand || 'orange',
            });
            contexts.push(saved);
            return saved;
          }
          const at = contexts.findIndex(c => c.id === context.id);
          contexts[at] = refreshed({ ...context, archived_at: contexts[at]?.archived_at ?? null });
          return contexts[at];
        }
        case 'context_archive': {
          const context = contexts.find(c => c.id === args['id']) as Context;
          context.archived_at = new Date().toISOString();
          return context;
        }
        case 'context_usage': {
          const id = args['id'];
          return {
            drafts: invoices.filter(i => i.context_id === id && i.status === 'draft').length,
            exported: invoices.filter(i => i.context_id === id && i.status === 'exported').length,
            customers: 0,
            templates: 0,
            layouts: 0,
          };
        }
        case 'context_remove':
          contexts.splice(
            contexts.findIndex(c => c.id === args['id']),
            1
          );
          return null;
        case 'settings_check':
          return {
            missing: missingOf(args['settings']),
            missing_for_xrechnung: missingForXRechnung(args['settings']),
          };
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
        case 'layouts_custom':
          return customLayouts;
        case 'layout_save': {
          const layout = args['layout'] as CustomLayout;
          if (layout.id === null) {
            customLayouts.push({
              ...layout,
              id: customLayouts.length + 1,
              uid: `mock-${customLayouts.length + 1}`,
              updated_at: new Date().toISOString(),
            });
            return customLayouts.at(-1);
          }
          customLayouts[customLayouts.findIndex(l => l.id === layout.id)] = layout;
          return layout;
        }
        case 'layout_delete':
          customLayouts.splice(
            customLayouts.findIndex(l => l.id === args['id']),
            1
          );
          return null;
        case 'layout_import_font':
          return { family: 'Mock Sans', style: 'Regular', path: String(args['path']) };
        case 'layouts_list':
          return [
            {
              id: 'standard',
              name: 'Klassisch',
              description:
                'Serifenschrift, Positionen mit Menge, Einheit und Preisen, Zahlungsblock',
              custom: false,
              archived: false,
            },
            ...customLayouts.map(l => ({
              id: `custom-${l.id}`,
              name: l.name,
              description: 'Eigenes Layout',
              custom: true,
              archived: l.archived,
            })),
          ];
        case 'layout_preview':
          return {
            pages: [1, 2].map(
              n =>
                `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 210 297"><text x="20" y="40" font-size="12">Beispiel Seite ${n}</text></svg>`
            ),
            problems: [],
            // where the parts are, so the pointer can find them in the sample
            parts: [
              { page: 1, first: true, index: 0, x: 25, y: 50, w: 80, h: 22 },
              { page: 1, first: true, index: 1, x: 115, y: 50, w: 75, h: 22 },
              { page: 2, first: false, index: 0, x: 25, y: 20, w: 60, h: 10 },
            ],
          };
        case 'layout_export':
          return null;
        case 'layout_import': {
          const layout = {
            ...builtinLayout(),
            id: customLayouts.length + 1,
            uid: `mock-${customLayouts.length + 1}`,
            name: 'Importiert',
          };
          customLayouts.push(layout);
          return { kind: 'done', layout, missing_fonts: ['Mock Sans'] };
        }
        case 'layout_builtin':
          return builtinLayout();
        case 'layout_import_image':
          return {
            name: String(args['path']).split('/').pop(),
            path: String(args['path']),
            width_px: 300,
            height_px: 200,
            vector: false,
          };
        case 'layout_usage':
          return [];
        case 'templates_list':
          return templates;
        case 'template_save': {
          const template = args['template'] as PositionTemplate;
          const same = templates.find(
            t =>
              template.id === null &&
              t.description.trim().toLowerCase() === template.description.trim().toLowerCase() &&
              t.price === template.price
          );
          if (same) {
            return same;
          }
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
        case 'preview_invoice': {
          const draft = args['draft'] as InvoiceDraft;
          const lines = draft.positions.map(
            (p, i) =>
              `<text x="60" y="${330 + i * 22}" font-size="12">${i + 1}  ${p.description || '–'}   ${p.price} €</text>`
          );
          return {
            pages: [
              `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 595 842"><rect width="595" height="842" fill="#fff"/>
               <text x="60" y="80" font-size="18" font-weight="bold" fill="#222">Vorschau (Mock)</text>
               <text x="60" y="200" font-size="13" fill="#222">${draft.customer.name || 'Kunde'}</text>
               <g fill="#222" font-family="serif">${lines.join('')}</g></svg>`,
            ],
            problems: [],
            parts: [],
          };
        }
        case 'invoices_list':
          return invoices
            .filter(i => args['contextId'] == null || i.context_id === args['contextId'])
            .filter(i => i.archived_at === null || Boolean(args['withArchived']))
            .map((i): ListEntry => ({
              id: i.id,
              context_id: i.context_id,
              seller_name: sellerOf(i.context_id),
              number: i.number,
              status: i.status,
              customer_name: i.draft.customer.name,
              issue_date: i.draft.issue_date,
              total: i.total,
              paid_at: i.paid_at,
              updated_at: i.updated_at,
              archived_at: i.archived_at,
            }));
        case 'invoice_archive':
          find(args['id']).archived_at = new Date().toISOString();
          return null;
        case 'invoice_unarchive': {
          const stored = find(args['id']);
          stored.archived_at = null;
          return { ...stored };
        }
        case 'invoice_set_paid': {
          const stored = find(args['id']);
          stored.paid_at = (args['paidAt'] as string | null) ?? null;
          return { ...stored };
        }
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
            context_id: (args['contextId'] as number | null) ?? 1,
            number: null,
            status: 'draft',
            draft,
            total: null,
            exported_at: null,
            paid_at: null,
            updated_at: '',
            archived_at: null,
            files: null,
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
            paid_at: null,
            archived_at: null,
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
        case 'invoice_next_number':
          return `RE-2026-${String(invoices.filter(i => i.number).length + 1).padStart(3, '0')}`;
        case 'invoice_create': {
          const stored = find(args['id']);
          // what the backend refuses: an XRechnung without buyer reference
          if (stored.draft.profile === 'xrechnung' && !stored.draft.buyer_reference) {
            throw {
              kind: 'invalid',
              findings: [
                {
                  rule: 'BR-DE-15',
                  severity: 'Fatal',
                  message: 'Für XRechnung ist die Leitweg-ID (Käuferreferenz) erforderlich.',
                },
              ],
            };
          }
          stored.status = 'exported';
          stored.number = `RE-2026-${String(invoices.filter(i => i.number).length + 1).padStart(3, '0')}`;
          stored.exported_at = `${today()} 12:00:00`;
          stored.draft.frozen = settings;
          stored.files = {
            pdf: { path: `invoices/1/${stored.number}.pdf`, sha256: '0'.repeat(64) },
            xml: { path: `invoices/1/${stored.number}.xml`, sha256: '0'.repeat(64) },
            regenerated: false,
          };
          return { invoice: { ...stored }, warnings: [] };
        }
        case 'invoice_output':
          return find(args['id']);
        case 'opened_files':
          return [];
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
