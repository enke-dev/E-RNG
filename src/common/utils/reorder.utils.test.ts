import { describe, expect, it } from 'vitest';

import { indexAfterMove } from './reorder.utils.js';

describe('indexAfterMove', () => {
  const after = (length: number, from: number, to: number) => {
    const rows = Array.from({ length }, (_, i) => i);
    const [moved] = rows.splice(from, 1);
    rows.splice(to, 0, moved as number);
    // where each row of before is in the list after
    return Array.from({ length }, (_, i) => rows.indexOf(i));
  };

  it('follows the moved row and the rows it passed', () => {
    expect(indexAfterMove(1, 1, 3)).toBe(3);
    expect(indexAfterMove(2, 1, 3)).toBe(1);
    expect(indexAfterMove(3, 1, 3)).toBe(2);
    expect(indexAfterMove(3, 3, 1)).toBe(1);
    expect(indexAfterMove(1, 3, 1)).toBe(2);
    expect(indexAfterMove(2, 3, 1)).toBe(3);
  });

  it('leaves the rows it did not pass', () => {
    expect(indexAfterMove(0, 1, 3)).toBe(0);
    expect(indexAfterMove(4, 1, 3)).toBe(4);
    expect(indexAfterMove(0, 3, 1)).toBe(0);
    expect(indexAfterMove(4, 3, 1)).toBe(4);
  });

  it('agrees with moving the rows of a list', () => {
    [
      [5, 0, 4],
      [5, 4, 0],
      [5, 2, 3],
      [5, 3, 2],
    ].forEach(([length, from, to]) =>
      expect(
        Array.from({ length: length as number }, (_, i) =>
          indexAfterMove(i, from as number, to as number)
        )
      ).toEqual(after(length as number, from as number, to as number))
    );
  });
});
