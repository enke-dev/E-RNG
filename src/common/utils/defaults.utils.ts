import type { Settings } from './api.utils.js';

/** The settings that are defaults for documents, not facts about the issuer */
export type Defaults = Pick<
  Settings,
  | 'tax_rate'
  | 'vat_note'
  | 'payment_days'
  | 'number_pattern'
  | 'number_reset'
  | 'fiscal_year_start'
  | 'intro'
  | 'closing'
  | 'invoice_wording'
  | 'offer'
>;

/** What a new context takes over from the one in use: a better start than the app's defaults */
export function inheritedDefaults(from: Defaults): Defaults {
  // a copy, the nested settings must not be shared between contexts
  return structuredClone({
    tax_rate: from.tax_rate,
    vat_note: from.vat_note,
    payment_days: from.payment_days,
    number_pattern: from.number_pattern,
    number_reset: from.number_reset,
    fiscal_year_start: from.fiscal_year_start,
    intro: from.intro,
    closing: from.closing,
    invoice_wording: from.invoice_wording,
    offer: from.offer,
  });
}
