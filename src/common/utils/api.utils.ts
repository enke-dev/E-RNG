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
  logo: string | null;
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

export type Profile = 'en16931' | 'xrechnung';

export interface InvoiceDraft {
  customer: Customer;
  issue_date: string;
  delivery_date: string;
  positions: Position[];
  profile: Profile;
  buyer_reference: string | null;
  note: string | null;
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
  updated_at: string;
}

export interface ListEntry {
  id: number;
  number: string | null;
  status: Status;
  customer_name: string;
  issue_date: string;
  total: string | null;
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

  invoicesList: () => invoke<ListEntry[]>('invoices_list'),
  invoiceGet: (id: number) => invoke<StoredInvoice>('invoice_get', { id }),
  invoiceBlank: () => invoke<InvoiceDraft>('invoice_blank'),
  invoiceSave: (id: number | null, draft: InvoiceDraft) =>
    invoke<StoredInvoice>('invoice_save', { id, draft }),
  invoiceDuplicate: (id: number) => invoke<StoredInvoice>('invoice_duplicate', { id }),
  invoiceDelete: (id: number) => invoke<null>('invoice_delete', { id }),
  invoiceCheck: (draft: InvoiceDraft) => invoke<Check>('check_invoice', { draft }),

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
