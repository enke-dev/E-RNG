import { describe, expect, it } from 'vitest';

import { casesRoute, parseSelection } from './cases.utils.js';

describe('the address of the Vorgänge', () => {
  it('names documents, new ones in or out of a Vorgang, and Vorgänge', () => {
    expect(casesRoute('offer')).toBe('/cases');
    expect(casesRoute('offer', 7)).toBe('/cases/7');
    expect(casesRoute('offer', 7, 12)).toBe('/cases/case-12/7');
    expect(casesRoute('invoice', 'new')).toBe('/cases/new-invoice');
    expect(casesRoute('offer', 'new', 12)).toBe('/cases/new-offer@12');
  });

  it('is read back as what it selects', () => {
    expect(parseSelection('')).toEqual({ type: 'newest' });
    expect(parseSelection('none')).toEqual({ type: 'none' });
    expect(parseSelection('7')).toEqual({ type: 'document', id: 7, caseId: null });
    expect(parseSelection('case-12/7')).toEqual({ type: 'document', id: 7, caseId: 12 });
    expect(parseSelection('new-invoice')).toEqual({ type: 'new', kind: 'invoice', caseId: null });
    expect(parseSelection('new-offer@12')).toEqual({ type: 'new', kind: 'offer', caseId: 12 });
    expect(parseSelection('case-3')).toEqual({ type: 'case', id: 3 });
    expect(parseSelection('nonsense')).toEqual({ type: 'none' });
  });
});
