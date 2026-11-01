import { describe, expect, it } from 'vitest';

import type { OfferBilling, Position } from './api.utils.js';
import { amount, billedPosition, quantityOf, sourceName } from './billing.utils.js';

const position = (quantity: string | null): Position => ({
  description: 'Montage',
  quantity,
  unit: 'HUR',
  unit_label: 'Std.',
  tax_rate: null,
  tax_label: null,
  price: '50',
});

const billing: OfferBilling = {
  offer_id: 7,
  number: 'AN-2026-001',
  positions: [
    { index: 0, position: position('10'), billed: '4', drafted: '0', open: '6' },
    { index: 1, position: position(null), billed: '0', drafted: '0', open: '1' },
  ],
  invoices: [],
};

describe('billing', () => {
  it('writes quantities and names offer positions as the documents do', () => {
    expect(amount('2.5')).toBe('2,5');
    expect(quantityOf(position(null))).toBe(1);
    expect(quantityOf(position(' 3 '))).toBe(3);
    expect(quantityOf(position('2,5'))).toBe(2.5);
    expect(sourceName('AN-2026-001', 1)).toBe('AN-2026-001 · Pos. 2');
  });

  it('makes a picked offer position an invoice position referring to it', () => {
    expect(billedPosition(billing, 0, '6')).toEqual({
      ...position('6'),
      source: { offer_id: 7, index: 0 },
    });
    // a lump sum stays one
    expect(billedPosition(billing, 1, '1').quantity).toBeNull();
    expect(() => billedPosition(billing, 2, '1')).toThrow();
  });
});
