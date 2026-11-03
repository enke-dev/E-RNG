import { invoke } from '@tauri-apps/api/core';

/** The shapes the Rust side stores and returns, see `src-tauri/src/model.rs` */

export interface Address {
  street: string;
  post_code: string;
  city: string;
  country: string;
}

export interface Seller {
  name: string;
  address: Address;
  vat_id: string | null;
  tax_number: string | null;
  contact_name: string;
  phone: string;
  email: string;
  /** Mobile, fax and website only appear as placeholders in layouts, not in the XML */
  mobile: string | null;
  fax: string | null;
  website: string | null;
  iban: string;
  bic: string | null;
  account_holder: string | null;
  /** The name of the bank, a placeholder of layouts only */
  bank_name: string | null;
  legal_info: string | null;
}

/** When the counter of the numbers starts again: never, or with every business year, quarter, month */
export type NumberReset = 'never' | 'fiscal_year' | 'quarter' | 'month';

/** The kinds of document: written alike, numbered apart; an offer is a PDF only */
export type Kind = 'invoice' | 'offer';

/** What a kind of document calls itself, a blank part is the default one */
export interface Wording {
  /** The headline: `Rechnung`, `Angebot` */
  title: string;
  /** The headline of a draft: `Rechnungsentwurf`, `Angebotsentwurf` */
  draft_title: string;
  /** The label of its number: `Rechnungsnummer`, `Angebotsnummer` */
  number_label: string;
}

/** What offers have of their own, their layout and texts otherwise follow the invoices' */
export interface OfferSettings {
  /** How offer numbers are made, like `Settings.number_pattern` */
  number_pattern: string;
  /** When the counter of the offer numbers starts again, like `Settings.number_reset` */
  number_reset: NumberReset;
  /** Days an offer is valid from its date; null: no validity */
  validity_days: number | null;
  /** Texts of the letter, `{valid_until}` stands for the day the offer is valid until */
  intro: string | null;
  closing: string | null;
  /** Null: the layout of the invoices */
  layout: string | null;
  wording: Wording;
}

export interface Settings {
  seller: Seller;
  /** Blank: no VAT is charged, the invoices show none and `vat_note` says why */
  tax_rate: string;
  vat_note: string;
  /** The label of the unit new positions start with, null (or one the list lacks): the first unit */
  unit: string | null;
  /** Blank: no due date and no payment terms */
  payment_days: number | null;
  /** How invoice numbers are made: date tokens like in layouts, N for the counter (NNN: three digits) */
  number_pattern: string;
  /** When the counter starts again with 1: the pattern only formats the number */
  number_reset: NumberReset;
  /**
   * ISO date the first business year starts on, for GGGG, GG, gggg, gg and Q in the pattern: day and
   * month repeat every year, the calendar year applies before it. Null: the business year is the
   * calendar year.
   */
  fiscal_year_start: string | null;
  /** The layout of the invoice document, invoices may choose another */
  layout: string;
  /** Texts of the letter, used unless an invoice has its own */
  intro: string | null;
  closing: string | null;
  invoice_wording: Wording;
  /** New invoices name the offers they are linked to, each invoice may choose otherwise */
  offer_reference: boolean;
  offer: OfferSettings;
  /** How the Vorgänge are numbered */
  cases: CaseSettings;
}

/** How the Vorgänge of a context are numbered */
export interface CaseSettings {
  number_pattern: string;
  number_reset: NumberReset;
}

/** Where a Vorgang stands, read from its documents */
/** Where a Vorgang stands, its furthest step (`cases::status`) */
export type CaseStage =
  | 'new'
  | 'drafting-offer'
  | 'drafting-invoice'
  | 'offered'
  | 'accepted'
  | 'declined'
  | 'invoiced'
  | 'partly-paid'
  | 'overdue'
  | 'partly-invoiced'
  | 'paid'
  | 'cancelled';

/** The documents of one business case for one customer */
export interface Case {
  id: number;
  context_id: number;
  number: string;
  customer_id: number | null;
  /** As its newest document has it, else as the customer is stored */
  customer_name: string;
  title: string;
  created_at: string;
  /** The customer cannot change any more: a second document or a created one */
  locked: boolean;
  documents: number;
  stage: CaseStage;
  /** Gross sums of its created documents */
  offered: string;
  accepted: string;
  invoiced: string;
  paid: string;
  open: string;
  /** Every document archived */
  archived: boolean;
  /** The Vorgänge it follows or collects, and those following or collecting it */
  sources: CaseRef[];
  followers: CaseRef[];
}

/** How a Vorgang refers to another: a Folgevorgang follows, a Sammelrechnung's Vorgang collects */
export type CaseLink = 'follow' | 'collect';

/** How one document relates to another, from the earlier to the later (`case_graph::EdgeKind`) */
export type EdgeKind =
  | 'bills'
  | 'share'
  | 'deducts'
  | 'supplement'
  | 'alternative'
  | 'replacement'
  | 'cancels'
  | 'replaces';

export interface GraphEdge {
  from: number;
  to: number;
  kind: EdgeKind;
}

/** The documents of a Vorgang and how they relate, documents of other Vorgänge on the other end too */
export interface CaseGraph {
  /** By issue date */
  documents: ListEntry[];
  edges: GraphEdge[];
  /** The other Vorgänge among the documents */
  others: { id: number; number: string }[];
}

/** Another Vorgang as a reference names it */
export interface CaseRef {
  id: number;
  number: string;
  link: CaseLink;
}

/**
 * One issuer (a company, later maybe a Verein or a private person) with its own seller, numbering
 * and defaults. The UI is tinted with its brand color.
 */
export interface Context {
  id: number | null;
  /** Palette name of the UI library (`wa-brand-<name>`), empty on a new context: the store picks one */
  brand: string;
  /** Set when archived: hidden from the switch, its invoices stay */
  archived_at: string | null;
  settings: Settings;
  /** What is missing before an invoice can be issued, as field paths of the form */
  missing: string[];
  /** What only XRechnung asks for on top */
  missing_for_xrechnung: string[];
  /** What is missing before an offer can be issued: the seller's name and address only */
  missing_for_offers: string[];
}

export interface Missing {
  missing: string[];
  missing_for_xrechnung: string[];
  missing_for_offers: string[];
}

/** Where the numbers of a kind of a context stand today */
export interface Sequence {
  next_number: string;
  /** The counter of the next number */
  counter: number;
  /** The highest number given in the period */
  highest: string | null;
  /** The counter the numbers continue from, while that is what decides the next number */
  floor: number | null;
}

/** What belongs to a context */
export interface ContextUsage {
  /** Drafts and created invoices, offers are counted apart */
  drafts: number;
  exported: number;
  offers: number;
  customers: number;
  templates: number;
  layouts: number;
}

/** What happens with what is scoped to a context that is removed */
export type Disposition = { kind: 'share' } | { kind: 'move'; to: number } | { kind: 'delete' };

export interface Customer {
  id: number | null;
  /** The context it belongs to, null: shared by all */
  context_id: number | null;
  name: string;
  address: Address;
  vat_id: string | null;
  email: string | null;
  buyer_reference: string | null;
}

export interface Position {
  description: string;
  quantity: string | null;
  /** UN/ECE rec. 20 code */
  unit: string | null;
  /** The label the unit was picked with: a created invoice keeps it whatever becomes of the unit */
  unit_label: string | null;
  /** VAT rate in percent as the position picked it, null: the default rate of the context */
  tax_rate: string | null;
  /** The label the rate was picked with, a created invoice keeps it whatever becomes of the rate */
  tax_label: string | null;
  price: string;
  /** Invoices: the position of a created offer of its Vorgang it bills */
  source?: PositionSource | null;
  /** Invoices on account: the share of an offer this lump sum is */
  share?: PositionShare | null;
}

/** A share of a created offer, the percent as typed */
export interface PositionShare {
  offer_id: number;
  percent: string;
}

/** A position of a created offer, by the offer and its place in it */
export interface PositionSource {
  offer_id: number;
  index: number;
}

/** What is billed of a position of an offer, decimals like the quantities */
export interface PositionBilling {
  index: number;
  position: Position;
  /** By created invoices */
  billed: string;
  /** By invoice drafts, which do not bill yet */
  drafted: string;
  /** What created invoices have not billed yet */
  open: string;
}

/** What is billed of a created offer, position by position, and by which invoices */
export interface OfferBilling {
  offer_id: number;
  number: string;
  positions: PositionBilling[];
  invoices: Linked[];
}

/** A unit of a context's list, which the positions of its documents pick from */
export interface Unit {
  id: number | null;
  context_id: number;
  code: string;
  label: string;
}

/** A unit without a place, what a new context starts with */
export type UnitDef = Pick<Unit, 'code' | 'label'>;

/** A VAT rate of a context's list, which the positions of its documents pick from */
export interface Rate {
  id: number | null;
  context_id: number;
  label: string;
  /** Percent, a dot as the separator and no trailing zeros: `19`, `5.5` */
  rate: string;
}

/** A rate without a place, what a new context starts with */
export type RateDef = Pick<Rate, 'label' | 'rate'>;

/** A rec. 20 / 21 code with its English name, and a German one if the code is common (listed first) */
export interface CatalogEntry {
  code: string;
  name: string;
  de?: string;
}

/** What changing or deleting a unit reaches: saved positions and drafts, never created documents */
export interface UnitUsage {
  templates: { id: number; description: string; context_id: number | null }[];
  drafts: { invoice_id: number; kind: Kind; customer: string }[];
}

/** What changing or deleting a rate reaches: saved positions, drafts and the default of the context */
export interface RateUsage extends UnitUsage {
  /** It is the rate that positions without a pick of their own take */
  default: boolean;
}

export interface PositionTemplate extends Position {
  id: number | null;
  /** The context it belongs to, null: shared by all */
  context_id: number | null;
}

export type Profile = 'en16931' | 'xrechnung';

export interface InvoiceDraft {
  customer: Customer;
  issue_date: string;
  /** The start, when there is an end */
  delivery_date: string;
  /** End of the service period, null: one day; blank while it is being typed */
  delivery_end: string | null;
  /** Offers: days from the issue date it is valid, null: no validity */
  validity_days: number | null;
  positions: Position[];
  profile: Profile;
  buyer_reference: string | null;
  note: string | null;
  /** Empty: the layout and texts from the settings */
  layout: string | null;
  intro: string | null;
  closing: string | null;
  /** Invoices: the document names the numbers of the offers it is linked to */
  offer_reference: boolean;
  frozen: Settings | null;
  /** Invoices: billed on account, provisional */
  on_account?: OnAccount | null;
  /** Invoices: a Schlussrechnung picked as such, even before it deducts one */
  settles?: boolean;
  /** Invoices: the invoices on account of its Vorgang it deducts, a Schlussrechnung */
  deductions?: Deduction[];
  /** Invoices: the created invoice it cancels (Stornorechnung) or replaces (Ersatzrechnung) */
  corrects?: Correction | null;
}

/** How an invoice corrects a created one: it cancels it, or replaces a cancelled one */
export type CorrectionKind = 'cancel' | 'replace';

/** The created invoice another corrects; the store fills in its number and date */
export interface Correction {
  invoice_id: number;
  kind: CorrectionKind;
  number: string;
  issue_date: string;
}

/** An invoice on account: money before any work (Anzahlung) or for the work so far (Abschlag) */
export type OnAccount = 'prepayment' | 'progress';

/** An invoice on account a Schlussrechnung deducts; the store fills in what it came to */
export interface Deduction {
  invoice_id: number;
  number: string;
  issue_date: string;
  net: string;
  vat: string;
  gross: string;
}

/** A created invoice on account of a Vorgang, with what deducts it */
export interface OnAccountInvoice {
  id: number;
  number: string;
  issue_date: string;
  on_account: OnAccount;
  net: string;
  vat: string;
  gross: string;
  /** The number of the created invoice deducting it, or `Entwurf` for a draft that does */
  deducted_by: string | null;
  /** What was received on it so far (none without payments), and the day it was paid completely */
  paid: string | null;
  paid_at: string | null;
}

export type Status = 'draft' | 'exported';

/** A payment received on an invoice */
export interface Payment {
  id: number;
  /** Gross, a decimal like the totals */
  amount: string;
  /** ISO date */
  paid_on: string;
}

/** What the customer said to a created offer */
export type Answer = 'accepted' | 'declined';

/**
 * How an offer refers to a created offer of its Vorgang: a Nachtrag (more work), an alternative
 * (a variant, one of the group is taken) or an Ersatzangebot (replaces it)
 */
export type Relation = 'supplement' | 'alternative' | 'replacement';

/** What an offer has from its relations, read from the data (`relations.rs`) */
/** Another document by its id and number */
export interface DocumentRef {
  id: number;
  number: string | null;
}

/** How a document stands to others: the offer it refers to, what supersedes or cancels it */
export interface Related {
  relation: Relation | null;
  /** The offer it refers to, see `relation` */
  related_id: number | null;
  related_number: string | null;
  /** Another offer of its alternative group is accepted, it is out of the running */
  lapsed: boolean;
  /** The other offer of its alternative group that is accepted */
  taken_by?: DocumentRef | null;
  /** An Ersatzangebot for it is created, and which */
  superseded: boolean;
  superseded_by?: DocumentRef | null;
  /** Invoices: a Stornorechnung for it is created, and which */
  cancelled: boolean;
  cancelled_by?: DocumentRef | null;
  /** Invoices: the draft of a Stornorechnung for it, not created yet */
  cancel_draft?: number | null;
  /** Invoices: bills only part of an offer, a Teilrechnung */
  partial: boolean;
}

export type EventKind =
  | 'created'
  | 'draft-exported'
  | 'baked'
  | 'payment'
  | 'payment-removed'
  | 'accepted'
  | 'declined'
  | 'answer-removed'
  | 'archived'
  | 'restored';

/** One entry of a document's history */
export interface DocumentEvent {
  id: number;
  invoice_id: number;
  kind: EventKind;
  /** When it was recorded, UTC */
  at: string;
  /** The day it is about, where it has one (paid on, accepted on, the date of a created document) */
  on: string | null;
  /** Draft exports: which one, from 1 */
  version?: number;
  /** Baked: the number it got */
  number?: string;
  /** Payments: the amount, gross */
  amount?: string;
  /** Created: the document it was made from */
  from?: number;
  /** The data of that version is kept, it can be compared */
  has_snapshot: boolean;
}

export interface StoredInvoice extends Related {
  id: number;
  /** The Vorgang it is in, for good */
  case_id: number;
  /** The context (issuer) it was made under, for good */
  context_id: number;
  /** What it is, for good */
  kind: Kind;
  number: string | null;
  status: Status;
  draft: InvoiceDraft;
  total: string | null;
  exported_at: string | null;
  /** ISO date it was paid completely on: the day of the payment that made it */
  paid_at: string | null;
  /** What was received on it, oldest first */
  payments: Payment[];
  /** Offers: what the customer said, and on which day */
  answer: Answer | null;
  answered_on: string | null;
  updated_at: string;
  /** Taken out of the list, it can be put back */
  archived_at: string | null;
  /** The originals made when it was created, none for a draft and for an invoice from before */
  files: InvoiceFiles | null;
}

export interface StoredFile {
  /** Relative to the app's data folder */
  path: string;
  sha256: string;
}

/** The PDF and XML that were validated on creation, handed out as they are */
export interface InvoiceFiles {
  pdf: StoredFile;
  /** Null for an offer, it has a PDF only */
  xml: StoredFile | null;
  /** Made again from the frozen data, for an invoice created before originals were kept */
  regenerated: boolean;
}

/** What creating an invoice came to */
export interface Created {
  invoice: StoredInvoice;
  /** What the validator said without refusing the invoice */
  warnings: Issue[];
}

export interface ListEntry extends Related {
  /** Invoices: on account, or a Schlussrechnung deducting such */
  on_account: OnAccount | null;
  closing: boolean;
  /** Invoices: the invoice it corrects */
  corrects: Correction | null;
  id: number;
  context_id: number;
  kind: Kind;
  /** The seller of its context, tells invoices apart in a mixed list */
  seller_name: string;
  number: string | null;
  status: Status;
  customer_name: string;
  issue_date: string;
  /** Offers: the day it is valid until, the issue date plus the days of the draft */
  valid_until: string | null;
  total: string | null;
  /** Paid completely on */
  paid_at: string | null;
  /** What was received so far, none without payments */
  paid: string | null;
  answer: Answer | null;
  answered_on: string | null;
  updated_at: string;
  archived_at: string | null;
  /** The Vorgang it is in */
  case_id: number;
}

/** The other end of a link between an offer and an invoice */
export interface Linked {
  id: number;
  kind: Kind;
  number: string | null;
  status: Status;
  customer_name: string;
  issue_date: string;
}

export interface Totals {
  net: string;
  vat: string;
  gross: string;
  /** What is to be paid: the gross less what a Schlussrechnung deducts */
  due: string;
}

export interface InputProblem {
  field: string;
  message: string;
}

export interface Finding {
  rule: string;
  severity: 'Fatal' | 'Warning' | 'Info';
  message: string;
}

export interface Check {
  problems: InputProblem[];
  findings: Finding[];
  totals: Totals | null;
}

export interface LayoutInfo {
  id: string;
  name: string;
  description: string;
  /** Made by the user, not built in */
  custom: boolean;
  /** Out of the selection, kept for the invoices that were made with it */
  archived: boolean;
}

/** Distances from the page edge to the content, in millimetres */
export interface Insets {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export interface FontFile {
  family: string;
  /** Weight and style as the font names it, empty when it does not say */
  style: string;
  path: string;
}

/** The answers the user gave while a layout file was being imported */
export interface ImportChoice {
  /** Replace the stored copy of the same layout */
  replace?: boolean;
  /** The name to store it under instead of the one in the file */
  name?: string;
}

/** What importing a layout file came to: done, or a question for the user */
export type ImportOutcome =
  | {
      kind: 'done';
      layout: CustomLayout;
      /** Fonts the file asked for but did not bring */
      missing_fonts: string[];
    }
  | {
      kind: 'replace';
      existing: { name: string; updated_at: string; uses: number; is_default: boolean };
      incoming_name: string;
      incoming_updated_at: string;
    }
  | { kind: 'name-taken'; name: string; suggested: string };

/** An invoice or offer made with an own layout, what keeps the layout from being deleted */
export interface LayoutUsage {
  invoice_id: number;
  kind: Kind;
  /** Null for a draft */
  number: string | null;
  customer: string;
}

/** One of the nine points of a page or of a part */
export type Anchor =
  | 'top-left'
  | 'top-center'
  | 'top-right'
  | 'middle-left'
  | 'center'
  | 'middle-right'
  | 'bottom-left'
  | 'bottom-center'
  | 'bottom-right';

export type Align = 'left' | 'center' | 'right';

export type InfoField =
  | 'number'
  | 'date'
  | 'valid-until'
  | 'service-date'
  | 'due-date'
  | 'buyer-reference'
  | 'preceding-invoice'
  | 'offer-reference'
  | 'tax-number'
  | 'vat-id';

/** A picture of a part, copied into the app's data when it was picked */
export interface ImageFile {
  name: string;
  path: string;
  /** Pixels of a raster image, 0 for a vector image */
  width_px: number;
  height_px: number;
  vector: boolean;
}

/** `#rrggbb`, none keeps the look of the kind of part */
type Color = string | null;

export type PartContent =
  | {
      kind: 'infos';
      fields: InfoField[];
      labels: boolean;
      align: Align;
      long_dates: boolean;
      size: number;
      color?: Color;
    }
  | { kind: 'address'; sender_line: boolean; size: number; color?: Color }
  | { kind: 'image'; image: ImageFile | null }
  | {
      kind: 'text';
      text: string;
      family: string | null;
      size: number;
      align: Align;
      color?: Color;
    }
  | {
      kind: 'page-number';
      text: string;
      size: number;
      align: Align;
      several_pages_only?: boolean;
      color?: Color;
    }
  | { kind: 'line'; thickness: number; color?: Color };

/** Something placed on the page, offsets in millimetres from the anchor towards the inside */
export type Part = {
  anchor: Anchor;
  x: number;
  y: number;
  /** Millimetres, null lets the content decide */
  width: number | null;
} & PartContent;

/** The frame of the flowing content on a page, and what is placed around it */
export interface PageSetup {
  margins: Insets;
  parts: Part[];
}

export type PhoneFormat = 'as-typed' | 'international' | 'national';

/** How the country of a foreign address is written: `AT`, `Österreich` or `ÖSTERREICH` */
export type CountryFormat = 'code' | 'name' | 'capitals';

/** How numbers, money and dates are written */
export interface Format {
  /** Between the whole and the fractional part */
  decimal: string;
  /** Between groups of three digits, empty for none */
  thousands: string;
  /** `€` instead of `EUR` */
  currency_symbol: boolean;
  /** `1,00 €` instead of `€ 1,00` */
  currency_after: boolean;
  /** A space between amount and currency */
  currency_space: boolean;
  /** `19 %` instead of `19%` */
  percent_space: boolean;
  /** `−` instead of `-` for deductions */
  true_minus: boolean;
  /** `DE02 1203 …` instead of `DE021203…` */
  iban_groups: boolean;
  /** Phone, mobile and fax of the seller: as typed, `+49 221 1234567` or `0221 1234567` */
  phone: PhoneFormat;
  /** The country of foreign addresses */
  country: CountryFormat;
  /** `muster.example` instead of `https://muster.example` */
  website_bare: boolean;
  /** A pattern like dayjs has them: `DD.MM.YYYY` */
  date: string;
  /** The same for the written out date: `D. MMMM YYYY` */
  date_long: string;
  /** The headline of a created document: `{title}`, `{draft_title}`, `{number}`, `{version}` */
  title: string;
  /** The same for a draft */
  draft_title: string;
}

/** A layout the user made, it starts as the built-in one */
export interface CustomLayout {
  id: number | null;
  /** The context it belongs to, null: every context can use it */
  context_id: number | null;
  /** Stays with the layout through renaming, export and import; empty until it is stored */
  uid: string;
  /** When it was last saved (UTC, ISO), empty until it is stored */
  updated_at: string;
  name: string;
  /** The pages after the first, and the first unless it has its own */
  page: PageSetup;
  /** The first page with a frame and parts of its own */
  first: PageSetup | null;
  fonts: FontFile[];
  /** The family the content is set in, null is the first font */
  font: string | null;
  format: Format;
  archived: boolean;
}

/** Where a part of a layout is on a page, millimetres from the top left of the page */
export interface PartBox {
  /** From 1 */
  page: number;
  /** On the setup of the first page, else on the one of the others */
  first: boolean;
  index: number;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Preview {
  /** One SVG document per page, rendered by the real template */
  pages: string[];
  problems: InputProblem[];
  /** Where the parts of a layout are, only for the editor of a layout */
  parts: PartBox[];
}

export interface Issue {
  section: 'pdf' | 'xml' | 'document';
  level: 'error' | 'warning' | 'notice';
  code: string;
  message: string;
}

export interface ValidationReport {
  valid: boolean;
  issues: Issue[];
}

export type ProduceError =
  | { kind: 'input'; problems: InputProblem[] }
  | { kind: 'invalid'; findings: Finding[] }
  | { kind: 'not-valid'; issues: Issue[] }
  | { kind: 'failed'; message: string };

/** Where the backups stand: when the last one was made (UTC, null: never) and where safety copies go */
export interface BackupStatus {
  last_backup_at: string | null;
  safety_dir: string;
  /** Folders a restore or removal that was cut off left the replaced data in */
  interrupted: string[];
}

export interface BackupCreated {
  path: string;
  files: number;
  bytes: number;
  created_at: string;
}

/** What a backup holds, as the user counts it */
export interface DataCounts {
  contexts: number;
  invoices: number;
  offers: number;
  layouts: number;
}

export interface RestoreOutcome {
  counts: DataCounts;
  /** The file the data in use was saved to before it was replaced */
  safety_copy: string;
  /** Kept originals that are missing or changed in the restored data */
  problems: string[];
  created_at: string;
  app_version: string;
  /** The schema the backup was written with: older ones were brought up to date */
  schema_version: number;
}

export interface RemoveOutcome {
  safety_copy: string;
}

/** Commands reject with a string, creating and handing out an invoice with a `ProduceError` */
export function errorText(error: unknown): string {
  if (typeof error === 'string') {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === 'object' && error !== null && 'kind' in error) {
    const failure = error as ProduceError;
    if (failure.kind === 'failed') {
      return failure.message;
    }
    if (failure.kind === 'input') {
      return failure.problems.map(p => p.message).join(', ');
    }
    if (failure.kind === 'not-valid') {
      return failure.issues.map(i => `${i.code}: ${i.message}`).join(', ');
    }
    return failure.findings.map(f => `${f.rule}: ${f.message}`).join(', ');
  }
  return String(error);
}

export const api = {
  backupStatus: () => invoke<BackupStatus>('backup_status'),
  backupDiscardInterrupted: (folder: string) =>
    invoke<null>('backup_discard_interrupted', { folder }),
  backupCreate: (path: string) => invoke<BackupCreated>('backup_create', { path }),
  /** Replaces all data by a backup, after a safety copy of the data in use; the app must reload */
  backupRestore: (path: string) => invoke<RestoreOutcome>('backup_restore', { path }),
  /** Back to a first start after a safety copy; `confirmation` is the word the user typed */
  dataRemove: (confirmation: string) => invoke<RemoveOutcome>('data_remove', { confirmation }),

  contextsList: () => invoke<Context[]>('contexts_list'),
  contextBlank: () => invoke<Context>('context_blank'),
  /** A new context (`id` null) starts with `units` and `rates` as its lists, none: the defaults */
  contextSave: (context: Context, units?: UnitDef[], rates?: RateDef[]) =>
    invoke<Context>('context_save', { context, units: units ?? null, rates: rates ?? null }),
  contextArchive: (id: number) => invoke<Context>('context_archive', { id }),
  contextUsage: (id: number) => invoke<ContextUsage>('context_usage', { id }),
  contextRemove: (id: number, what: Disposition) => invoke<null>('context_remove', { id, what }),
  settingsCheck: (settings: Settings) => invoke<Missing>('settings_check', { settings }),
  numberExample: (pattern: string, reset: NumberReset, fiscalYearStart: string | null) =>
    invoke<string>('number_example', { pattern, reset, fiscalYearStart }),
  numberSequence: (contextId: number, kind: Kind) =>
    invoke<Sequence>('number_sequence', { contextId, kind }),
  numberSequenceSet: (contextId: number, kind: Kind, next: number) =>
    invoke<Sequence>('number_sequence_set', { contextId, kind, next }),

  customersList: (contextId: number) => invoke<Customer[]>('customers_list', { contextId }),
  customerSave: (customer: Customer) => invoke<Customer>('customer_save', { customer }),
  customerDelete: (id: number) => invoke<null>('customer_delete', { id }),

  layoutsList: (contextId?: number) => invoke<LayoutInfo[]>('layouts_list', { contextId }),
  layoutsCustom: (contextId: number) => invoke<CustomLayout[]>('layouts_custom', { contextId }),
  layoutSave: (layout: CustomLayout) => invoke<CustomLayout>('layout_save', { layout }),
  layoutPreview: (layout: CustomLayout | null, guides: boolean, contextId: number | null) =>
    invoke<Preview>('layout_preview', { layout, guides, contextId }),
  layoutExport: (layout: CustomLayout, withFonts: boolean, path: string) =>
    invoke<null>('layout_export', { layout, withFonts, path }),
  layoutImport: (path: string, choice: ImportChoice) =>
    invoke<ImportOutcome>('layout_import', { path, choice }),
  layoutBuiltin: () => invoke<CustomLayout>('layout_builtin'),
  layoutImportImage: (path: string) => invoke<ImageFile>('layout_import_image', { path }),
  layoutUsage: (id: number) => invoke<LayoutUsage[]>('layout_usage', { id }),
  layoutDelete: (id: number) => invoke<null>('layout_delete', { id }),
  layoutImportFont: (path: string) => invoke<FontFile[]>('layout_import_font', { path }),

  templatesList: (contextId: number) => invoke<PositionTemplate[]>('templates_list', { contextId }),
  templateSave: (template: PositionTemplate) =>
    invoke<PositionTemplate>('template_save', { template }),
  templateDelete: (id: number) => invoke<null>('template_delete', { id }),

  unitsList: (contextId: number) => invoke<Unit[]>('units_list', { contextId }),
  unitsCatalog: () => invoke<CatalogEntry[]>('units_catalog'),
  /** How many drafts and saved positions use each unit, by unit id */
  unitsCounts: (contextId: number) => invoke<Record<string, number>>('units_counts', { contextId }),
  /** Adds a unit (`id` null); drafts and saved positions follow a changed label or code */
  unitSave: (unit: Unit) => invoke<Unit>('unit_save', { unit }),
  unitUsage: (id: number) => invoke<UnitUsage>('unit_usage', { id }),
  /** A unit in use needs the one that takes its place, in the same context */
  unitDelete: (id: number, replacement: number | null) =>
    invoke<null>('unit_delete', { id, replacement }),
  /** The unit new positions start with, by its label; saved at once, null: the first unit */
  unitDefault: (contextId: number, label: string | null) =>
    invoke<null>('unit_default', { contextId, label }),
  unitsReorder: (contextId: number, ids: number[]) =>
    invoke<null>('units_reorder', { contextId, ids }),
  /** The default rate of the context, a rate of its list; saved at once, blank: no VAT */
  rateDefault: (contextId: number, rate: string) =>
    invoke<null>('rate_default', { contextId, rate }),
  ratesList: (contextId: number) => invoke<Rate[]>('rates_list', { contextId }),
  /** How many drafts and saved positions use each rate, by rate id */
  ratesCounts: (contextId: number) => invoke<Record<string, number>>('rates_counts', { contextId }),
  /** Adds a rate (`id` null); drafts, saved positions and the default follow a changed one */
  rateSave: (rate: Rate) => invoke<Rate>('rate_save', { rate }),
  rateUsage: (id: number) => invoke<RateUsage>('rate_usage', { id }),
  /** A rate in use needs the one that takes its place, in the same context */
  rateDelete: (id: number, replacement: number | null) =>
    invoke<null>('rate_delete', { id, replacement }),
  ratesReorder: (contextId: number, ids: number[]) =>
    invoke<null>('rates_reorder', { contextId, ids }),

  /** The documents of one kind, `null` lists those of all contexts; archived ones only when asked */
  /** `kind` null lists both, as the Vorgänge do */
  invoicesList: (contextId: number | null, kind: Kind | null, withArchived = false) =>
    invoke<ListEntry[]>('invoices_list', { contextId, kind, withArchived }),
  invoiceGet: (id: number) => invoke<StoredInvoice>('invoice_get', { id }),
  /** A blank draft with the layout, texts and validity the settings of the context give its kind */
  invoiceBlank: (contextId: number, kind: Kind) =>
    invoke<InvoiceDraft>('invoice_blank', { contextId, kind }),
  /** A new document (`id` null) is made under `contextId` and of `kind`, a stored one stays as it is */
  /** A new document goes into `caseId` when given, else into a Vorgang of its own */
  invoiceSave: (
    id: number | null,
    contextId: number | null,
    kind: Kind,
    draft: InvoiceDraft,
    caseId: number | null = null
  ) => invoke<StoredInvoice>('invoice_save', { id, contextId, kind, draft, caseId }),
  invoiceArchive: (id: number) => invoke<null>('invoice_archive', { id }),
  invoiceUnarchive: (id: number) => invoke<StoredInvoice>('invoice_unarchive', { id }),
  /** Pays what is open on a day, or takes all payments back (`null`) */
  invoiceSetPaid: (id: number, paidAt: string | null) =>
    invoke<StoredInvoice>('invoice_set_paid', { id, paidAt }),
  invoiceAddPayment: (id: number, amount: string, paidOn: string) =>
    invoke<StoredInvoice>('invoice_add_payment', { id, amount, paidOn }),
  invoiceRemovePayment: (paymentId: number) =>
    invoke<StoredInvoice>('invoice_remove_payment', { paymentId }),
  /** Marks a created offer as accepted or declined on a day, or takes the answer back (`null`) */
  offerSetAnswer: (id: number, answer: Answer | null, on: string | null) =>
    invoke<StoredInvoice>('offer_set_answer', { id, answer, on }),
  invoiceEvents: (id: number) => invoke<DocumentEvent[]>('invoice_events', { id }),
  invoiceSnapshot: (eventId: number) => invoke<InvoiceDraft>('invoice_snapshot', { eventId }),
  /** A copy in a Vorgang of its own, or in the original's (`sameCase`) */
  invoiceDuplicate: (id: number, sameCase = false) =>
    invoke<StoredInvoice>('invoice_duplicate', { id, sameCase }),
  /** A Folgeangebot: the created offer copied into a new Vorgang that follows its own */
  offerFollow: (id: number) => invoke<StoredInvoice>('offer_follow', { id }),
  /** A Sammelrechnung: a new Vorgang collecting these of one customer, with an invoice of what is open */
  casesCollect: (ids: number[], title: string) =>
    invoke<StoredInvoice>('cases_collect', { ids, title }),
  /** A new offer in the Vorgang of the created offer `id` that refers to it */
  offerRelated: (id: number, relation: Relation) =>
    invoke<StoredInvoice>('offer_related', { id, relation }),
  casesList: (contextId: number | null, withArchived = false) =>
    invoke<Case[]>('cases_list', { contextId, withArchived }),
  caseGet: (id: number) => invoke<Case>('case_get', { id }),
  /** Starts a Vorgang for a customer, without documents yet */
  caseCreate: (contextId: number, customerId: number | null, title: string) =>
    invoke<Case>('case_create', { contextId, customerId, title }),
  caseSetTitle: (id: number, title: string) => invoke<Case>('case_set_title', { id, title }),
  /** Only a Vorgang without documents */
  caseDelete: (id: number) => invoke<null>('case_delete', { id }),
  caseEvents: (id: number) => invoke<DocumentEvent[]>('case_events', { id }),
  /** The documents of a Vorgang and how they relate */
  caseGraph: (id: number) => invoke<CaseGraph>('case_graph', { id }),
  /** Where the numbers of a context's Vorgänge stand today */
  caseSequence: (contextId: number) => invoke<Sequence>('case_sequence', { contextId }),
  /** Continues the numbers of the Vorgänge from `next` in today's period */
  caseSequenceSet: (contextId: number, next: number) =>
    invoke<Sequence>('case_sequence_set', { contextId, next }),
  invoiceDelete: (id: number) => invoke<null>('invoice_delete', { id }),
  /** A new invoice draft from a created offer, linked to it, with the layout and texts of invoices */
  offerToInvoice: (offerId: number) => invoke<StoredInvoice>('offer_to_invoice', { offerId }),
  /** The documents of the other kind a document is billed with: offers an invoice bills, invoices billing an offer */
  documentLinks: (id: number) => invoke<Linked[]>('document_links', { id }),
  /** What is billed of a created offer; `except` leaves an invoice draft out */
  offerBilling: (id: number, except: number | null = null) =>
    invoke<OfferBilling>('offer_billing', { id, except }),
  /** A Stornorechnung for a created invoice: a draft that mirrors it */
  invoiceCancel: (id: number) => invoke<StoredInvoice>('invoice_cancel', { id }),
  /** An Ersatzrechnung for a cancelled invoice: a draft of what it said */
  invoiceReplace: (id: number) => invoke<StoredInvoice>('invoice_replace', { id }),
  /** The created invoices on account of a Vorgang; `except` leaves the asking draft out */
  caseOnAccount: (caseId: number, except: number | null = null) =>
    invoke<OnAccountInvoice[]>('case_on_account', { caseId, except }),
  /** What is billed of the offers of a Vorgang that can still be billed */
  caseBilling: (caseId: number, except: number | null = null) =>
    invoke<OfferBilling[]>('case_billing', { caseId, except }),
  /** Totals and findings, an offer has no findings */
  invoiceCheck: (contextId: number, kind: Kind, draft: InvoiceDraft) =>
    invoke<Check>('check_invoice', { contextId, kind, draft }),

  invoicePreview: (draft: InvoiceDraft, id: number | null, contextId: number, kind: Kind) =>
    invoke<Preview>('preview_invoice', { draft, id, contextId, kind }),

  /** The draft as a plain PDF like the preview: marked as a draft, no number, no XML, not checked */
  invoiceDraftPdf: (
    draft: InvoiceDraft,
    id: number | null,
    contextId: number,
    kind: Kind,
    target: string
  ) => invoke<null>('invoice_draft_pdf', { draft, id, contextId, kind, target }),

  /** The number a draft would get if it were created now, by the pattern of its kind */
  invoiceNextNumber: (id: number) => invoke<string>('invoice_next_number', { id }),
  /** Checks, numbers, freezes and locks a draft and keeps its PDF and XML (an offer: its PDF only); rejects with a `ProduceError` */
  invoiceCreate: (id: number) => invoke<Created>('invoice_create', { id }),
  /** Copies the kept originals to the picked places, an offer has no XML to ask for */
  invoiceOutput: (id: number, pdfTarget: string | null, xmlTarget: string | null) =>
    invoke<StoredInvoice>('invoice_output', { id, pdfTarget, xmlTarget }),
  openedFiles: () => invoke<string[]>('opened_files'),
};
