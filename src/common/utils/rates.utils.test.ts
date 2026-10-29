import { describe, expect, it } from 'vitest';

import type { Rate } from './api.utils.js';
import { percent } from './format.utils.js';
import {
  chargesVat,
  defaultRateOf,
  pickedText,
  pickRate,
  rateChoice,
  rateOf,
  rateText,
  sameRate,
} from './rates.utils.js';

const rate = (id: number, label: string, value: string): Rate => ({
  id,
  context_id: 1,
  label,
  rate: value,
});
const list = [rate(1, 'Regelsatz', '19'), rate(2, 'Ermäßigt', '7'), rate(3, 'Halb', '5.5')];

describe('percent', () => {
  it('writes the German way, with a no-break space', () => {
    expect(percent('19')).toBe('19 %');
    expect(percent('5.5')).toBe('5,5 %');
  });
});

describe('sameRate', () => {
  it('compares numbers, comma or dot', () => {
    expect(sameRate('19', '19,0')).toBe(true);
    expect(sameRate('5.5', '5,50')).toBe(true);
    expect(sameRate('19', '7')).toBe(false);
  });

  it('knows no blank as a rate', () => {
    expect(sameRate('', '')).toBe(false);
    expect(sameRate(' ', '0')).toBe(false);
  });
});

describe('rateText', () => {
  it('names the label and the percent', () => {
    expect(rateText(rate(1, 'Regelsatz', '19'))).toBe('Regelsatz (19 %)');
  });
});

describe('rateChoice', () => {
  it('selects the default entry for a position that picked none', () => {
    expect(rateChoice(list, { tax_rate: null, tax_label: null }, '19')).toEqual({
      selected: list[0],
    });
  });

  it('selects the entry a position picked', () => {
    expect(rateChoice(list, { tax_rate: '7', tax_label: 'Ermäßigt' }, '19')).toEqual({
      selected: list[1],
    });
  });

  it('gives a rate the list lacks an entry of its own', () => {
    expect(rateChoice(list, { tax_rate: '16', tax_label: 'Corona' }, '19')).toEqual({
      missing: { percent: percent('16'), text: `Corona (${percent('16')})` },
    });
    expect(rateChoice(list, { tax_rate: null, tax_label: null }, '16')).toEqual({
      missing: { percent: percent('16'), text: percent('16') },
    });
  });
});

describe('defaultRateOf', () => {
  it('finds the entry by its rate', () => {
    expect(defaultRateOf(list, '5,5')?.id).toBe(3);
    expect(defaultRateOf(list, '')).toBeUndefined();
  });
});

describe('rateOf', () => {
  it('finds the entry by rate and label', () => {
    expect(rateOf(list, { tax_rate: '7', tax_label: 'Ermäßigt' })?.id).toBe(2);
    expect(rateOf(list, { tax_rate: '7,0', tax_label: 'Ermäßigt' })?.id).toBe(2);
  });

  it('finds none for a position that takes the default, or one that picked what is gone', () => {
    expect(rateOf(list, { tax_rate: null, tax_label: null })).toBeUndefined();
    expect(rateOf(list, { tax_rate: ' ', tax_label: null })).toBeUndefined();
    expect(rateOf(list, { tax_rate: '7', tax_label: 'Essen' })).toBeUndefined();
    expect(rateOf(list, { tax_rate: '16', tax_label: 'Regelsatz' })).toBeUndefined();
  });
});

describe('pickRate', () => {
  it('writes value and label, and clears both for the default', () => {
    const position = { tax_rate: null, tax_label: null };

    pickRate(position, list[1]);
    expect(position).toEqual({ tax_rate: '7', tax_label: 'Ermäßigt' });

    pickRate(position, undefined);
    expect(position).toEqual({ tax_rate: null, tax_label: null });
  });
});

describe('pickedText', () => {
  it('shows what the position was picked with', () => {
    expect(pickedText({ tax_rate: '16', tax_label: 'Essen' })).toBe('Essen (16 %)');
    expect(pickedText({ tax_rate: '16', tax_label: null })).toBe('16 %');
  });
});

describe('chargesVat', () => {
  it('is no VAT for a blank rate', () => {
    expect(chargesVat('19')).toBe(true);
    expect(chargesVat('  ')).toBe(false);
  });
});
