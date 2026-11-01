import { describe, expect, it } from 'vitest';

import type { OfferBilling, Position } from './api.utils.js';
import { amount, billedPosition, quantityOf, sharePositions, sourceName } from './billing.utils.js';

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

  it('makes a share of an offer one lump sum per VAT rate', () => {
    // 10 hours at 50 and a lump sum of 50: 550 net
    const [share] = sharePositions(billing, 30, 'progress');
    expect(share).toMatchObject({
      description: 'Abschlag 30 % auf Angebot AN-2026-001',
      quantity: null,
      unit_label: 'pauschal',
      price: '165,00',
      source: null,
    });
    const mixed: OfferBilling = {
      ...billing,
      positions: [
        ...billing.positions,
        {
          index: 2,
          position: { ...position('2'), price: '10,50', tax_rate: '7', tax_label: '7%' },
          billed: '0',
          drafted: '0',
          open: '2',
        },
      ],
    };
    const shares = sharePositions(mixed, 12.5, 'prepayment');
    expect(shares.map(s => [s.description, s.price, s.tax_rate])).toEqual([
      ['Anzahlung 12,5 % auf Angebot AN-2026-001 (USt. Standard)', '68,75', null],
      ['Anzahlung 12,5 % auf Angebot AN-2026-001 (USt. 7%)', '2,63', '7'],
    ]);
  });
});
