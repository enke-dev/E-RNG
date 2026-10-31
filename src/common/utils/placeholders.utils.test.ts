import { describe, expect, it } from 'vitest';

import { PART_PLACEHOLDERS, searchPlaceholders } from './placeholders.utils.js';

describe('searchPlaceholders', () => {
  it('keeps every group without a search', () => {
    expect(searchPlaceholders(PART_PLACEHOLDERS, '  ')).toBe(PART_PLACEHOLDERS);
  });

  it('finds by name, meaning and example, and leaves empty groups out', () => {
    const found = searchPlaceholders(PART_PLACEHOLDERS, 'iban');
    expect(found.map(g => g.heading)).toEqual(['Bankverbindung', 'Mehrere Zeilen']);
    expect(found[0]?.placeholders.map(p => p.name)).toEqual(['iban']);
    expect(searchPlaceholders(PART_PLACEHOLDERS, 'Vorgang')[0]?.placeholders[0]?.name).toBe(
      'vorgang_number'
    );
    expect(searchPlaceholders(PART_PLACEHOLDERS, 'xyz')).toEqual([]);
  });
});
