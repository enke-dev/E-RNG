import { describe, expect, it } from 'vitest';

import names from '../../../src-tauri/data/unit-names.json' with { type: 'json' };
import type { CatalogEntry, Unit } from './api.utils.js';
import {
  defaultUnitOf,
  describe as describeCode,
  pick,
  searchCatalog,
  startUnit,
  unitLabel,
  unitOf,
} from './units.utils.js';

const unit = (id: number, code: string, label: string): Unit => ({
  id,
  context_id: 1,
  code,
  label,
});
const list = [unit(1, 'C62', 'Stk.'), unit(2, 'HUR', 'Std.'), unit(3, 'HUR', 'Stunde')];

describe('unitOf', () => {
  it('finds the unit by code and label, so two labels on one code stay apart', () => {
    expect(unitOf(list, { unit: 'HUR', unit_label: 'Stunde' })?.id).toBe(3);
    expect(unitOf(list, { unit: 'HUR', unit_label: 'Std.' })?.id).toBe(2);
  });

  it('reads a blank code as C62', () => {
    expect(unitOf(list, { unit: null, unit_label: 'Stk.' })?.id).toBe(1);
  });

  it('finds nothing for a unit that is not in the list', () => {
    expect(unitOf(list, { unit: 'DAY', unit_label: 'Tag(e)' })).toBeUndefined();
    expect(unitOf(list, { unit: 'HUR', unit_label: 'Hour' })).toBeUndefined();
  });
});

describe('pick', () => {
  it('writes code and label into the position', () => {
    const position = { unit: null, unit_label: null };

    pick(position, list[2]);

    expect(position).toEqual({ unit: 'HUR', unit_label: 'Stunde' });
    expect(unitLabel(position)).toBe('Stunde');
  });
});

describe('the default unit', () => {
  it('is the unit the label names', () => {
    expect(defaultUnitOf(list, 'Stunde')?.id).toBe(3);
    expect(defaultUnitOf(list, 'Std.')?.id).toBe(2);
  });

  it('is the first unit without a label, or with one the list no longer has', () => {
    expect(defaultUnitOf(list, null)?.id).toBe(1);
    expect(defaultUnitOf(list, undefined)?.id).toBe(1);
    expect(defaultUnitOf(list, 'Meter')?.id).toBe(1);
    expect(defaultUnitOf([], 'Std.')).toBeUndefined();
  });

  it('is what a new position starts with, none for an empty list', () => {
    expect(startUnit(list, 'Stunde')).toEqual({ unit: 'HUR', unit_label: 'Stunde' });
    expect(startUnit(list, null)).toEqual({ unit: 'C62', unit_label: 'Stk.' });
    expect(startUnit([], null)).toEqual({ unit: null, unit_label: null });
  });
});

describe('searchCatalog', () => {
  const catalog: CatalogEntry[] = names.units;
  const codes = (search: string) => searchCatalog(catalog, search).map(e => e.code);

  it('offers the common codes while nothing is asked for', () => {
    const common = searchCatalog(catalog, '  ');

    expect(common.length).toBeGreaterThan(20);
    expect(common.length).toBeLessThan(60);
    expect(common.every(e => e.de !== undefined)).toBe(true);
    expect(common[0]?.code).toBe('C62');
  });

  it('finds a code, a German and an English name, ignoring case and umlauts', () => {
    expect(codes('kwh')[0]).toBe('KWH');
    expect(codes('STUCK')[0]).toBe('C62');
    expect(codes('hour')[0]).toBe('HUR');
    expect(codes('stunde')[0]).toBe('HUR');
  });

  it('puts an exact code before names that merely contain it, and common before other names', () => {
    expect(codes('day')[0]).toBe('DAY');
    expect(codes('mm')[0]).toBe('MMT');
    expect(codes('acre')).toContain('ACR');
  });

  it('needs every word, in any order', () => {
    expect(codes('metre cubic')).toContain('MTQ');
    expect(codes('metre cubic zzz')).toEqual([]);
  });

  it('finds every code by its code', () => {
    const missed = catalog.filter(e => searchCatalog(catalog, e.code)[0]?.code !== e.code);

    expect(missed.map(e => e.code)).toEqual([]);
  });
});

describe('describe', () => {
  it('writes the code with the German name, else the English one', () => {
    expect(describeCode({ code: 'HUR', name: 'hour', de: 'Stunde' })).toBe('HUR – Stunde');
    expect(describeCode({ code: 'ACR', name: 'acre' })).toBe('ACR – acre');
  });
});
