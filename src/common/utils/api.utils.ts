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
  tax_rate: string;
  payment_days: number;
  number_prefix: string;
  footer: string | null;
  /** The layout of the invoice document, invoices may choose another */
  layout: string;
  /** Texts of the letter, used unless an invoice has its own */
  intro: string | null;
  closing: string | null;
}

export interface Customer {
  id: number | null;
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
}

export type Profile = 'en16931' | 'xrechnung';

export interface InvoiceDraft {
  customer: Customer;
  issue_date: string;
  delivery_date: string;
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
  number: string | null;
  status: Status;
  draft: InvoiceDraft;
  total: string | null;
  exported_at: string | null;
  /** ISO date the exported invoice was paid on */
  paid_at: string | null;
  updated_at: string;
}

export interface ListEntry {
  id: number;
  number: string | null;
  status: Status;
  customer_name: string;
  issue_date: string;
  total: string | null;
  paid_at: string | null;
  updated_at: string;
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

/** A layout the user made, it starts as the built-in one */
export interface CustomLayout {
  id: number | null;
  name: string;
  /** The pages after the first, and the first unless it has its own */
  page: PageSetup;
  /** The first page with a frame and parts of its own */
  first: PageSetup | null;
  fonts: FontFile[];
  /** The family the content is set in, null is the first font */
  font: string | null;
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

export interface ProduceResult {
  work_dir: string;
  number: string;
  report: ValidationReport;
}

export type ProduceError =
  | { kind: 'input'; problems: InputProblem[] }
  | { kind: 'invalid'; findings: Finding[] }
  | { kind: 'failed'; message: string };

/** Commands reject with a string, `produce_invoice` with a `ProduceError` */
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
    return failure.findings.map(f => `${f.rule}: ${f.message}`).join(', ');
  }
  return String(error);
}

export const api = {
  settingsGet: () => invoke<Settings>('settings_get'),
  settingsSave: (settings: Settings) => invoke<null>('settings_save', { settings }),

  customersList: () => invoke<Customer[]>('customers_list'),
  customerSave: (customer: Customer) => invoke<Customer>('customer_save', { customer }),
  customerDelete: (id: number) => invoke<null>('customer_delete', { id }),

  layoutsList: () => invoke<LayoutInfo[]>('layouts_list'),
  layoutsCustom: () => invoke<CustomLayout[]>('layouts_custom'),
  layoutSave: (layout: CustomLayout) => invoke<CustomLayout>('layout_save', { layout }),
  layoutPreview: (layout: CustomLayout | null, guides: boolean) =>
    invoke<Preview>('layout_preview', { layout, guides }),
  layoutBuiltin: () => invoke<CustomLayout>('layout_builtin'),
  layoutImportImage: (path: string) => invoke<ImageFile>('layout_import_image', { path }),
  layoutUsage: (id: number) => invoke<LayoutUsage[]>('layout_usage', { id }),
  layoutDelete: (id: number) => invoke<null>('layout_delete', { id }),
  layoutImportFont: (path: string) => invoke<FontFile>('layout_import_font', { path }),

  templatesList: () => invoke<PositionTemplate[]>('templates_list'),
  templateSave: (template: PositionTemplate) =>
    invoke<PositionTemplate>('template_save', { template }),
  templateDelete: (id: number) => invoke<null>('template_delete', { id }),

  invoicesList: () => invoke<ListEntry[]>('invoices_list'),
  invoiceGet: (id: number) => invoke<StoredInvoice>('invoice_get', { id }),
  invoiceBlank: () => invoke<InvoiceDraft>('invoice_blank'),
  invoiceSave: (id: number | null, draft: InvoiceDraft) =>
    invoke<StoredInvoice>('invoice_save', { id, draft }),
  invoiceSetPaid: (id: number, paidAt: string | null) =>
    invoke<StoredInvoice>('invoice_set_paid', { id, paidAt }),
  invoiceDuplicate: (id: number) => invoke<StoredInvoice>('invoice_duplicate', { id }),
  invoiceDelete: (id: number) => invoke<null>('invoice_delete', { id }),
  invoiceCheck: (draft: InvoiceDraft) => invoke<Check>('check_invoice', { draft }),

  invoicePreview: (draft: InvoiceDraft, id: number | null) =>
    invoke<Preview>('preview_invoice', { draft, id }),

  produce: (id: number) => invoke<ProduceResult>('produce_invoice', { id }),
  exportInvoice: (
    id: number,
    workDir: string,
    pdfTarget: string | null,
    xmlTarget: string | null
  ) =>
    invoke<StoredInvoice>('export_invoice', {
      id,
      workDir,
      pdfTarget,
      xmlTarget,
    }),
  discardWorkDir: (workDir: string) => invoke<null>('discard_work_dir', { workDir }),
};
