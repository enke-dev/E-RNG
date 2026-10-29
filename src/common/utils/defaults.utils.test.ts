import { describe, expect, it } from 'vitest';

import type { Unit } from './api.utils.js';
import type { Defaults } from './defaults.utils.js';
import { inheritedDefaults, inheritedUnits } from './defaults.utils.js';

const defaults = (): Defaults => ({
  tax_rate: '7',
  vat_note: 'Kleinunternehmer',
  payment_days: 30,
  number_pattern: '[R-]GGGG-NNN',
  number_reset: 'quarter',
  fiscal_year_start: '2026-04-01',
  intro: 'Hallo',
  closing: 'Tschüss',
  invoice_wording: { title: 'Faktura', draft_title: '', number_label: '' },
  offer_reference: true,
  offer: {
    number_pattern: '[O-]YYYY-NNN',
    number_reset: 'never',
    validity_days: 14,
    intro: null,
    closing: 'Gilt bis {valid_until}.',
    layout: 'custom-3',
    wording: { title: 'Offerte', draft_title: 'Offertenentwurf', number_label: 'Offertennummer' },
  },
});

describe('inheritedDefaults', () => {
  it('takes the invoice defaults, the counter resets, the fiscal year, the offer reference and the offer settings along', () => {
    expect(inheritedDefaults(defaults())).toEqual(defaults());
  });

  it('leaves out what is the issuer’s own', () => {
    const from = { ...defaults(), seller: { name: 'Muster' }, layout: 'standard' };

    expect(Object.keys(inheritedDefaults(from)).sort()).toEqual(Object.keys(defaults()).sort());
  });

  it('does not share the nested settings with the context it came from', () => {
    const from = defaults();
    const copy = inheritedDefaults(from);

    copy.offer.wording.title = 'Kostenvoranschlag';
    copy.invoice_wording.title = 'Honorarnote';

    expect(from.offer.wording.title).toBe('Offerte');
    expect(from.invoice_wording.title).toBe('Faktura');
  });
});

describe('inheritedUnits', () => {
  const units: Unit[] = [
    { id: 4, context_id: 1, code: 'HUR', label: 'Stunde' },
    { id: 2, context_id: 1, code: 'C62', label: 'Lizenz' },
  ];

  it('takes the units along in their order, without the ids of the context they came from', () => {
    expect(inheritedUnits(units)).toEqual([
      { code: 'HUR', label: 'Stunde' },
      { code: 'C62', label: 'Lizenz' },
    ]);
  });

  it('does not share anything with the units it came from', () => {
    const copy = inheritedUnits(units);

    const [first] = copy;
    if (first) {
      first.label = 'Std.';
    }

    expect(units[0]?.label).toBe('Stunde');
  });
});
