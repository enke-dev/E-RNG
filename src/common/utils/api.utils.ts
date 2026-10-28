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
  iban: string;
  bic: string | null;
  account_holder: string | null;
  legal_info: string | null;
}

export interface Settings {
  seller: Seller;
  /** Blank: no VAT is charged, the invoices show none and `vat_note` says why */
  tax_rate: string;
  vat_note: string;
  /** Blank: no due date and no payment terms */
  payment_days: number | null;
  /** How invoice numbers are made: date tokens like in layouts, N for the counter (NNN: three digits) */
  number_pattern: string;
  /** The layout of the invoice document, invoices may choose another */
  layout: string;
  /** Texts of the letter, used unless an invoice has its own */
  intro: string | null;
  closing: string | null;
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
}

export interface Missing {
  missing: string[];
  missing_for_xrechnung: string[];
}

/** What belongs to a context */
export interface ContextUsage {
  drafts: number;
  exported: number;
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
  unit: string | null;
  price: string;
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
  positions: Position[];
  profile: Profile;
  buyer_reference: string | null;
  note: string | null;
  /** Empty: the layout and texts from the settings */
  layout: string | null;
  intro: string | null;
  closing: string | null;
  frozen: Settings | null;
}

export type Status = 'draft' | 'exported';

export interface StoredInvoice {
  id: number;
  /** The context (issuer) it was made under, for good */
  context_id: number;
  number: string | null;
  status: Status;
  draft: InvoiceDraft;
  total: string | null;
  exported_at: string | null;
  /** ISO date the exported invoice was paid on */
  paid_at: string | null;
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
  xml: StoredFile;
  /** Made again from the frozen data, for an invoice created before originals were kept */
  regenerated: boolean;
}

/** What creating an invoice came to */
export interface Created {
  invoice: StoredInvoice;
  /** What the validator said without refusing the invoice */
  warnings: Issue[];
}

export interface ListEntry {
  id: number;
  context_id: number;
  /** The seller of its context, tells invoices apart in a mixed list */
  seller_name: string;
  number: string | null;
  status: Status;
  customer_name: string;
  issue_date: string;
  total: string | null;
  paid_at: string | null;
  updated_at: string;
  archived_at: string | null;
}

export interface Totals {
  net: string;
  vat: string;
  gross: string;
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

/** An invoice made with an own layout, what keeps the layout from being deleted */
export interface LayoutUsage {
  invoice_id: number;
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
  | 'service-date'
  | 'due-date'
  | 'buyer-reference'
  | 'preceding-invoice'
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

export type PartContent =
  | {
      kind: 'infos';
      fields: InfoField[];
      labels: boolean;
      align: Align;
      long_dates: boolean;
      size: number;
    }
  | { kind: 'address'; sender_line: boolean; size: number }
  | { kind: 'image'; image: ImageFile | null }
  | { kind: 'text'; text: string; family: string | null; size: number; align: Align }
  | { kind: 'page-number'; text: string; size: number; align: Align }
  | { kind: 'line'; thickness: number };

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
  /** A pattern like dayjs has them: `DD.MM.YYYY` */
  date: string;
  /** The same for the written out date: `D. MMMM YYYY` */
  date_long: string;
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
  contextsList: () => invoke<Context[]>('contexts_list'),
  contextBlank: () => invoke<Context>('context_blank'),
  contextSave: (context: Context) => invoke<Context>('context_save', { context }),
  contextArchive: (id: number) => invoke<Context>('context_archive', { id }),
  contextUsage: (id: number) => invoke<ContextUsage>('context_usage', { id }),
  contextRemove: (id: number, what: Disposition) => invoke<null>('context_remove', { id, what }),
  settingsCheck: (settings: Settings) => invoke<Missing>('settings_check', { settings }),
  numberExample: (pattern: string) => invoke<string>('number_example', { pattern }),

  customersList: (contextId: number) => invoke<Customer[]>('customers_list', { contextId }),
  customerSave: (customer: Customer) => invoke<Customer>('customer_save', { customer }),
  customerDelete: (id: number) => invoke<null>('customer_delete', { id }),

  layoutsList: (contextId: number) => invoke<LayoutInfo[]>('layouts_list', { contextId }),
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
  layoutImportFont: (path: string) => invoke<FontFile>('layout_import_font', { path }),

  templatesList: (contextId: number) => invoke<PositionTemplate[]>('templates_list', { contextId }),
  templateSave: (template: PositionTemplate) =>
    invoke<PositionTemplate>('template_save', { template }),
  templateDelete: (id: number) => invoke<null>('template_delete', { id }),

  /** `null` lists the invoices of all contexts */
  /** Archived invoices only when asked */
  invoicesList: (contextId: number | null, withArchived = false) =>
    invoke<ListEntry[]>('invoices_list', { contextId, withArchived }),
  invoiceGet: (id: number) => invoke<StoredInvoice>('invoice_get', { id }),
  invoiceBlank: () => invoke<InvoiceDraft>('invoice_blank'),
  /** A new invoice (`id` null) is made under `contextId`, a stored one stays where it is */
  invoiceSave: (id: number | null, contextId: number | null, draft: InvoiceDraft) =>
    invoke<StoredInvoice>('invoice_save', { id, contextId, draft }),
  invoiceArchive: (id: number) => invoke<null>('invoice_archive', { id }),
  invoiceUnarchive: (id: number) => invoke<StoredInvoice>('invoice_unarchive', { id }),
  invoiceSetPaid: (id: number, paidAt: string | null) =>
    invoke<StoredInvoice>('invoice_set_paid', { id, paidAt }),
  invoiceDuplicate: (id: number) => invoke<StoredInvoice>('invoice_duplicate', { id }),
  invoiceDelete: (id: number) => invoke<null>('invoice_delete', { id }),
  invoiceCheck: (contextId: number, draft: InvoiceDraft) =>
    invoke<Check>('check_invoice', { contextId, draft }),

  invoicePreview: (draft: InvoiceDraft, id: number | null, contextId: number) =>
    invoke<Preview>('preview_invoice', { draft, id, contextId }),

  /** The draft as a plain PDF like the preview: marked as a draft, no number, no XML, not checked */
  invoiceDraftPdf: (draft: InvoiceDraft, id: number | null, contextId: number, target: string) =>
    invoke<null>('invoice_draft_pdf', { draft, id, contextId, target }),

  /** The number a draft would get if it were created now */
  invoiceNextNumber: (id: number) => invoke<string>('invoice_next_number', { id }),
  /** Checks, numbers, freezes and locks a draft and keeps its PDF and XML; rejects with a `ProduceError` */
  invoiceCreate: (id: number) => invoke<Created>('invoice_create', { id }),
  /** Copies the kept originals to the picked places */
  invoiceOutput: (id: number, pdfTarget: string | null, xmlTarget: string | null) =>
    invoke<StoredInvoice>('invoice_output', { id, pdfTarget, xmlTarget }),
  openedFiles: () => invoke<string[]>('opened_files'),
};
