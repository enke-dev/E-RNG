import type {
  Deduction,
  InvoiceDraft,
  OfferBilling,
  OnAccount,
  OnAccountInvoice,
  Position,
} from './api.utils.js';
import { money } from './format.utils.js';

/** A typed price or quantity as a number, either way it is written (`1.130,50`, `1130.50`) */
export const parsed = (typed: string) => {
  const text = typed.trim();
  return Number(text.includes(',') ? text.replace(/\./g, '').replace(',', '.') : text) || 0;
};

/** A price as typed, as money: `95,00` is `95,00 €`, not `95,00,00 €` */
export const typedMoney = (typed: string) => money(String(parsed(typed)));

/** How many of a position: its quantity as typed, a blank one is one (`billing::quantity`) */
export const quantityOf = (position: Position) =>
  position.quantity?.trim() ? parsed(position.quantity) : 1;

/** A decimal quantity as the documents write it: `2.5` → `2,5` */
export const amount = (quantity: string) => quantity.replace('.', ',');

/** How an offer position is named where an invoice position refers to it */
export const sourceName = (number: string, index: number) => `${number}, Pos. ${index + 1}`;

/**
 * What a picked offer position becomes on an invoice: its data and `quantity` of it, referring to
 * it; a blank quantity stays blank when one is billed (`billing::billed`)
 */
export function billedPosition(billing: OfferBilling, index: number, quantity: string): Position {
  const { position } = billing.positions[index] ?? { position: undefined };
  if (!position) {
    throw new Error(`no position ${index} on offer ${billing.number}`);
  }
  const keepBlank = position.quantity === null && Number(quantity) === 1;
  return {
    ...position,
    quantity: keepBlank ? null : quantity,
    source: { offer_id: billing.offer_id, index },
  };
}

/**
 * A share of an offer on account: `percent` of its net, one position per VAT rate of its positions,
 * as a lump sum (`Abschlag 30 % auf Angebot AN-2026-001`); a position on account bills nothing of
 * the offer for good, so it refers to no position
 */
export function sharePositions(
  billing: OfferBilling,
  percent: number,
  onAccount: OnAccount
): Position[] {
  const groups = billing.positions.reduce((byRate, { position }) => {
    const key = position.tax_rate ?? '';
    const group = byRate.get(key) ?? { net: 0, position };
    byRate.set(key, { ...group, net: group.net + quantityOf(position) * parsed(position.price) });
    return byRate;
  }, new Map<string, { net: number; position: Position }>());
  const what = onAccount === 'prepayment' ? 'Anzahlung' : 'Abschlag';
  const several = groups.size > 1;
  const typed = String(percent).replace('.', ',');
  return [...groups.values()].map(({ net, position }) => ({
    description:
      `${what} ${typed} % auf Angebot ${billing.number}` +
      (several ? ` (USt. ${position.tax_label ?? 'Standard'})` : ''),
    // one lump sum: quantity and unit are fixed (`share`)
    share: { offer_id: billing.offer_id, percent: typed },
    quantity: null,
    unit: 'LS',
    unit_label: 'pauschal',
    tax_rate: position.tax_rate,
    tax_label: position.tax_label,
    // written as prices are typed
    price: (Math.round(net * percent) / 100).toFixed(2).replace('.', ','),
    source: null,
  }));
}

/** Whether a draft is a Schlussrechnung: picked or deducting, no Storno (`InvoiceDraft::is_closing`) */
export const isClosing = (draft: InvoiceDraft): boolean =>
  !draft.on_account &&
  draft.corrects?.kind !== 'cancel' &&
  (Boolean(draft.settles) || (draft.deductions ?? []).length > 0);

/** A created invoice on account as a Schlussrechnung deducts it */
export const deductionOf = (invoice: OnAccountInvoice): Deduction => ({
  invoice_id: invoice.id,
  number: invoice.number,
  issue_date: invoice.issue_date,
  net: invoice.net,
  vat: invoice.vat,
  gross: invoice.gross,
});

/**
 * The offers an invoice bills only part of, less of a position or not all of them: a Teilrechnung
 * of them, whatever was billed before (as `billing::partial`)
 */
export function partlyBilled(positions: Position[], billings: OfferBilling[]): string[] {
  return billings
    .filter(billing => positions.some(p => p.source?.offer_id === billing.offer_id))
    .filter(billing =>
      billing.positions.some(({ index, position }) => {
        const here = positions
          .filter(p => p.source?.offer_id === billing.offer_id && p.source.index === index)
          .reduce((sum, p) => sum + quantityOf(p), 0);
        return here < quantityOf(position);
      })
    )
    .map(billing => billing.number);
}
