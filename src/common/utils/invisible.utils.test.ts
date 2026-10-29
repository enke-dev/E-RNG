import { describe, expect, it } from 'vitest';

import { cleanInvisible, cleanInvisibleLine, invisibleIn } from './invisible.utils.js';

describe('invisibleIn', () => {
  it('says nothing about a plain text, tabs and line feeds included', () => {
    expect(invisibleIn('Telefon:\t{seller_phone}\nMobil: 0171 – Köln')).toEqual([]);
  });

  it('counts each kind by its code, in the order it first shows up', () => {
    const found = invisibleIn('a\u00A0b\u200Bc\u00A0d\u2009e\u00ADf');
    expect(found).toEqual([
      { label: 'NBSP', count: 2 },
      { label: 'ZWSP', count: 1 },
      { label: 'THSP', count: 1 },
      { label: 'SHY', count: 1 },
    ]);
  });

  it('knows the separators, the bidi controls and the control characters', () => {
    const found = invisibleIn('\u2028\u2029\u200E\u202E\u2066\uFEFF\u0000\u0007\u007F');
    expect(found.map(f => f.label)).toEqual([
      'LS',
      'PS',
      'LRM',
      'RLO',
      'LRI',
      'BOM',
      'Steuerzeichen',
    ]);
    expect(found.at(-1)?.count).toBe(3);
  });

  it('knows what behaves unlike it looks, but leaves visible dashes alone', () => {
    const found = invisibleIn(
      'E\u2011Mail\u034F\uFFFC\u180E 10,\u2013 \u2212 \u2014 \u2010 \u2012'
    );
    expect(found.map(f => f.label)).toEqual(['NBHY', 'CGJ', 'OBJ', 'MVS']);
  });
});

describe('cleanInvisible', () => {
  it('turns special spaces into normal ones', () => {
    expect(cleanInvisible('+49\u00A0221\u202F1\u20092\u30003')).toBe('+49 221 1 2 3');
  });

  it('drops what has no width, soft hyphens, bidi and control characters', () => {
    expect(
      cleanInvisible('a\u200Bb\u200Cc\u200Dd\u2060e\uFEFFf\u00ADg\u202Ah\u2069i\u0000j\u007Fk')
    ).toBe('abcdefghijk');
  });

  it('makes a non-breaking hyphen a normal one and drops pasted attachments', () => {
    expect(cleanInvisible('E\u2011Mail\uFFFC\u034F')).toBe('E-Mail');
  });

  it('makes line and paragraph separators line breaks', () => {
    expect(cleanInvisible('0171\u20285551234\u2029x')).toBe('0171\n5551234\nx');
  });

  it('keeps normal spaces, tabs, line feeds and everything visible', () => {
    const plain = 'Telefon:\t+49 221\n**fett** *kursiv* {iban} – ß';
    expect(cleanInvisible(plain)).toBe(plain);
    expect(invisibleIn(cleanInvisible('a\u00A0b\u200B\u2028c'))).toEqual([]);
  });
});

describe('cleanInvisibleLine', () => {
  it('keeps no line breaks or tabs, like a seller field after saving', () => {
    expect(cleanInvisibleLine('Musterstr.\u20281\n\t2\r')).toBe('Musterstr. 1  2');
  });
});
