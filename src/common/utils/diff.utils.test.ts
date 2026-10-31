import { describe, expect, it } from 'vitest';

import type { InvoiceDraft, Position } from './api.utils.js';
import { diffDrafts, diffWords } from './diff.utils.js';

const position = (description: string, price = '10.00'): Position => ({
  description,
  quantity: '1',
  unit: 'C62',
  unit_label: 'Stk.',
  tax_rate: null,
  tax_label: null,
  price,
});

const draft = (change: Partial<InvoiceDraft> = {}): InvoiceDraft => ({
  customer: {
    id: 1,
    context_id: null,
    name: 'Beispiel AG',
    address: { street: 'Hauptstraße 5', post_code: '50667', city: 'Köln', country: 'DE' },
    vat_id: null,
    email: null,
    buyer_reference: null,
  },
  issue_date: '2026-10-31',
  delivery_date: '2026-10-31',
  delivery_end: null,
  validity_days: 30,
  positions: [position('Planung'), position('Montage'), position('Abnahme')],
  profile: 'en16931',
  buyer_reference: null,
  note: null,
  layout: null,
  intro: 'Sehr geehrte Damen und Herren,',
  closing: null,
  offer_reference: false,
  frozen: null,
  ...change,
});

describe('diffWords', () => {
  it('keeps what both have and marks what was dropped and what is new', () => {
    expect(diffWords('Hallo liebe Frau Muster', 'Hallo Frau Muster, schön')).toEqual([
      { text: 'Hallo ' },
      { text: 'liebe ', change: 'removed' },
      { text: 'Frau ' },
      { text: 'Muster', change: 'removed' },
      { text: 'Muster, schön', change: 'added' },
    ]);
  });

  it('keeps words changed together in one piece, the spaces between them included', () => {
    expect(diffWords('Sehr geehrte Damen und Herren,', 'Sehr geehrte Frau Muster,')).toEqual([
      { text: 'Sehr geehrte ' },
      { text: 'Damen und Herren,', change: 'removed' },
      { text: 'Frau Muster,', change: 'added' },
    ]);
  });

  it('shows a text that is new or gone as a whole', () => {
    expect(diffWords('', 'Neu hier')).toEqual([{ text: 'Neu hier', change: 'added' }]);
    expect(diffWords('Weg', '')).toEqual([{ text: 'Weg', change: 'removed' }]);
    expect(diffWords('Gleich', 'Gleich')).toEqual([{ text: 'Gleich' }]);
  });
});

describe('diffDrafts', () => {
  it('finds nothing in the same version', () => {
    expect(diffDrafts(draft(), draft())).toEqual({ fields: [], positions: [], unchanged: 3 });
  });

  it('lists the fields that differ, as they are shown', () => {
    const { fields } = diffDrafts(
      draft(),
      draft({ validity_days: null, issue_date: '2026-11-02', intro: null, layout: 'klar' })
    );
    expect(fields).toEqual([
      { label: 'Datum', before: '31.10.2026', after: '02.11.2026' },
      { label: 'Gültigkeit (Tage)', before: '30', after: '' },
      { label: 'Layout', before: 'aus den Einstellungen', after: 'klar' },
      { label: 'Einleitung', before: 'Sehr geehrte Damen und Herren,', after: '', words: true },
    ]);
  });

  it('matches the positions both keep, a replaced one is changed, the rest added or removed', () => {
    const { positions, unchanged } = diffDrafts(
      draft(),
      draft({
        positions: [position('Planung'), position('Montage', '12.50'), position('Wartung')],
      })
    );
    expect(unchanged).toBe(1);
    expect(positions).toEqual([
      {
        change: 'changed',
        before: position('Montage'),
        after: position('Montage', '12.50'),
        fields: [{ label: 'Preis (netto)', before: '10,00 €', after: '12,50 €' }],
      },
      {
        change: 'changed',
        before: position('Abnahme'),
        after: position('Wartung'),
        fields: [{ label: 'Beschreibung', before: 'Abnahme', after: 'Wartung', words: true }],
      },
    ]);
  });

  it('tells added and removed positions where there is nothing to pair them with', () => {
    const { positions } = diffDrafts(
      draft(),
      draft({ positions: [position('Vorab'), position('Planung'), position('Abnahme')] })
    );
    expect(positions).toEqual([
      { change: 'added', after: position('Vorab') },
      { change: 'removed', before: position('Montage') },
    ]);
  });
});
