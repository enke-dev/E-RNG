/**
 * A stand-in for the Rust side, only used when the app runs in a plain browser (`vite dev`
 * without Tauri). It lets the UI be looked at and clicked through, none of the rules, rounding or
 * validation of the real backend lives here: totals are plain floating point.
 */
import { mockIPC } from '@tauri-apps/api/mocks';

import unitNames from '../../../src-tauri/data/unit-names.json' with { type: 'json' };
import type {
  Address,
  Answer,
  Case,
  CaseGraph,
  CaseLink,
  CaseRef,
  CaseStage,
  CatalogEntry,
  Context,
  Customer,
  CustomLayout,
  DocumentEvent,
  EdgeKind,
  EventKind,
  GraphEdge,
  InvoiceDraft,
  Kind,
  Linked,
  ListEntry,
  NumberReset,
  OfferBilling,
  OnAccountInvoice,
  Position,
  PositionTemplate,
  Rate,
  RateDef,
  RateUsage,
  Relation,
  Seller,
  Sequence,
  Settings,
  Status,
  StoredInvoice,
  Unit,
  UnitDef,
  UnitUsage,
} from './api.utils.js';
import { billedPosition, isClosing, quantityOf } from './billing.utils.js';
import { today } from './form.utils.js';
import { cleanInvisible, cleanInvisibleLine } from './invisible.utils.js';
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
    vat_id: 'DE123456788',
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
  unit: null,
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
  cases: { number_pattern: '[VG-]YYYY-NNN', number_reset: 'fiscal_year' },
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

/** Like `Address::cleaned`: every field one line without what a paste brings along */
const cleanAddress = (address: Address): Address => ({
  street: cleanInvisibleLine(address.street),
  post_code: cleanInvisibleLine(address.post_code),
  city: cleanInvisibleLine(address.city),
  country: cleanInvisibleLine(address.country),
});

const someLine = (value: string | null) => (value === null ? null : cleanInvisibleLine(value));

/** Like `Customer::cleaned` */
const cleanCustomer = (customer: Customer): Customer => ({
  ...customer,
  name: cleanInvisibleLine(customer.name),
  address: cleanAddress(customer.address),
  vat_id: someLine(customer.vat_id),
  email: someLine(customer.email),
  buyer_reference: someLine(customer.buyer_reference),
});

/** Like `Seller::cleaned`: every field one line but the legal information */
const cleanSeller = (seller: Seller): Seller => {
  return {
    name: cleanInvisibleLine(seller.name),
    address: cleanAddress(seller.address),
    vat_id: someLine(seller.vat_id),
    tax_number: someLine(seller.tax_number),
    contact_name: cleanInvisibleLine(seller.contact_name),
    phone: cleanInvisibleLine(seller.phone),
    email: cleanInvisibleLine(seller.email),
    mobile: someLine(seller.mobile),
    fax: someLine(seller.fax),
    website: someLine(seller.website),
    iban: cleanInvisibleLine(seller.iban),
    bic: someLine(seller.bic),
    account_holder: someLine(seller.account_holder),
    bank_name: someLine(seller.bank_name),
    legal_info: seller.legal_info === null ? null : cleanInvisible(seller.legal_info),
  };
};

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
    phone: 'as-typed',
    country: 'code',
    website_bare: false,
    date: 'DD.MM.YYYY',
    date_long: 'D. MMMM YYYY',
    title: '{title}',
    draft_title: '{draft_title}',
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
    tax_rate: null,
    tax_label: null,
    price: '95,00',
  },
  {
    id: 2,
    context_id: null,
    description: 'Reisekosten',
    quantity: null,
    unit: 'C62',
    unit_label: 'Stk.',
    tax_rate: '7',
    tax_label: 'Ermäßigt',
    price: '42,50',
  },
  {
    id: 3,
    context_id: null,
    description: 'Wartungsvertrag',
    quantity: '12',
    unit: 'MON',
    unit_label: 'Monat(e)',
    tax_rate: null,
    tax_label: null,
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

/** What every context starts with, the same as `rates::DEFAULTS` */
const DEFAULT_RATES: RateDef[] = [
  { label: 'Regelsatz', rate: '19' },
  { label: 'Ermäßigt', rate: '7' },
];

/** The rates of all contexts, in the order of their lists */
const rates: Rate[] = [
  ...DEFAULT_RATES.map(r => ({ ...r, context_id: 1 })),
  ...DEFAULT_RATES.map(r => ({ ...r, context_id: 2 })),
].map((r, i) => ({ ...r, id: i + 1 }));

const nextRateId = () => Math.max(0, ...rates.map(r => r.id as number)) + 1;

const ratesOf = (contextId: number) => rates.filter(r => r.context_id === contextId);

/** Mirrors `rates::canonical`: more than 0 and at most 100, comma or dot, no trailing zeros */
function canonicalRate(text: string | null | undefined): string | undefined {
  const value = (text ?? '').trim();
  const rate = value === '' ? Number.NaN : Number(value.replace(',', '.'));
  return rate > 0 && rate <= 100 ? String(rate) : undefined;
}

type Rated = Pick<Position, 'tax_rate' | 'tax_label'>;

const namesRate = (rate: Rate) => (p: Rated) =>
  canonicalRate(p.tax_rate) === rate.rate && (p.tax_label ?? '').trim() === rate.label;

/** The drafts of the context and the saved positions it sees, what a rate change reaches */
const reachedByRate = (rate: Rate) => {
  const names = namesRate(rate);
  return {
    templates: templates.filter(
      t => (t.context_id === null || t.context_id === rate.context_id) && names(t)
    ),
    drafts: invoices.filter(
      i => i.context_id === rate.context_id && i.status === 'draft' && i.draft.positions.some(names)
    ),
    names,
  };
};

const rateUsageOf = (rate: Rate): RateUsage => {
  const { templates: saved, drafts } = reachedByRate(rate);
  const context = contexts.find(c => c.id === rate.context_id);
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
    default: canonicalRate(context?.settings.tax_rate) === rate.rate,
  };
};

/** What names `from` names `to` instead: drafts, saved positions, and the default by its value */
function followRate(from: Rate, to: RateDef) {
  const { templates: saved, drafts, names } = reachedByRate(from);
  const point = (p: Rated) => {
    p.tax_rate = to.rate;
    p.tax_label = to.label;
  };
  saved.forEach(point);
  drafts.forEach(i => i.draft.positions.filter(names).forEach(point));
  const context = contexts.find(c => c.id === from.context_id);
  if (context && from.rate !== to.rate && canonicalRate(context.settings.tax_rate) === from.rate) {
    context.settings.tax_rate = to.rate;
  }
}

/** Mirrors `rates::check`: a label and a rate that nothing else in the list has */
function checkedRate(rate: RateDef, others: Rate[]): RateDef {
  const label = rate.label.trim();
  if (label === '') {
    throw 'Eine Bezeichnung ist erforderlich.';
  }
  const value = canonicalRate(rate.rate);
  if (value === undefined) {
    throw 'Der Steuersatz muss größer als 0 und höchstens 100 sein (in Prozent).';
  }
  if (others.some(o => o.label.trim().toLowerCase() === label.toLowerCase())) {
    throw `Die Bezeichnung „${label}“ gibt es schon.`;
  }
  if (others.some(o => o.rate === value)) {
    throw `Den Steuersatz ${value}% gibt es schon.`;
  }
  return { label, rate: value };
}

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
      tax_rate: null,
      tax_label: null,
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

/** A document that refers to no other, what every one starts as */
const UNRELATED = {
  relation: null,
  related_id: null,
  related_number: null,
  lapsed: false,
  superseded: false,
  cancelled: false,
  partial: false,
} as const;

const invoices: StoredInvoice[] = [
  {
    id: 1,
    context_id: 1,
    kind: 'invoice',
    number: 'RE-2026-001',
    status: 'exported',
    draft: { ...draftOf(customers[0] as Customer), frozen: settings },
    case_id: 1,
    total: '1130.50',
    exported_at: '2026-10-02 08:00:00',
    paid_at: null,
    payments: [],
    answer: null,
    answered_on: null,
    updated_at: '2026-10-02 08:00:00',
    archived_at: null,
    files: null,
    ...UNRELATED,
  },
  {
    id: 2,
    context_id: 1,
    kind: 'invoice',
    number: null,
    status: 'draft',
    draft: draftOf(customers[1] as Customer),
    case_id: 2,
    total: '1130.50',
    exported_at: null,
    paid_at: null,
    payments: [],
    answer: null,
    answered_on: null,
    updated_at: '2026-10-05 09:00:00',
    archived_at: null,
    files: null,
    ...UNRELATED,
  },
  // an offer that has run out, one still valid and a draft
  ...[
    { id: 3, number: 'AN-2026-001', issued: '2026-09-01', days: 30, customer: 0, case: 1 },
    { id: 4, number: 'AN-2026-002', issued: '2026-10-20', days: 30, customer: 1, case: 3 },
    { id: 5, number: null, issued: '2026-10-27', days: 30, customer: 0, case: 4 },
  ].map((o): StoredInvoice => ({
    id: o.id,
    context_id: 1,
    kind: 'offer',
    number: o.number,
    status: o.number ? 'exported' : 'draft',
    case_id: o.case,
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
    payments: [],
    answer: null,
    answered_on: null,
    updated_at: `${o.issued} 08:00:00`,
    archived_at: null,
    files: null,
    ...UNRELATED,
  })),
];

/** What happened to the mock's documents, as the store records it */
const events: DocumentEvent[] = invoices.flatMap((i): DocumentEvent[] => [
  ...(i.exported_at
    ? [
        {
          id: i.id * 100,
          invoice_id: i.id,
          kind: 'baked' as const,
          at: i.exported_at,
          on: i.draft.issue_date,
          number: i.number ?? undefined,
          has_snapshot: true,
        },
      ]
    : []),
]);

/** The versions kept with draft exports and baking, by event; the seeded ones fall back to the document */
const snapshots = new Map<number, InvoiceDraft>();

/**
 * Sample versions to compare: the offer AN-2026-001 went out as a draft once before it was created
 * at a lower price, the invoice draft went out once with fewer hours and another closing text
 */
[
  {
    id: 3,
    versions: [
      { at: '2026-08-28 10:00:00', kind: 'created' as const, hours: '8', price: '90,00' },
      { at: '2026-08-30 15:30:00', kind: 'draft-exported' as const, hours: '10', price: '90,00' },
    ],
  },
  {
    id: 2,
    versions: [
      { at: '2026-10-01 09:00:00', kind: 'created' as const, hours: '6', price: '95,00' },
      { at: '2026-10-03 11:15:00', kind: 'draft-exported' as const, hours: '8', price: '95,00' },
    ],
  },
].forEach(({ id, versions }) => {
  const document = invoices.find(i => i.id === id) as StoredInvoice;
  versions.forEach((v, index) => {
    const eventId = id * 100 + index + 1;
    events.push({
      id: eventId,
      invoice_id: id,
      kind: v.kind,
      at: v.at,
      on: null,
      has_snapshot: true,
      ...(v.kind === 'draft-exported' ? { version: index } : {}),
    });
    const [first, ...rest] = document.draft.positions;
    snapshots.set(eventId, {
      ...structuredClone(document.draft),
      frozen: null,
      closing: index === 0 ? 'Wir freuen uns auf Ihre Rückmeldung.' : document.draft.closing,
      positions: [{ ...(first as Position), quantity: v.hours, price: v.price }, ...rest],
    });
  });
});
// oldest first, as the store hands them out
events.sort((a, b) => a.at.localeCompare(b.at));

/** Notes an event now, with the version it keeps */
function event(
  id: number,
  kind: EventKind,
  on: string | null,
  detail: Partial<DocumentEvent> = {},
  snapshot?: InvoiceDraft
) {
  const eventId = Math.max(0, ...events.map(e => e.id)) + 1;
  events.push({
    id: eventId,
    invoice_id: id,
    kind,
    at: new Date().toISOString(),
    on,
    has_snapshot: snapshot !== undefined,
    ...detail,
  });
  if (snapshot) {
    snapshots.set(eventId, structuredClone({ ...snapshot, frozen: null }));
  }
}

/** Adds decimals as the store does, two places */
const sum = (amounts: string[]) => amounts.reduce((total, a) => total + Number(a), 0).toFixed(2);

function pay(stored: StoredInvoice, typed: string, paidOn: string) {
  // the store reads `1.130,50` and `1130.50` alike
  const amount = typed.includes(',') ? typed.replace(/\./g, '').replace(',', '.') : typed;
  const id = Math.max(0, ...invoices.flatMap(i => i.payments.map(p => p.id))) + 1;
  stored.payments = [...stored.payments, { id, amount, paid_on: paidOn }];
  event(stored.id, 'payment', paidOn, { amount });
}

/** Paid completely on the day the payments reached the total */
function settle(stored: StoredInvoice) {
  let total = 0;
  stored.paid_at = null;
  for (const payment of stored.payments) {
    total += Number(payment.amount);
    if (stored.paid_at === null && total >= Number(stored.total ?? 0)) {
      stored.paid_at = payment.paid_on;
    }
  }
}

/** A Vorgang as the mock keeps it, the rest is read from its documents */
interface CaseRow {
  id: number;
  context_id: number;
  number: string;
  customer_id: number | null;
  title: string;
  created_at: string;
  /** Started for its customer (`Neuer Vorgang`): the customer is locked from the start */
  started: boolean;
}

const caseNumber = (counter: number) => `VG-2026-${String(counter).padStart(3, '0')}`;

let mockCases: CaseRow[] = [1, 2, 3, 4].map(id => ({
  id,
  context_id: 1,
  number: caseNumber(id),
  customer_id: (invoices.find(i => i.case_id === id)?.draft.customer.id as number | null) ?? null,
  title: ['Website-Relaunch', 'Wartung 2026', 'Schulung', 'Badsanierung'][id - 1] ?? 'Vorgang',
  created_at: `2026-09-0${id} 08:00:00`,
  started: false,
}));

const caseRow = (id: number) => mockCases.find(c => c.id === id) as CaseRow;

/** As `cases::title_after`: a Vorgang made without a name is called after its first document */
const titleAfter = (kind: Kind, day: string) =>
  `${kind === 'offer' ? 'Angebot' : 'Rechnung'} vom ${day.split('-').reverse().join('.')}`;

/** As `cases::named`: a Vorgang always has a name */
const named = (title: unknown) => {
  const trimmed = String(title ?? '').trim();
  if (trimmed === '') {
    throw 'Ein Vorgang braucht eine Bezeichnung.';
  }
  return trimmed;
};

function newCase(contextId: number, customerId: number | null, title: string, started = false) {
  const id = Math.max(0, ...mockCases.map(c => c.id)) + 1;
  mockCases.push({
    id,
    context_id: contextId,
    number: caseNumber(id),
    customer_id: customerId,
    title: named(title),
    created_at: new Date().toISOString(),
    started,
  });
  return id;
}

/** As `cases::follow_customer`: a lone draft takes the Vorgang's customer along, a locked one refuses */
function followCustomer(caseId: number, customerId: number | null | undefined, isNew: boolean) {
  const row = caseRow(caseId);
  const documents = invoices.filter(i => i.case_id === caseId);
  const locked =
    row.started ||
    documents.length + (isNew ? 1 : 0) > 1 ||
    documents.some(i => i.status === 'exported');
  if ((customerId ?? null) === row.customer_id) {
    return;
  }
  if (row.customer_id !== null && locked) {
    throw 'Der Kunde des Vorgangs ist festgelegt, alle seine Dokumente gehen an ihn.';
  }
  row.customer_id = customerId ?? null;
}

/** Vorgänge that refer to each other, from the source to its follower (`case_links`) */
const caseLinks: { source: number; follower: number; link: CaseLink }[] = [];

/** The references of a Vorgang, its sources or its followers, as `case_links::refs` reads them */
const refsOf = (id: number, sources: boolean): CaseRef[] =>
  caseLinks
    .filter(l => (sources ? l.follower : l.source) === id)
    .map(l => {
      const other = sources ? l.source : l.follower;
      return { id: other, number: caseRow(other).number, link: l.link };
    });

/** As `cases::collected_parts`: the share of each invoice collecting a Vorgang that bills its offers, by net */
function collectedParts(id: number): { total: number; paid: number }[] {
  const offers = new Set(
    invoices.filter(i => i.case_id === id && i.kind === 'offer').map(i => i.id)
  );
  const net = (p: Position) => (number(p.quantity) || 1) * number(p.price);
  return invoices
    .filter(
      i =>
        i.kind === 'invoice' &&
        i.status === 'exported' &&
        !isCancelled(i.id) &&
        caseLinks.some(l => l.source === id && l.follower === i.case_id && l.link === 'collect')
    )
    .flatMap(i => {
      const all = i.draft.positions.reduce((s, p) => s + net(p), 0);
      const ours = i.draft.positions
        .filter(p => p.source && offers.has(p.source.offer_id))
        .reduce((s, p) => s + net(p), 0);
      const share = all === 0 ? 0 : ours / all;
      const paid = i.payments.reduce((t, p) => t + Number(p.amount), 0);
      return share === 0 ? [] : [{ total: Number(i.total ?? 0) * share, paid: paid * share }];
    });
}

/** What the store reads from a Vorgang's documents, and a collected one from the invoices collecting it */
function caseView(row: CaseRow): Case {
  const documents = invoices.filter(i => i.case_id === row.id);
  const created = documents.filter(i => i.status === 'exported');
  const total = (list: StoredInvoice[]) => list.reduce((s, i) => s + Number(i.total ?? 0), 0);
  const offers = created.filter(i => i.kind === 'offer');
  // a cancelled invoice and its Storno bill nothing (`Standing.settled`)
  const settled = (i: StoredInvoice) => isCancelled(i.id) || i.draft.corrects?.kind === 'cancel';
  const billing = created.filter(i => i.kind === 'invoice' && !settled(i));
  const bills = [
    ...created
      .filter(i => i.kind === 'invoice')
      .map(i => ({
        total: Number(i.total ?? 0),
        paid: i.payments.reduce((t, p) => t + Number(p.amount), 0),
      })),
    ...collectedParts(row.id),
  ];
  const invoiced = bills.reduce((s, b) => s + b.total, 0);
  const paid = bills.reduce((s, b) => s + b.paid, 0);
  const open = Math.max(0, invoiced - paid);
  const drafted = (kind: Kind) => documents.some(i => i.kind === kind && i.status === 'draft');
  // as `Standing.due`: by the payment days it was made with, 14 in the mock
  const due = (i: StoredInvoice) => {
    const day = new Date(`${i.draft.issue_date}T12:00:00Z`);
    day.setUTCDate(day.getUTCDate() + (i.draft.frozen?.payment_days ?? 14));
    return day.toISOString().slice(0, 10);
  };
  const today = new Date().toISOString().slice(0, 10);
  const overdue = billing.some(
    i =>
      i.payments.reduce((t, p) => t + Number(p.amount), 0) < Number(i.total ?? 0) && due(i) < today
  );
  const accepted = offers
    .filter(o => o.answer === 'accepted')
    .reduce((s, o) => s + Number(o.total ?? 0), 0);
  const leftToBill = Math.round((accepted - invoiced) * 100) > 0;
  // the furthest step, as `cases::status`
  const stage: CaseStage =
    billing.length + collectedParts(row.id).length > 0
      ? overdue
        ? 'overdue'
        : open > 0 && paid === 0
          ? 'invoiced'
          : open > 0
            ? 'partly-paid'
            : leftToBill
              ? 'partly-invoiced'
              : 'paid'
      : offers.some(o => o.answer === 'accepted')
        ? 'accepted'
        : offers.length > 0
          ? offers.every(o => o.answer === 'declined')
            ? 'declined'
            : 'offered'
          : created.some(i => i.kind === 'invoice')
            ? 'cancelled'
            : drafted('invoice')
              ? 'drafting-invoice'
              : drafted('offer')
                ? 'drafting-offer'
                : 'new';
  return {
    ...row,
    customer_name:
      documents[0]?.draft.customer.name ??
      customers.find(c => c.id === row.customer_id)?.name ??
      '',
    locked: row.started || documents.length > 1 || created.length > 0,
    documents: documents.length,
    stage,
    offered: total(offers.filter(o => o.answer !== 'declined')).toFixed(2),
    accepted: total(offers.filter(o => o.answer === 'accepted')).toFixed(2),
    invoiced: invoiced.toFixed(2),
    paid: paid.toFixed(2),
    open: open.toFixed(2),
    archived: documents.length > 0 && documents.every(i => i.archived_at !== null),
    sources: refsOf(row.id, true),
    followers: refsOf(row.id, false),
  };
}

/** When the mock was last backed up, none at first */
/** Folders of an interrupted restore, none until a test puts some here */
let interrupted: string[] = [];
let lastBackup: string | null = null;

/** As `billing::partial`: an invoice billing only part of an offer it refers to */
function partial(draft: InvoiceDraft): boolean {
  if (draft.on_account || isClosing(draft) || draft.corrects) {
    return false;
  }
  const offers = new Set(draft.positions.flatMap(p => (p.source ? [p.source.offer_id] : [])));
  return [...offers].some(id =>
    (invoices.find(o => o.id === id && o.kind === 'offer')?.draft.positions ?? []).some(
      (position, index) =>
        draft.positions
          .filter(p => p.source?.offer_id === id && p.source.index === index)
          .reduce((sum, p) => sum + (number(p.quantity) || 1), 0) < (number(position.quantity) || 1)
    )
  );
}

/**
 * What the store refuses on saving a draft into a Vorgang, with its messages: `check_sources`,
 * `check_deductions`, `check_on_account` and `check_correction`
 */
function checkSave(caseId: number, id: number | null, kind: Kind, draft: InvoiceDraft) {
  const bills = (offerCase: number) =>
    offerCase === caseId ||
    caseLinks.some(l => l.follower === caseId && l.source === offerCase && l.link === 'collect');
  draft.positions.forEach(({ source }) => {
    if (!source) {
      return;
    }
    if (kind !== 'invoice') {
      throw 'Nur die Positionen einer Rechnung beziehen sich auf ein Angebot.';
    }
    const offer = invoices.find(i => i.id === source.offer_id);
    if (!offer) {
      throw 'Das Angebot einer Position gibt es nicht.';
    }
    if (offer.kind !== 'offer' || offer.status !== 'exported' || !bills(offer.case_id)) {
      throw 'Eine Position bezieht sich nur auf ein erstelltes Angebot desselben Vorgangs oder eines, den er zusammenfasst.';
    }
    if (source.index >= offer.draft.positions.length) {
      throw 'Die Position des Angebots gibt es nicht.';
    }
  });
  if (draft.on_account && draft.positions.some(p => p.source)) {
    throw 'Eine Abschlags- oder Anzahlungsrechnung berechnet keine Angebotsposition endgültig.';
  }
  const deductions = draft.deductions ?? [];
  const cancels = draft.corrects?.kind === 'cancel';
  if (deductions.length > 0 && (kind !== 'invoice' || draft.on_account)) {
    throw 'Nur eine Schlussrechnung zieht Abschlags- und Anzahlungsrechnungen ab.';
  }
  if (!cancels) {
    deductions.forEach((d, i) => {
      if (deductions.findIndex(o => o.invoice_id === d.invoice_id) !== i) {
        throw 'Eine Abschlagsrechnung wird nur einmal abgezogen.';
      }
      const deducted = invoices.find(o => o.id === d.invoice_id);
      if (
        !deducted ||
        deducted.case_id !== caseId ||
        !onAccountOf(deducted) ||
        isCancelled(deducted.id)
      ) {
        throw 'Abgezogen werden nur erstellte Abschlags- und Anzahlungsrechnungen desselben Vorgangs.';
      }
      const elsewhere = invoices.some(
        o =>
          o.status === 'exported' &&
          o.id !== id &&
          o.draft.corrects?.kind !== 'cancel' &&
          !isCancelled(o.id) &&
          (o.draft.deductions ?? []).some(x => x.invoice_id === d.invoice_id)
      );
      if (elsewhere) {
        throw `Die Abschlagsrechnung ${deducted.number ?? ''} zieht schon eine andere Schlussrechnung ab.`;
      }
    });
  }
  const correction = draft.corrects;
  if (!correction) {
    return;
  }
  const original = invoices.find(i => i.id === correction.invoice_id);
  if (kind !== 'invoice') {
    throw 'Nur eine Rechnung wird storniert oder ersetzt.';
  }
  if (
    !original ||
    original.kind !== 'invoice' ||
    original.status !== 'exported' ||
    original.case_id !== caseId
  ) {
    throw 'Storniert oder ersetzt wird eine erstellte Rechnung desselben Vorgangs.';
  }
  if (original.draft.corrects?.kind === 'cancel') {
    throw 'Eine Stornorechnung wird nicht storniert.';
  }
  if (cancels) {
    if (
      invoices.some(
        s =>
          s.id !== id &&
          s.status === 'exported' &&
          s.draft.corrects?.kind === 'cancel' &&
          s.draft.corrects.invoice_id === original.id
      )
    ) {
      throw 'Diese Rechnung ist schon storniert.';
    }
    // the parts that matter, the mock keeps no field order
    const position = (p: Position) => [
      p.description,
      p.quantity?.trim() || '1',
      p.price,
      p.tax_rate,
      p.unit,
    ];
    const negated = (q: string | null) => {
      const typed = q?.trim() || '1';
      return typed.startsWith('-') ? typed.slice(1) : `-${typed}`;
    };
    const mirrors =
      draft.customer.id === original.draft.customer.id &&
      JSON.stringify(draft.positions.map(position)) ===
        JSON.stringify(
          original.draft.positions.map(p => position({ ...p, quantity: negated(p.quantity) }))
        ) &&
      JSON.stringify(deductions.map(d => d.invoice_id)) ===
        JSON.stringify((original.draft.deductions ?? []).map(d => d.invoice_id)) &&
      draft.delivery_date === original.draft.delivery_date &&
      draft.delivery_end === original.draft.delivery_end;
    if (!mirrors) {
      throw 'Eine Stornorechnung gibt die Rechnung so wieder, wie sie war.';
    }
  }
}

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
  // a position's own rate, else the default; no VAT at all ignores them
  const fallback = Number(settings.tax_rate.replace(',', '.')) || 0;
  const vat = draft.positions.reduce((sum, p) => {
    const own = settings.tax_rate.trim() === '' ? undefined : canonicalRate(p.tax_rate);
    return sum + ((number(p.quantity) || 1) * number(p.price) * (Number(own) || fallback)) / 100;
  }, 0);
  // a Schlussrechnung takes off what its invoices on account came to, its Storno takes that back
  const sign = draft.corrects?.kind === 'cancel' ? -1 : 1;
  const deducted =
    sign * (draft.deductions ?? []).reduce((sum, d) => sum + Number(d.gross || 0), 0);
  return {
    net: euro(net),
    vat: euro(vat),
    gross: euro(net + vat),
    due: euro(net + vat - deducted),
  };
}

/** A created invoice on account, with what it came to (`on_account::figures`) */
function onAccountOf(i: StoredInvoice): OnAccountInvoice | undefined {
  if (i.kind !== 'invoice' || i.status !== 'exported' || !i.draft.on_account || !i.number) {
    return undefined;
  }
  const { net, vat, gross } = totals(i.draft);
  return {
    id: i.id,
    number: i.number,
    issue_date: i.draft.issue_date,
    on_account: i.draft.on_account,
    net,
    vat,
    gross,
    deducted_by: null,
    paid: i.payments.length > 0 ? sum(i.payments.map(p => p.amount)) : null,
    paid_at: i.paid_at,
  };
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
    if ((p.tax_rate ?? '').trim() !== '' && canonicalRate(p.tax_rate) === undefined) {
      problems.push({
        field: `positions[${i}].tax_rate`,
        message: `Der Steuersatz „${p.tax_rate}“ ist nicht zulässig`,
      });
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

// the first invoice billed the first position of the first offer
const [billedFirst] = find(1).draft.positions;
if (billedFirst) {
  billedFirst.source = { offer_id: 3, index: 0 };
}

/** The Stornorechnung of an invoice, created or drafted */
const cancelling = (id: number, status: Status) =>
  invoices.find(
    s =>
      s.status === status &&
      s.draft.corrects?.kind === 'cancel' &&
      s.draft.corrects.invoice_id === id
  );

const isCancelled = (id: number) => cancelling(id, 'exported') !== undefined;

/** The invoice positions that refer to offer positions, as `billing::references` reads them */
const references = () =>
  invoices
    .filter(i => i.kind === 'invoice' && !isCancelled(i.id))
    .flatMap(invoice =>
      invoice.draft.positions.flatMap(p =>
        p.source ? [{ invoice, source: p.source, quantity: quantityOf(p) }] : []
      )
    );

/** A document as the other end of a reference names it */
const linkedOf = (i: StoredInvoice): Linked => ({
  id: i.id,
  kind: i.kind,
  number: i.number,
  status: i.status,
  customer_name: i.draft.customer.name,
  issue_date: i.draft.issue_date,
});

/** The documents of the other kind a document is billed with, newest first (`Store::links`) */
function linksOf(id: number): Linked[] {
  const others = new Set(
    references().flatMap(r =>
      r.invoice.id === id ? [r.source.offer_id] : r.source.offer_id === id ? [r.invoice.id] : []
    )
  );
  return [...others]
    .map(other => linkedOf(find(other)))
    .sort((a, b) => b.issue_date.localeCompare(a.issue_date) || b.id - a.id);
}

/** What is billed of a created offer (`Store::billing`) */
function billingOf(offerId: number, except: number | null): OfferBilling {
  const offer = find(offerId);
  const refs = references().filter(r => r.source.offer_id === offerId && r.invoice.id !== except);
  const sum = (index: number, created: boolean) =>
    refs
      .filter(r => r.source.index === index && (r.invoice.status === 'exported') === created)
      .reduce((total, r) => total + r.quantity, 0);
  return {
    offer_id: offerId,
    number: offer.number ?? '',
    positions: offer.draft.positions.map((position, index) => {
      const billed = sum(index, true);
      return {
        index,
        position,
        billed: String(billed),
        drafted: String(sum(index, false)),
        open: String(Math.max(0, quantityOf(position) - billed)),
      };
    }),
    invoices: linksOf(offerId).filter(l => l.kind === 'invoice'),
  };
}

/** The offer a group of alternatives is headed by; every other offer heads its own */
const groupOf = (i: StoredInvoice) => (i.relation === 'alternative' ? i.related_id : i.id);

/** What the relations say of a document, read from the others as `relations.rs` does */
function related(i: StoredInvoice): StoredInvoice {
  const taken =
    i.kind === 'offer'
      ? invoices.find(m => m.id !== i.id && m.answer === 'accepted' && groupOf(m) === groupOf(i))
      : undefined;
  const replacement = invoices.find(
    r => r.related_id === i.id && r.relation === 'replacement' && r.status === 'exported'
  );
  const cancelled = cancelling(i.id, 'exported');
  const ref = (d: StoredInvoice | undefined) => (d ? { id: d.id, number: d.number } : null);
  return {
    ...i,
    related_number: i.related_id === null ? null : find(i.related_id).number,
    lapsed: i.status === 'exported' && i.answer === null && taken !== undefined,
    taken_by: ref(taken),
    superseded: replacement !== undefined,
    superseded_by: ref(replacement),
    cancelled: cancelled !== undefined,
    cancelled_by: ref(cancelled),
    cancel_draft: cancelling(i.id, 'draft')?.id ?? null,
    partial: partial(i.draft),
  };
}

/** A document as a row of a list reads it (`ListEntry`) */
const entryOf = (stored: StoredInvoice): ListEntry => {
  const i = related(stored);
  return {
    id: i.id,
    context_id: i.context_id,
    kind: i.kind,
    valid_until: i.kind === 'offer' ? validUntil(i.draft.issue_date, i.draft.validity_days) : null,
    seller_name: sellerOf(i.context_id),
    number: i.number,
    status: i.status,
    customer_name: i.draft.customer.name,
    issue_date: i.draft.issue_date,
    total: i.total,
    paid_at: i.paid_at,
    paid: i.payments.length > 0 ? sum(i.payments.map(p => p.amount)) : null,
    answer: i.answer,
    answered_on: i.answered_on,
    updated_at: i.updated_at,
    archived_at: i.archived_at,
    case_id: i.case_id,
    on_account: i.draft.on_account ?? null,
    closing: isClosing(i.draft),
    partial: i.partial,
    corrects: i.draft.corrects ?? null,
    cancelled: i.cancelled,
    cancelled_by: i.cancelled_by,
    cancel_draft: i.cancel_draft,
    relation: i.relation,
    related_id: i.related_id,
    related_number: i.related_number,
    lapsed: i.lapsed,
    taken_by: i.taken_by,
    superseded: i.superseded,
    superseded_by: i.superseded_by,
  };
};

/** As `Store::case_graph`: the documents of a Vorgang, how they relate, the other Vorgänge on the other end */
function caseGraph(id: number): CaseGraph {
  const own = invoices.filter(i => i.case_id === id);
  const edgesOf = (i: StoredInvoice): GraphEdge[] => {
    const edge = (from: number, kind: EdgeKind): GraphEdge => ({ from, to: i.id, kind });
    return [
      ...(i.relation && i.related_id !== null ? [edge(i.related_id, i.relation)] : []),
      ...i.draft.positions.flatMap(p => [
        ...(p.source ? [edge(p.source.offer_id, 'bills')] : []),
        ...(p.share ? [edge(p.share.offer_id, 'share')] : []),
      ]),
      ...(i.draft.deductions ?? []).map(d => edge(d.invoice_id, 'deducts')),
      ...(i.draft.corrects
        ? [
            edge(
              i.draft.corrects.invoice_id,
              i.draft.corrects.kind === 'cancel' ? 'cancels' : 'replaces'
            ),
          ]
        : []),
    ];
  };
  const offers = new Set(own.filter(i => i.kind === 'offer').map(i => i.id));
  // a Sammelrechnung of another Vorgang billing its offers
  const incoming = invoices
    .filter(i => i.case_id !== id)
    .flatMap(i => edgesOf(i).filter(e => e.kind === 'bills' && offers.has(e.from)));
  const unique = new Map(
    [...own.flatMap(edgesOf), ...incoming].map(e => [`${e.from}:${e.to}:${e.kind}`, e])
  );
  const ids = new Set([
    ...own.map(i => i.id),
    ...[...unique.values()].flatMap(e => [e.from, e.to]),
  ]);
  const documents = invoices
    .filter(i => ids.has(i.id))
    .map(entryOf)
    .sort((a, b) => a.issue_date.localeCompare(b.issue_date) || a.id - b.id);
  const others = [...new Set(documents.filter(d => d.case_id !== id).map(d => d.case_id))];
  return {
    documents,
    edges: [...unique.values()].filter(e => ids.has(e.from) && ids.has(e.to)),
    others: others.map(other => ({ id: other, number: caseRow(other).number })),
  };
}

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
          const given = args['context'] as Context;
          const context = {
            ...given,
            settings: { ...given.settings, seller: cleanSeller(given.settings.seller) },
          };
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
            const wanted = (args['rates'] as RateDef[] | null) ?? [];
            const start: RateDef[] = [];
            [...(wanted.length > 0 ? wanted : DEFAULT_RATES)].forEach(r =>
              start.push(
                checkedRate(
                  r,
                  start.map(s => ({ ...s, id: null, context_id: 0 }))
                )
              )
            );
            // the default is one of the list, whatever list the context was handed
            const charged = canonicalRate(saved.settings.tax_rate);
            if (charged !== undefined && !start.some(r => r.rate === charged)) {
              const taken = start.some(r => r.label.toLowerCase() === 'steuersatz');
              start.push({
                label: taken ? `Steuersatz ${charged}%` : 'Steuersatz',
                rate: charged,
              });
            }
            start.forEach(r =>
              rates.push({ ...r, id: nextRateId(), context_id: saved.id as number })
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
          rates.splice(0, rates.length, ...rates.filter(r => r.context_id !== args['id']));
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
          const customer = cleanCustomer(args['customer'] as Customer);
          if (customer.id === null) {
            customers.push({ ...customer, id: customers.length + 1 });
          } else {
            customers[customers.findIndex(c => c.id === customer.id)] = customer;
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
          // a saved position bills no offer
          const template = { ...(args['template'] as PositionTemplate), source: null };
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
        // the browser has no Finder to show a path in
        case 'plugin:opener|reveal_item_in_dir':
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
          // the default unit goes by its label
          const owner = contexts.find(c => c.id === context);
          if (owner?.settings.unit === old.label) {
            owner.settings.unit = def.label;
          }
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
          // no replacement for a default: the first unit is it then
          const owner = contexts.find(c => c.id === unit.context_id);
          if (owner?.settings.unit === unit.label) {
            owner.settings.unit = null;
          }
          return null;
        }
        case 'unit_default': {
          const label = args['label'] as string | null;
          if (label !== null && !unitsOf(args['contextId']).some(u => u.label === label)) {
            throw 'Diese Einheit gibt es nicht.';
          }
          (contexts.find(c => c.id === args['contextId']) as Context).settings.unit = label;
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
        case 'rates_list':
          return ratesOf(args['contextId']).map(r => ({ ...r }));
        case 'rates_counts':
          return Object.fromEntries(
            ratesOf(args['contextId']).map(r => {
              const usage = rateUsageOf(r);
              return [r.id, usage.templates.length + usage.drafts.length];
            })
          );
        case 'rate_save': {
          const rate = args['rate'] as Rate;
          const before = rate.id === null ? undefined : rates.find(r => r.id === rate.id);
          const context = before?.context_id ?? rate.context_id;
          const def = checkedRate(
            rate,
            ratesOf(context).filter(r => r.id !== rate.id)
          );
          if (!before) {
            const added = { ...def, id: nextRateId(), context_id: context };
            rates.push(added);
            return { ...added };
          }
          const old = { ...before };
          Object.assign(before, def);
          if (old.rate !== def.rate || old.label !== def.label) {
            followRate(old, def);
          }
          return { ...before };
        }
        case 'rate_usage':
          return rateUsageOf(rates.find(r => r.id === args['id']) as Rate);
        case 'rate_delete': {
          const rate = rates.find(r => r.id === args['id']) as Rate;
          const target = rates.find(r => r.id === args['replacement']);
          const usage = rateUsageOf(rate);
          if (usage.templates.length + usage.drafts.length > 0 || usage.default) {
            if (!target || target.context_id !== rate.context_id || target === rate) {
              throw 'Der Steuersatz wird noch verwendet, bitte einen Ersatz wählen.';
            }
            followRate(rate, target);
          }
          rates.splice(rates.indexOf(rate), 1);
          return null;
        }
        case 'rate_default': {
          const rate = String(args['rate']).trim();
          const listed = ratesOf(args['contextId']).find(r => r.rate === canonicalRate(rate));
          if (rate !== '' && !listed) {
            throw 'Diesen Steuersatz gibt es nicht.';
          }
          (contexts.find(c => c.id === args['contextId']) as Context).settings.tax_rate =
            listed?.rate ?? '';
          return null;
        }
        case 'rates_reorder': {
          const ids = args['ids'] as number[];
          const own = ratesOf(args['contextId']);
          rates.splice(
            0,
            rates.length,
            ...rates.filter(r => r.context_id !== args['contextId']),
            ...ids.map(id => own.find(r => r.id === id) as Rate)
          );
          return null;
        }
        case 'invoice_draft_pdf': {
          if (args['id'] !== null) {
            const version = events.filter(
              e => e.invoice_id === args['id'] && e.kind === 'draft-exported'
            ).length;
            event(
              args['id'] as number,
              'draft-exported',
              null,
              { version: version + 1 },
              args['draft'] as InvoiceDraft
            );
          }
          return null;
        }
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
            .filter(i => args['kind'] == null || i.kind === args['kind'])
            .filter(i => args['contextId'] == null || i.context_id === args['contextId'])
            .filter(i => i.archived_at === null || Boolean(args['withArchived']))
            .map(entryOf);
        case 'invoice_archive':
          find(args['id']).archived_at = new Date().toISOString();
          event(args['id'] as number, 'archived', null);
          return null;
        case 'invoice_unarchive': {
          const stored = find(args['id']);
          stored.archived_at = null;
          event(stored.id, 'restored', null);
          return { ...stored };
        }
        case 'invoice_set_paid': {
          const stored = find(args['id']);
          const paidOn = (args['paidAt'] as string | null) ?? null;
          if (paidOn === null) {
            stored.payments.forEach(p =>
              event(stored.id, 'payment-removed', p.paid_on, { amount: p.amount })
            );
            stored.payments = [];
          } else {
            const open =
              Number(stored.total ?? 0) - Number(sum(stored.payments.map(p => p.amount)));
            if (open > 0) {
              pay(stored, open.toFixed(2), paidOn);
            }
          }
          settle(stored);
          return { ...stored };
        }
        case 'invoice_add_payment': {
          const stored = find(args['id']);
          pay(stored, String(args['amount']), String(args['paidOn']));
          settle(stored);
          return { ...stored };
        }
        case 'invoice_remove_payment': {
          const stored = invoices.find(i => i.payments.some(p => p.id === args['paymentId']));
          if (!stored) {
            throw new Error('unknown payment');
          }
          const payment = stored.payments.find(p => p.id === args['paymentId']);
          stored.payments = stored.payments.filter(p => p !== payment);
          event(stored.id, 'payment-removed', payment?.paid_on ?? null, {
            amount: payment?.amount,
          });
          settle(stored);
          return { ...stored };
        }
        case 'offer_set_answer': {
          const stored = find(args['id']);
          const answer = (args['answer'] as Answer | null) ?? null;
          const on = (args['on'] as string | null) ?? null;
          // mirrors `Store::may_accept`
          const standing = related(stored);
          if (answer === 'accepted' && standing.superseded) {
            throw 'Ein ersetztes Angebot wird nicht angenommen, sein Ersatzangebot schon.';
          }
          if (
            answer === 'accepted' &&
            invoices.some(
              m => m.id !== stored.id && m.answer === 'accepted' && groupOf(m) === groupOf(stored)
            )
          ) {
            throw 'Ein anderes Angebot dieser Gruppe ist schon angenommen.';
          }
          if (answer !== null || stored.answer !== null) {
            event(stored.id, answer ?? 'answer-removed', answer ? on : null);
          }
          stored.answer = answer;
          stored.answered_on = answer ? on : null;
          return related(stored);
        }
        case 'invoice_events':
          return events.filter(e => e.invoice_id === args['id']);
        case 'invoice_snapshot': {
          const id = args['eventId'] as number;
          const found = events.find(e => e.id === id);
          return snapshots.get(id) ?? find(found?.invoice_id ?? 0).draft;
        }
        case 'invoice_get':
          return related(find(args['id']));
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
          // the figures of what it deducts come from the invoices on account (`fill_deductions`)
          draft.deductions = (draft.deductions ?? []).map(d => {
            const found = onAccountOf(find(d.invoice_id));
            return found ? { ...d, ...found, invoice_id: d.invoice_id } : d;
          });
          const existing = args['id'] === null ? undefined : find(args['id']);
          const into = (args['caseId'] as number | null | undefined) ?? null;
          const context =
            into === null ? ((args['contextId'] as number | null) ?? 1) : caseRow(into).context_id;
          const stored: StoredInvoice = existing ?? {
            id: nextId(),
            case_id:
              into ??
              newCase(
                context,
                draft.customer.id,
                titleAfter(args['kind'] as Kind, draft.issue_date)
              ),
            context_id: context,
            kind: args['kind'] as Kind,
            number: null,
            status: 'draft',
            draft,
            total: null,
            exported_at: null,
            paid_at: null,
            payments: [],
            answer: null,
            answered_on: null,
            updated_at: '',
            archived_at: null,
            files: null,
            ...UNRELATED,
          };
          followCustomer(stored.case_id, draft.customer.id, existing === undefined);
          checkSave(stored.case_id, existing?.id ?? null, stored.kind, draft);
          stored.draft = draft;
          // what is due, a Stornorechnung's negative
          stored.total = check(draft).totals?.due ?? null;
          if (!existing) {
            invoices.push(stored);
            event(stored.id, 'created', null, {}, draft);
          }
          return stored;
        }
        case 'invoice_cancel':
        case 'invoice_replace': {
          // mirrors `Store::cancel_invoice` and `Store::replace_invoice`
          const kind = command === 'invoice_cancel' ? 'cancel' : 'replace';
          const original = find(args['id'] as number);
          if (kind === 'cancel' && isCancelled(original.id)) {
            throw 'Diese Rechnung ist schon storniert.';
          }
          // one Storno draft at a time, it is opened again
          const drafted = related(original).cancel_draft;
          if (kind === 'cancel' && drafted) {
            return related(find(drafted));
          }
          if (kind === 'replace' && !isCancelled(original.id)) {
            throw 'Ersetzt wird eine stornierte Rechnung; erst die Stornorechnung erstellen.';
          }
          const draft: InvoiceDraft = {
            ...structuredClone(original.draft),
            issue_date: today(),
            frozen: null,
            corrects: {
              invoice_id: original.id,
              kind,
              number: original.number ?? '',
              issue_date: original.draft.issue_date,
            },
          };
          if (kind === 'cancel') {
            // quantities taken back, deductions kept (`corrections::mirrored`)
            draft.positions = draft.positions.map(p => {
              const typed = p.quantity?.trim() || '1';
              const quantity = typed.startsWith('-') ? typed.slice(1) : `-${typed}`;
              return { ...p, quantity, source: null };
            });
            draft.on_account = null;
          }
          const made: StoredInvoice = {
            ...structuredClone(original),
            ...UNRELATED,
            id: nextId(),
            number: null,
            status: 'draft',
            draft,
            total: check(draft).totals?.due ?? null,
            exported_at: null,
            paid_at: null,
            payments: [],
            archived_at: null,
            files: null,
          };
          invoices.push(made);
          event(made.id, 'created', null, { from: original.id }, made.draft);
          return made;
        }
        case 'offer_follow': {
          // mirrors `Store::follow_offer`: a copy in a new Vorgang following the offer's
          const source = find(args['id'] as number);
          if (source.kind !== 'offer' || source.status !== 'exported') {
            throw 'Ein Folgeangebot gibt es nur zu einem erstellten Angebot.';
          }
          const copy: StoredInvoice = {
            ...structuredClone(source),
            ...UNRELATED,
            id: nextId(),
            case_id: newCase(
              source.context_id,
              source.draft.customer.id,
              caseRow(source.case_id).title
            ),
            number: null,
            status: 'draft',
            exported_at: null,
            answer: null,
            answered_on: null,
            archived_at: null,
            files: null,
          };
          copy.draft.issue_date = today();
          copy.draft.frozen = null;
          invoices.push(copy);
          caseLinks.push({ source: source.case_id, follower: copy.case_id, link: 'follow' });
          event(copy.id, 'created', null, { from: source.id }, copy.draft);
          return copy;
        }
        case 'cases_collect': {
          // mirrors `Store::collect`
          const ids = args['ids'] as number[];
          const rows = ids.map(caseRow);
          const [first] = rows;
          if (!first || rows.length < 2) {
            throw 'Eine Sammelrechnung fasst mindestens zwei Vorgänge zusammen.';
          }
          if (ids.some(id => caseLinks.some(l => l.follower === id && l.link === 'collect'))) {
            throw 'Ein Vorgang, der andere zusammenfasst, wird nicht noch einmal gesammelt.';
          }
          if (first.customer_id === null || rows.some(r => r.customer_id !== first.customer_id)) {
            throw 'Eine Sammelrechnung fasst Vorgänge desselben Kunden zusammen.';
          }
          const offers = invoices
            .filter(i => ids.includes(i.case_id) && i.kind === 'offer' && i.status === 'exported')
            .map(related)
            .filter(o => o.answer !== 'declined' && !o.lapsed && !o.superseded);
          const positions = offers.flatMap(o => {
            const billing = billingOf(o.id, null);
            return billing.positions
              .filter(b => Number(b.open) > 0)
              .map(b => billedPosition(billing, b.index, b.open));
          });
          const [offer] = offers;
          if (!offer || positions.length === 0) {
            throw 'In diesen Vorgängen ist nichts mehr offen.';
          }
          const caseId = newCase(first.context_id, first.customer_id, named(args['title']), true);
          ids.forEach(source => caseLinks.push({ source, follower: caseId, link: 'collect' }));
          const draft: InvoiceDraft = {
            ...structuredClone(offer.draft),
            issue_date: today(),
            delivery_date: today(),
            delivery_end: null,
            validity_days: null,
            positions,
            frozen: null,
          };
          const invoice: StoredInvoice = {
            ...structuredClone(offer),
            ...UNRELATED,
            id: nextId(),
            case_id: caseId,
            kind: 'invoice',
            number: null,
            status: 'draft',
            draft,
            total: check(draft).totals?.due ?? null,
            exported_at: null,
            answer: null,
            answered_on: null,
            archived_at: null,
            files: null,
          };
          invoices.push(invoice);
          event(invoice.id, 'created', null, {}, invoice.draft);
          return invoice;
        }
        case 'offer_related': {
          // mirrors `Store::related_offer`
          const source = related(find(args['id'] as number));
          const relation = args['relation'] as Relation;
          if (source.kind !== 'offer' || source.status !== 'exported') {
            throw 'Nachtrag, Alternative und Ersatz gibt es nur zu einem erstellten Angebot.';
          }
          if (relation === 'supplement' && source.answer === 'declined') {
            throw 'Ein abgelehntes Angebot bekommt keinen Nachtrag.';
          }
          const taken = invoices.some(
            m => m.id !== source.id && m.answer === 'accepted' && groupOf(m) === groupOf(source)
          );
          if (relation === 'alternative' && (source.answer === 'accepted' || taken)) {
            throw 'Ein Angebot dieser Gruppe ist schon angenommen, eine Alternative kommt zu spät.';
          }
          if (relation === 'replacement' && source.answer === 'accepted') {
            throw 'Ein angenommenes Angebot wird nicht ersetzt.';
          }
          const target =
            relation !== 'replacement' && source.relation === relation && source.related_id !== null
              ? source.related_id
              : source.id;
          const own = (contexts.find(c => c.id === source.context_id) as Context).settings;
          const draft: InvoiceDraft =
            relation === 'supplement'
              ? {
                  ...draftOf(source.draft.customer),
                  buyer_reference: source.draft.buyer_reference,
                  validity_days: own.offer.validity_days,
                  layout: own.offer.layout || own.layout,
                  intro: own.offer.intro ?? '',
                  closing: own.offer.closing ?? '',
                  offer_reference: false,
                }
              : structuredClone(source.draft);
          draft.issue_date = today();
          draft.delivery_date = today();
          draft.delivery_end = null;
          draft.frozen = null;
          const offer: StoredInvoice = {
            ...structuredClone(source),
            id: nextId(),
            number: null,
            status: 'draft',
            draft,
            total: null,
            exported_at: null,
            paid_at: null,
            payments: [],
            answer: null,
            answered_on: null,
            archived_at: null,
            updated_at: new Date().toISOString(),
            files: null,
            ...UNRELATED,
            relation,
            related_id: target,
            related_number: find(target).number,
          };
          invoices.push(offer);
          event(offer.id, 'created', null, { from: source.id }, offer.draft);
          return offer;
        }
        case 'invoice_duplicate': {
          const copy: StoredInvoice = {
            ...structuredClone(find(args['id'])),
            id: nextId(),
            case_id: args['sameCase']
              ? find(args['id']).case_id
              : newCase(
                  find(args['id']).context_id,
                  find(args['id']).draft.customer.id,
                  caseRow(find(args['id']).case_id).title
                ),
            number: null,
            status: 'draft',
            exported_at: null,
            paid_at: null,
            payments: [],
            answer: null,
            answered_on: null,
            archived_at: null,
            ...UNRELATED,
          };
          copy.draft.issue_date = today();
          copy.draft.delivery_date = today();
          copy.draft.delivery_end = copy.draft.delivery_end === null ? null : today();
          copy.draft.frozen = null;
          // a copy bills nothing again
          copy.draft.positions = copy.draft.positions.map(p => ({ ...p, source: null }));
          invoices.push(copy);
          event(copy.id, 'created', null, { from: args['id'] as number }, copy.draft);
          return copy;
        }
        case 'invoice_delete': {
          const gone = find(args['id']).case_id;
          invoices.splice(
            invoices.findIndex(i => i.id === args['id']),
            1
          );
          if (!invoices.some(i => i.case_id === gone)) {
            mockCases = mockCases.filter(c => c.id !== gone);
          }
          return null;
        }
        case 'cases_list':
          return mockCases
            .filter(c => args['contextId'] == null || c.context_id === args['contextId'])
            .map(caseView)
            .filter(c => Boolean(args['withArchived']) || !c.archived)
            .reverse();
        case 'case_get':
          return caseView(caseRow(args['id'] as number));
        case 'case_create':
          return caseView(
            caseRow(
              newCase(
                args['contextId'] as number,
                (args['customerId'] as number | null) ?? null,
                named(args['title']),
                args['customerId'] != null
              )
            )
          );
        case 'case_set_title': {
          const row = caseRow(args['id'] as number);
          row.title = named(args['title']);
          return caseView(row);
        }
        case 'case_delete':
          if (invoices.some(i => i.case_id === args['id'])) {
            throw new Error('only a Vorgang without documents can be removed');
          }
          mockCases = mockCases.filter(c => c.id !== args['id']);
          return null;
        case 'case_graph':
          return caseGraph(args['id'] as number);
        case 'case_events': {
          const ids = invoices.filter(i => i.case_id === args['id']).map(i => i.id);
          return events
            .filter(e => ids.includes(e.invoice_id))
            .sort((a, b) => a.at.localeCompare(b.at));
        }
        case 'case_sequence':
        case 'case_sequence_set': {
          const counter = Math.max(mockCases.length + 1, Number(args['next'] ?? 0));
          return {
            next_number: caseNumber(counter),
            counter,
            highest: caseNumber(mockCases.length),
            floor: args['next'] ? counter : null,
          };
        }
        case 'offer_to_invoice': {
          // mirrors `Store::invoice_from_offer`
          const offer = find(args['offerId']);
          if (offer.kind !== 'offer' || offer.status !== 'exported') {
            throw 'Nur aus einem erstellten Angebot kann eine Rechnung entstehen.';
          }
          const standing = related(offer);
          if (standing.superseded || standing.lapsed) {
            throw 'Ein ersetztes Angebot oder eines, zu dem eine Alternative angenommen ist, wird nicht in Rechnung gestellt.';
          }
          if (offer.answer === 'declined') {
            throw 'Ein abgelehntes Angebot wird nicht in Rechnung gestellt.';
          }
          const own = (contexts.find(c => c.id === offer.context_id) as Context).settings;
          const { customer, buyer_reference, note } = structuredClone(offer.draft);
          // what is open of it, each position billing its own
          const billing = billingOf(offer.id, null);
          const positions = billing.positions
            .filter(b => Number(b.open) > 0)
            .map(b => billedPosition(billing, b.index, b.open));
          if (positions.length === 0) {
            throw 'Dieses Angebot ist vollständig in Rechnung gestellt.';
          }
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
            case_id: offer.case_id,
            context_id: offer.context_id,
            kind: 'invoice',
            number: null,
            status: 'draft',
            draft,
            total: check(draft).totals?.due ?? null,
            exported_at: null,
            paid_at: null,
            payments: [],
            answer: null,
            answered_on: null,
            updated_at: '',
            archived_at: null,
            files: null,
            ...UNRELATED,
          };
          invoices.push(invoice);
          event(invoice.id, 'created', null, { from: offer.id }, invoice.draft);
          return invoice;
        }
        case 'document_links':
          return linksOf(args['id'] as number);
        case 'offer_billing':
          return billingOf(args['id'] as number, (args['except'] as number | null) ?? null);
        case 'case_on_account': {
          const except = (args['except'] as number | null) ?? null;
          return invoices
            .filter(i => i.case_id === args['caseId'])
            .flatMap(i => {
              const found = onAccountOf(i);
              if (!found) {
                return [];
              }
              // a Storno takes them back, a cancelled one frees them (`on_account::DEDUCTING`)
              const by = invoices.find(
                other =>
                  other.id !== except &&
                  other.draft.corrects?.kind !== 'cancel' &&
                  !isCancelled(other.id) &&
                  (other.draft.deductions ?? []).some(d => d.invoice_id === i.id)
              );
              return [{ ...found, deducted_by: by ? (by.number ?? 'Entwurf') : null }];
            });
        }
        case 'case_billing': {
          const except = (args['except'] as number | null) ?? null;
          return invoices
            .filter(
              i =>
                (i.case_id === args['caseId'] ||
                  caseLinks.some(
                    l =>
                      l.follower === args['caseId'] &&
                      l.source === i.case_id &&
                      l.link === 'collect'
                  )) &&
                i.kind === 'offer' &&
                i.status === 'exported'
            )
            .map(related)
            .filter(o => o.answer !== 'declined' && !o.lapsed && !o.superseded)
            .sort((a, b) => a.id - b.id)
            .map(o => billingOf(o.id, except));
        }
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
          event(
            stored.id,
            'baked',
            stored.draft.issue_date,
            { number: stored.number },
            stored.draft
          );
          stored.exported_at = `${today()} 12:00:00`;
          stored.draft.frozen = settings;
          // what is due, a Stornorechnung's negative
          stored.total = check(stored.draft).totals?.due ?? stored.total;
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
        // backup and restore do nothing here, the answers are what the real ones would say
        case 'backup_status':
          return { last_backup_at: lastBackup, safety_dir: '/tmp/mock-data/backups', interrupted };
        case 'backup_discard_interrupted':
          interrupted = interrupted.filter(folder => folder !== args['folder']);
          return null;
        case 'backup_create':
          lastBackup = new Date().toISOString().replace(/\.\d+Z$/, 'Z');
          return {
            path: args['path'],
            files: 6,
            bytes: 1_300_000,
            created_at: lastBackup,
          };
        case 'backup_restore':
          return {
            counts: {
              contexts: contexts.length,
              invoices: invoices.filter(i => i.kind === 'invoice').length,
              offers: invoices.filter(i => i.kind === 'offer').length,
              layouts: customLayouts.length,
            },
            safety_copy:
              '/tmp/mock-data/backups/2026-01-01-120000-vor-wiederherstellung.erng-backup',
            problems: [],
            created_at: '2026-01-01T12:00:00Z',
            app_version: '0.0.0',
            schema_version: 11,
          };
        case 'data_remove':
          return {
            safety_copy: '/tmp/mock-data/backups/2026-01-01-120000-vor-entfernen.erng-backup',
          };
        case 'plugin:dialog|save':
          return '/tmp/mock-export';
        case 'plugin:dialog|open':
          return '/tmp/mock-backup.erng-backup';
        // `?update` in the address offers a newer version (`?update=0.8.0` that one), to try the updater
        case 'plugin:updater|check': {
          const offered = new URLSearchParams(location.search).get('update');
          return offered === null
            ? null
            : { rid: 1, currentVersion: '0.6.0', version: offered || '0.7.0', rawJson: {} };
        }
        case 'plugin:updater|download':
          // a moment of loading, so the button shows it
          return new Promise(resolve => setTimeout(() => resolve(2), 800));
        case 'plugin:updater|install':
          return null;
        case 'plugin:process|restart':
          location.reload();
          return null;
        default:
          return null;
      }
    },
    { shouldMockEvents: true }
  );
}
