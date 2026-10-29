/**
 * A stand-in for the Rust side, only used when the app runs in a plain browser (`vite dev`
 * without Tauri). It lets the UI be looked at and clicked through, none of the rules, rounding or
 * validation of the real backend lives here: totals are plain floating point.
 */
import { mockIPC } from '@tauri-apps/api/mocks';

import unitNames from '../../../src-tauri/data/unit-names.json' with { type: 'json' };
import type {
  CatalogEntry,
  Context,
  Customer,
  CustomLayout,
  InvoiceDraft,
  Kind,
  Linked,
  ListEntry,
  NumberReset,
  Position,
  PositionTemplate,
  Sequence,
  Settings,
  StoredInvoice,
  Unit,
  UnitDef,
  UnitUsage,
} from './api.utils.js';
import { today } from './form.utils.js';
import { validUntil } from './kind.utils.js';

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
    mobile: '+49 171 5551234',
    fax: null,
    website: 'https://muster.example',
    iban: 'DE02 1203 0000 0000 2020 51',
    bic: 'BYLADEM1001',
    account_holder: 'Muster Software GmbH',
    bank_name: 'Musterbank Köln',
    legal_info: null,
  },
  tax_rate: '19',
  vat_note: 'Gemäß § 19 UStG wird keine Umsatzsteuer berechnet.',
  payment_days: 14,
  number_pattern: '[RE-]YYYY-NNN',
  number_reset: 'fiscal_year',
  fiscal_year_start: null,
  layout: 'standard',
  intro: null,
  closing: null,
  invoice_wording: {
    title: 'Rechnung',
    draft_title: 'Rechnungsentwurf',
    number_label: 'Rechnungsnummer',
  },
  offer_reference: false,
  offer: {
    number_pattern: '[AN-]YYYY-NNN',
    number_reset: 'fiscal_year',
    validity_days: 30,
    intro: null,
    closing: 'Dieses Angebot gilt bis {valid_until}.',
    layout: null,
    wording: { title: 'Angebot', draft_title: 'Angebotsentwurf', number_label: 'Angebotsnummer' },
  },
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
  offer: { ...settings.offer, number_pattern: '[AN-KV-]YYYY-NNN' },
};

/** A day as the mock numbers write it */
interface Day {
  year: number;
  month: number;
  day: number;
}

/** A day from `2026-10-01` */
const dayOf = (date: string): Day => {
  const [year = 0, month = 0, day = 0] = date.split('-').map(Number);
  return { year, month, day };
};

/** The day the numbers are looked at when no document says otherwise */
const todayDay = () => dayOf(today());

/** Mirrors `Calendar::fiscal` in `numbering.rs` */
const fiscalOf = (start: string | null, { year, month, day }: Day = todayDay()) => {
  const [sy = 0, sm = 0, sd = 0] = (start ?? '').split('-').map(Number);
  if (!start || year * 10000 + month * 100 + day < sy * 10000 + sm * 100 + sd) {
    return { from: year, to: year, quarter: Math.ceil(month / 3), calendar: true };
  }
  const from = month * 100 + day >= sm * 100 + sd ? year : year - 1;
  const months = (year - from) * 12 + month - sm - (day < sd ? 1 : 0);
  return {
    from,
    to: sm === 1 && sd === 1 ? from : from + 1,
    quarter: Math.min(4, Math.floor(months / 3) + 1),
    calendar: false,
  };
};

/** A pattern split at its counter, the date and fiscal tokens written out for a day */
function numberParts(pattern: string, fiscalYearStart: string | null, day: Day = todayDay()) {
  const fiscal = fiscalOf(fiscalYearStart, day);
  const values: Record<string, string> = {
    YYYY: String(day.year),
    YY: String(day.year).slice(-2),
    MM: String(day.month).padStart(2, '0'),
    DD: String(day.day).padStart(2, '0'),
    GGGG: String(fiscal.from),
    GG: String(fiscal.from).slice(-2),
    gggg: String(fiscal.to),
    gg: String(fiscal.to).slice(-2),
    Q: String(fiscal.quarter),
  };
  let width = 0;
  const text = pattern.replace(
    /\[([^\]]*)\]|YYYY|YY|MM|DD|GGGG|GG|gggg|gg|Q|N+/g,
    (m, literal?: string) => {
      if (literal !== undefined) {
        return literal;
      }
      if (m.startsWith('N')) {
        width = m.length;
        return '\0';
      }
      return values[m] ?? m;
    }
  );
  if (width === 0) {
    throw 'Das Muster braucht einen Zähler: N, NNN für drei Stellen mit führenden Nullen.';
  }
  const [before = '', after = '', ...more] = text.split('\0');
  // one run of Ns, not a counter here and another there
  if (more.length > 0) {
    throw 'Das Muster darf nur einen Zähler haben, Text mit N gehört in [eckige Klammern].';
  }
  return { before, after, width };
}

/** Mirrors `Calendar::period` in `numbering.rs` */
function periodKey(reset: NumberReset, fiscalYearStart: string | null, day: Day) {
  const fiscal = fiscalOf(fiscalYearStart, day);
  switch (reset) {
    case 'never':
      return 'never';
    case 'fiscal_year':
      return `year:${fiscal.from}`;
    case 'quarter':
      // the calendar quarters before the first start are no fiscal ones
      return fiscal.calendar
        ? `quarter:${fiscal.from}-${fiscal.quarter}`
        : `quarter:fiscal-${fiscal.from}-${fiscal.quarter}`;
    case 'month':
      return `month:${day.year}-${String(day.month).padStart(2, '0')}`;
  }
}

const RESET_NEEDS: Record<NumberReset, string> = {
  never: '',
  fiscal_year:
    'Mit Neubeginn je Geschäftsjahr muss das Muster das Geschäftsjahr enthalten (z. B. GGGG oder, wenn es dem Kalenderjahr entspricht, YYYY), sonst wiederholen sich Nummern.',
  quarter:
    'Mit Neubeginn je Quartal muss das Muster das Quartal samt Jahr enthalten (z. B. YYYY-[Q]Q), sonst wiederholen sich Nummern.',
  month:
    'Mit Neubeginn je Monat muss das Muster den Monat samt Jahr enthalten (z. B. YYYYMM), sonst wiederholen sich Nummern.',
};

/** Mirrors `SHORT_YEAR_QUARTERS` in `numbering.rs` */
const SHORT_YEAR_QUARTERS =
  'Vor dem Beginn des ersten Geschäftsjahres zählen die Kalenderquartale, danach die Quartale des Geschäftsjahres: Jahr und Quartal wiederholen sich dabei. Mit dem Monat im Muster (z. B. YYYYMM-NN) oder Neubeginn je Monat bleiben die Nummern eindeutig.';

/** Whether the pattern has one counter, what `Pattern::parse` in `numbering.rs` asks for */
function parses(pattern: string) {
  try {
    numberParts(pattern, null);
    return true;
  } catch {
    return false;
  }
}

/**
 * Mirrors `reset_problem` in `numbering.rs`: two days of different periods making the same number,
 * none for a pattern that is broken anyway (`number_pattern` reports that)
 */
function resetProblem(pattern: string, reset: NumberReset, fiscalYearStart: string | null) {
  if (reset === 'never' || !parses(pattern)) {
    return null;
  }
  const first = Math.max(todayDay().year, Number(fiscalYearStart?.slice(0, 4) ?? 0));
  // the period a text was first seen in, and whether that day still counted calendar quarters
  const seen = new Map<string, { period: string; calendar: boolean }>();
  const clash = Array.from({ length: 3 * 366 }, (_, n) => new Date(Date.UTC(first, 0, 1 + n)))
    .map(d => ({ year: d.getUTCFullYear(), month: d.getUTCMonth() + 1, day: d.getUTCDate() }))
    .map(day => {
      const { before, after } = numberParts(pattern, fiscalYearStart, day);
      const now = {
        period: periodKey(reset, fiscalYearStart, day),
        calendar: fiscalOf(fiscalYearStart, day).calendar,
      };
      const earlier = seen.get(before + after);
      seen.set(before + after, earlier ?? now);
      return earlier && earlier.period !== now.period ? { earlier, now } : undefined;
    })
    .find(found => found !== undefined);
  if (!clash) {
    return null;
  }
  return reset === 'quarter' && clash.earlier.calendar !== clash.now.calendar
    ? SHORT_YEAR_QUARTERS
    : RESET_NEEDS[reset];
}

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
    ['number_pattern', !parses(s.number_pattern)],
    ['number_reset', resetProblem(s.number_pattern, s.number_reset, s.fiscal_year_start) !== null],
  ]
    .filter(([, absent]) => absent)
    .map(([field]) => String(field));

/** Mirrors `Settings::missing_for_offers`: the seller's name and address, the offer pattern and reset */
const missingForOffers = (s: Settings) => [
  ...missingOf(s).filter(field => field === 'seller.name' || field.startsWith('seller.address.')),
  ...(parses(s.offer.number_pattern) ? [] : ['offer.number_pattern']),
  ...(resetProblem(s.offer.number_pattern, s.offer.number_reset, s.fiscal_year_start)
    ? ['offer.number_reset']
    : []),
];

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
  missing_for_offers: missingForOffers(c.settings),
}));

const refreshed = (c: Context): Context => ({
  ...c,
  missing: missingOf(c.settings),
  missing_for_xrechnung: missingForXRechnung(c.settings),
  missing_for_offers: missingForOffers(c.settings),
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
  page: { margins: { top: 20, right: 20, bottom: 42, left: 25 }, parts: [] },
  first: {
    margins: { top: 98, right: 20, bottom: 42, left: 25 },
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
  {
    id: 1,
    context_id: null,
    description: 'Beratung',
    quantity: '1',
    unit: 'HUR',
    unit_label: 'Std.',
    price: '95,00',
  },
  {
    id: 2,
    context_id: null,
    description: 'Reisekosten',
    quantity: null,
    unit: 'C62',
    unit_label: 'Stk.',
    price: '42,50',
  },
  {
    id: 3,
    context_id: null,
    description: 'Wartungsvertrag',
    quantity: '12',
    unit: 'MON',
    unit_label: 'Monat(e)',
    price: '49,00',
  },
];

/** What every context starts with, the same as `units::DEFAULTS` */
const DEFAULT_UNITS: UnitDef[] = [
  { code: 'C62', label: 'Stk.' },
  { code: 'HUR', label: 'Std.' },
  { code: 'LS', label: 'pauschal' },
];

/** The units of all contexts, in the order of their lists; the shared template uses "Monat(e)" */
const units: Unit[] = [
  ...[...DEFAULT_UNITS, { code: 'MON', label: 'Monat(e)' }].map(u => ({ ...u, context_id: 1 })),
  ...DEFAULT_UNITS.map(u => ({ ...u, context_id: 2 })),
].map((u, i) => ({ ...u, id: i + 1 }));

const nextUnitId = () => Math.max(0, ...units.map(u => u.id as number)) + 1;

const unitsOf = (contextId: number) => units.filter(u => u.context_id === contextId);

/** The same names the app ships (`units::catalog`) */
const catalog: CatalogEntry[] = unitNames.units;

const namesUnit = (unit: Unit) => (p: Pick<Position, 'unit' | 'unit_label'>) =>
  (p.unit ?? 'C62') === unit.code && p.unit_label === unit.label;

/** The drafts of the context and the saved positions it sees, what a unit change reaches */
const reachedBy = (unit: Unit) => {
  const names = namesUnit(unit);
  return {
    templates: templates.filter(
      t => (t.context_id === null || t.context_id === unit.context_id) && names(t)
    ),
    drafts: invoices.filter(
      i => i.context_id === unit.context_id && i.status === 'draft' && i.draft.positions.some(names)
    ),
    names,
  };
};

const usageOf = (unit: Unit): UnitUsage => {
  const { templates: saved, drafts } = reachedBy(unit);
  return {
    templates: saved.map(t => ({
      id: t.id as number,
      description: t.description,
      context_id: t.context_id,
    })),
    drafts: drafts.map(i => ({
      invoice_id: i.id,
      kind: i.kind,
      customer: i.draft.customer.name,
    })),
  };
};

/** Drafts and saved positions that named `from` name `to` instead */
function retarget(from: Unit, to: UnitDef) {
  const { templates: saved, drafts, names } = reachedBy(from);
  const point = (p: Pick<Position, 'unit' | 'unit_label'>) => {
    p.unit = to.code;
    p.unit_label = to.label;
  };
  saved.forEach(point);
  drafts.forEach(i => i.draft.positions.filter(names).forEach(point));
}

/** Mirrors `units::check`, roughly: the code is only looked at for case and spaces */
function checkedUnit(unit: UnitDef, others: Unit[]): UnitDef {
  const label = unit.label.trim();
  if (label === '') {
    throw 'Eine Bezeichnung ist erforderlich.';
  }
  if (unit.code !== unit.code.trim() || unit.code === '') {
    throw `Der Code „${unit.code}“ ist nicht zulässig: it contains whitespace`;
  }
  if (unit.code !== unit.code.toUpperCase()) {
    throw `Der Code „${unit.code}“ ist nicht zulässig: did you mean "${unit.code.toUpperCase()}"?`;
  }
  if (others.some(o => o.label.trim().toLowerCase() === label.toLowerCase())) {
    throw `Die Bezeichnung „${label}“ gibt es schon.`;
  }
  return { code: unit.code, label };
}

const draftOf = (customer: Customer): InvoiceDraft => ({
  customer,
  issue_date: '2026-10-01',
  delivery_date: '2026-10-01',
  delivery_end: null,
  validity_days: null,
  positions: [
    {
      description: 'Beratung',
      quantity: '10',
      unit: 'HUR',
      unit_label: 'Std.',
      price: '95,00',
    },
  ],
  profile: 'en16931',
  buyer_reference: null,
  note: null,
  layout: null,
  intro: null,
  closing: null,
  offer_reference: false,
  frozen: null,
});

const invoices: StoredInvoice[] = [
  {
    id: 1,
    context_id: 1,
    kind: 'invoice',
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
    kind: 'invoice',
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
  // an offer that has run out, one still valid and a draft
  ...[
    { id: 3, number: 'AN-2026-001', issued: '2026-09-01', days: 30, customer: 0 },
    { id: 4, number: 'AN-2026-002', issued: '2026-10-20', days: 30, customer: 1 },
    { id: 5, number: null, issued: '2026-10-27', days: 30, customer: 0 },
  ].map((o): StoredInvoice => ({
    id: o.id,
    context_id: 1,
    kind: 'offer',
    number: o.number,
    status: o.number ? 'exported' : 'draft',
    draft: {
      ...draftOf(customers[o.customer] as Customer),
      issue_date: o.issued,
      delivery_date: o.issued,
      validity_days: o.days,
      frozen: o.number ? settings : null,
    },
    total: '1130.50',
    exported_at: o.number ? `${o.issued} 08:00:00` : null,
    paid_at: null,
    updated_at: `${o.issued} 08:00:00`,
    archived_at: null,
    files: null,
  })),
];

/** Offer id and invoice id, the first offer was billed with the first invoice */
let links: [number, number][] = [[3, 1]];

/** Ids are not reused after a draft was deleted */
const nextId = () => Math.max(0, ...invoices.map(i => i.id)) + 1;

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

/** The counters a context has continued from, by period */
const floors = new Map<string, number>();

/** The pattern and reset of a context's kind, written out for a day */
function periodOf(contextId: number, kind: Kind, day: Day) {
  const settings = (contexts.find(c => c.id === contextId) as Context).settings;
  const pattern = kind === 'offer' ? settings.offer.number_pattern : settings.number_pattern;
  const reset = kind === 'offer' ? settings.offer.number_reset : settings.number_reset;
  const parts = numberParts(pattern, settings.fiscal_year_start, day);
  const period = periodKey(reset, settings.fiscal_year_start, day);
  return { ...parts, period, key: `${contextId}\u001f${kind}\u001f${period}` };
}

/** The numbers given in the period of a day, the highest counter first, like `Calendar::highest` */
const numbersOf = (contextId: number, kind: Kind, day: Day) => {
  const { period } = periodOf(contextId, kind, day);
  return invoices
    .filter(i => i.context_id === contextId && i.kind === kind && i.number)
    .flatMap(i => {
      // a number is read with the pattern of the day it was issued on
      const issued = dayOf(i.draft.issue_date);
      const { before, after, period: own } = periodOf(contextId, kind, issued);
      const digits = (i.number as string).slice(before.length, -after.length || undefined);
      return own === period &&
        i.number?.startsWith(before) &&
        i.number.endsWith(after) &&
        /^\d+$/.test(digits)
        ? [{ number: i.number, counter: Number(digits) }]
        : [];
    })
    .sort((a, b) => b.counter - a.counter);
};

const highestCounter = (contextId: number, kind: Kind, day: Day) =>
  numbersOf(contextId, kind, day)[0]?.counter;

/** Mirrors `Store::sequence`, roughly */
function sequenceOf(contextId: number, kind: Kind, day: Day = todayDay()): Sequence {
  const { before, after, width, key } = periodOf(contextId, kind, day);
  const highest = numbersOf(contextId, kind, day)[0];
  const floor = floors.get(key) ?? 0;
  const counter = Math.max((highest?.counter ?? 0) + 1, floor);
  return {
    next_number: `${before}${String(counter).padStart(width, '0')}${after}`,
    counter,
    highest: highest?.number ?? null,
    floor: floor > (highest?.counter ?? 0) ? floor : null,
  };
}

/** The number a stored document gets: the sequence of its kind on its issue date */
const numberOf = (stored: StoredInvoice) =>
  sequenceOf(stored.context_id, stored.kind, dayOf(stored.draft.issue_date)).next_number;

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
          const { before, after, width } = numberParts(
            String(args['pattern']),
            args['fiscalYearStart'] ?? null
          );
          const problem = resetProblem(
            String(args['pattern']),
            args['reset'],
            args['fiscalYearStart'] ?? null
          );
          if (problem) {
            throw problem;
          }
          return `${before}${'1'.padStart(width, '0')}${after}`;
        }
        case 'number_sequence':
          return sequenceOf(args['contextId'], args['kind']);
        case 'number_sequence_set': {
          const contextId = args['contextId'] as number;
          const kind = args['kind'] as Kind;
          const next = args['next'] as number;
          const day = todayDay();
          const counter = highestCounter(contextId, kind, day) ?? 0;
          if (next <= counter) {
            throw `${sequenceOf(contextId, kind, day).highest} ist bereits vergeben, der Zähler kann nur auf ${counter + 1} oder höher gesetzt werden.`;
          }
          const { key } = periodOf(contextId, kind, day);
          if (next === counter + 1) {
            floors.delete(key);
          } else {
            floors.set(key, next);
          }
          return sequenceOf(contextId, kind, day);
        }
        case 'contexts_list':
          // a new array each time, as the backend hands one out: the signal only notices a change then
          return [...contexts];
        case 'context_blank':
          return refreshed({
            id: null,
            brand: '',
            archived_at: null,
            missing: [],
            missing_for_xrechnung: [],
            missing_for_offers: [],
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
          const { number_pattern, number_reset, fiscal_year_start, offer } = context.settings;
          // a pattern that cannot count is refused with the reason, like `Pattern::parse`
          numberParts(number_pattern, fiscal_year_start);
          numberParts(offer.number_pattern, fiscal_year_start);
          const problem =
            resetProblem(number_pattern, number_reset, fiscal_year_start) ??
            resetProblem(offer.number_pattern, offer.number_reset, fiscal_year_start);
          if (problem) {
            throw problem;
          }
          if (context.id === null) {
            const saved = refreshed({
              ...context,
              id: contexts.length + 1,
              brand: context.brand || 'orange',
            });
            contexts.push(saved);
            const given = (args['units'] as UnitDef[] | null) ?? [];
            (given.length > 0 ? given : DEFAULT_UNITS).forEach(u =>
              units.push({ ...u, id: nextUnitId(), context_id: saved.id as number })
            );
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
          const of = (kind: Kind) => invoices.filter(i => i.context_id === id && i.kind === kind);
          return {
            drafts: of('invoice').filter(i => i.status === 'draft').length,
            exported: of('invoice').filter(i => i.status === 'exported').length,
            offers: of('offer').length,
            customers: 0,
            templates: 0,
            layouts: 0,
          };
        }
        case 'context_remove':
          units.splice(0, units.length, ...units.filter(u => u.context_id !== args['id']));
          contexts.splice(
            contexts.findIndex(c => c.id === args['id']),
            1
          );
          return null;
        case 'settings_check':
          return {
            missing: missingOf(args['settings']),
            missing_for_xrechnung: missingForXRechnung(args['settings']),
            missing_for_offers: missingForOffers(args['settings']),
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
          return [{ family: 'Mock Sans', style: 'Regular', path: String(args['path']) }];
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
        case 'units_list':
          return unitsOf(args['contextId']).map(u => ({ ...u }));
        case 'units_catalog':
          return catalog;
        case 'plugin:opener|open_url':
          window.open(String(args['url']), '_blank', 'noopener');
          return null;
        case 'units_counts':
          return Object.fromEntries(
            unitsOf(args['contextId']).map(u => {
              const usage = usageOf(u);
              return [u.id, usage.templates.length + usage.drafts.length];
            })
          );
        case 'unit_save': {
          const unit = args['unit'] as Unit;
          const before = unit.id === null ? undefined : units.find(u => u.id === unit.id);
          const context = before?.context_id ?? unit.context_id;
          const def = checkedUnit(
            unit,
            unitsOf(context).filter(u => u.id !== unit.id)
          );
          if (!before) {
            const added = { ...def, id: nextUnitId(), context_id: context };
            units.push(added);
            return { ...added };
          }
          const old = { ...before };
          Object.assign(before, def);
          retarget(old, def);
          return { ...before };
        }
        case 'unit_usage':
          return usageOf(units.find(u => u.id === args['id']) as Unit);
        case 'unit_delete': {
          const unit = units.find(u => u.id === args['id']) as Unit;
          const target = units.find(u => u.id === args['replacement']);
          if (unitsOf(unit.context_id).length < 2) {
            throw 'Die letzte Einheit kann nicht gelöscht werden.';
          }
          const usage = usageOf(unit);
          if (usage.templates.length + usage.drafts.length > 0) {
            if (!target || target.context_id !== unit.context_id || target === unit) {
              throw 'Die Einheit wird noch verwendet, bitte eine Ersatz-Einheit wählen.';
            }
            retarget(unit, target);
          }
          units.splice(units.indexOf(unit), 1);
          return null;
        }
        case 'units_reorder': {
          const ids = args['ids'] as number[];
          const own = unitsOf(args['contextId']);
          units.splice(
            0,
            units.length,
            ...units.filter(u => u.context_id !== args['contextId']),
            ...ids.map(id => own.find(u => u.id === id) as Unit)
          );
          return null;
        }
        case 'invoice_draft_pdf':
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
            .filter(i => i.kind === args['kind'])
            .filter(i => args['contextId'] == null || i.context_id === args['contextId'])
            .filter(i => i.archived_at === null || Boolean(args['withArchived']))
            .map((i): ListEntry => ({
              id: i.id,
              context_id: i.context_id,
              kind: i.kind,
              valid_until:
                i.kind === 'offer' ? validUntil(i.draft.issue_date, i.draft.validity_days) : null,
              seller_name: sellerOf(i.context_id),
              number: i.number,
              status: i.status,
              customer_name: i.draft.customer.name,
              issue_date: i.draft.issue_date,
              total: i.total,
              paid_at: i.paid_at,
              updated_at: i.updated_at,
              archived_at: i.archived_at,
              linked: links.filter(l => l.includes(i.id)).length,
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
        case 'invoice_blank': {
          // mirrors `Settings::blank_draft`
          const kind = args['kind'] as Kind;
          const own = (contexts.find(c => c.id === args['contextId']) as Context).settings;
          return {
            ...draftOf({ ...(customers[0] as Customer), id: null, name: '' }),
            issue_date: today(),
            delivery_date: today(),
            delivery_end: null,
            validity_days: kind === 'offer' ? own.offer.validity_days : null,
            offer_reference: kind === 'invoice' && own.offer_reference,
            layout: (kind === 'offer' && own.offer.layout) || own.layout,
            intro: (kind === 'offer' ? own.offer.intro : own.intro) ?? '',
            closing: (kind === 'offer' ? own.offer.closing : own.closing) ?? '',
          };
        }
        case 'invoice_save': {
          const draft = args['draft'] as InvoiceDraft;
          const existing = args['id'] === null ? undefined : find(args['id']);
          const stored: StoredInvoice = existing ?? {
            id: nextId(),
            context_id: (args['contextId'] as number | null) ?? 1,
            kind: args['kind'] as Kind,
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
            id: nextId(),
            number: null,
            status: 'draft',
            exported_at: null,
            paid_at: null,
            archived_at: null,
          };
          copy.draft.issue_date = today();
          copy.draft.delivery_date = today();
          copy.draft.delivery_end = copy.draft.delivery_end === null ? null : today();
          copy.draft.frozen = null;
          invoices.push(copy);
          return copy;
        }
        case 'invoice_delete':
          invoices.splice(
            invoices.findIndex(i => i.id === args['id']),
            1
          );
          links = links.filter(l => !l.includes(args['id']));
          return null;
        case 'offer_to_invoice': {
          // mirrors `Store::invoice_from_offer`
          const offer = find(args['offerId']);
          if (offer.kind !== 'offer' || offer.status !== 'exported') {
            throw 'Nur aus einem erstellten Angebot kann eine Rechnung entstehen.';
          }
          const own = (contexts.find(c => c.id === offer.context_id) as Context).settings;
          const { customer, positions, buyer_reference, note } = structuredClone(offer.draft);
          const draft: InvoiceDraft = {
            ...draftOf(customer),
            issue_date: today(),
            delivery_date: today(),
            positions,
            buyer_reference,
            note,
            layout: own.layout,
            intro: own.intro ?? '',
            closing: own.closing ?? '',
            offer_reference: own.offer_reference,
          };
          const invoice: StoredInvoice = {
            id: nextId(),
            context_id: offer.context_id,
            kind: 'invoice',
            number: null,
            status: 'draft',
            draft,
            total: check(draft).totals?.gross ?? null,
            exported_at: null,
            paid_at: null,
            updated_at: '',
            archived_at: null,
            files: null,
          };
          invoices.push(invoice);
          links.push([offer.id, invoice.id]);
          return invoice;
        }
        case 'document_links':
          return links
            .filter(l => l.includes(args['id']))
            .map(l => find(l.find(id => id !== args['id']) as number))
            .map((i): Linked => ({
              id: i.id,
              kind: i.kind,
              number: i.number,
              status: i.status,
              customer_name: i.draft.customer.name,
              issue_date: i.draft.issue_date,
            }));
        case 'document_link': {
          const [offer, invoice] = [find(args['offerId']), find(args['invoiceId'])];
          if (offer.kind !== 'offer' || invoice.kind !== 'invoice') {
            throw 'Eine Verknüpfung verbindet ein Angebot mit einer Rechnung.';
          }
          if (offer.context_id !== invoice.context_id) {
            throw 'Angebot und Rechnung werden innerhalb eines Kontexts verknüpft.';
          }
          if (!links.some(l => l[0] === offer.id && l[1] === invoice.id)) {
            links.push([offer.id, invoice.id]);
          }
          return null;
        }
        case 'document_unlink':
          links = links.filter(l => l[0] !== args['offerId'] || l[1] !== args['invoiceId']);
          return null;
        case 'check_invoice':
          // an offer has no rules to check
          return args['kind'] === 'offer'
            ? { ...check(args['draft']), findings: [] }
            : check(args['draft']);
        case 'invoice_next_number':
          return numberOf(find(args['id']));
        case 'invoice_create': {
          const stored = find(args['id']);
          // what the backend refuses: an XRechnung without buyer reference
          if (
            stored.kind === 'invoice' &&
            stored.draft.profile === 'xrechnung' &&
            !stored.draft.buyer_reference
          ) {
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
          stored.number = numberOf(stored);
          stored.exported_at = `${today()} 12:00:00`;
          stored.draft.frozen = settings;
          stored.files = {
            pdf: {
              path: `${stored.kind === 'offer' ? 'offers' : 'invoices'}/1/${stored.number}.pdf`,
              sha256: '0'.repeat(64),
            },
            xml:
              stored.kind === 'offer'
                ? null
                : { path: `invoices/1/${stored.number}.xml`, sha256: '0'.repeat(64) },
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
        // `ask` is a message with two custom buttons: the question is answered yes (the label of the
        // first button), except the one that stops leaving a form with edits
        case 'plugin:dialog|message': {
          const [ok, cancel] = (args['buttons']?.OkCancelCustom ?? ['Ok', 'Abbrechen']) as string[];
          return args['title'] === 'Änderungen verwerfen?' ? cancel : ok;
        }
        case 'plugin:updater|check':
          return null;
        default:
          return null;
      }
    },
    { shouldMockEvents: true }
  );
}
