import { describe, expect, it } from 'vitest';

import type { EdgeKind, Kind } from './api.utils.js';
import { layoutGraph } from './graph.utils.js';

const edge = (from: number, to: number, kind: EdgeKind = 'bills') => ({ from, to, kind });
const doc = (id: number, kind: Kind, issue_date: string, case_id = 1) => ({
  id,
  kind,
  issue_date,
  case_id,
});

describe('the graph of a Vorgang', () => {
  // an offer, its alternative and Nachtrag; an Abschlag on both; a Schlussrechnung deducting it,
  // cancelled and replaced
  const documents = [
    doc(1, 'offer', '2026-10-17'),
    doc(2, 'offer', '2026-10-18'),
    doc(3, 'offer', '2026-10-25'),
    doc(4, 'invoice', '2026-10-28'),
    doc(5, 'invoice', '2026-11-01'),
    doc(6, 'invoice', '2026-11-02'),
    doc(7, 'invoice', '2026-11-03'),
  ];
  const edges = [
    edge(1, 2, 'alternative'),
    edge(1, 3, 'supplement'),
    edge(1, 4, 'share'),
    edge(3, 4, 'share'),
    edge(1, 5),
    edge(3, 5),
    edge(4, 5, 'deducts'),
    edge(5, 6, 'cancels'),
    edge(4, 6, 'deducts'),
    edge(5, 7, 'replaces'),
    edge(1, 7),
    edge(3, 7),
    edge(4, 7, 'deducts'),
  ];

  it('puts offers and invoices in columns, each a tree by date', () => {
    expect(layoutGraph(documents, edges, 1).placed).toEqual([
      { id: 1, column: 0, row: 0, depth: 0 },
      { id: 2, column: 0, row: 1, depth: 1 },
      { id: 3, column: 0, row: 2, depth: 1 },
      { id: 4, column: 1, row: 0, depth: 0 },
      { id: 5, column: 1, row: 1, depth: 0 },
      { id: 6, column: 1, row: 2, depth: 1 },
      { id: 7, column: 1, row: 3, depth: 1 },
    ]);
  });

  it('draws relations and corrections as branches, leaves out what a correction repeats', () => {
    const { branches, lines } = layoutGraph(documents, edges, 1);
    expect(branches.map(e => e.kind)).toEqual(['alternative', 'supplement', 'cancels', 'replaces']);
    expect(lines.map(e => [e.from, e.to, e.kind])).toEqual([
      [1, 4, 'share'],
      [3, 4, 'share'],
      [1, 5, 'bills'],
      [3, 5, 'bills'],
      [4, 5, 'deducts'],
    ]);
  });

  it('puts documents of other Vorgänge last and survives a loop', () => {
    const { placed } = layoutGraph(
      [
        doc(1, 'invoice', '2026-01-01', 2),
        doc(2, 'invoice', '2026-02-01'),
        doc(3, 'offer', '2026-03-01'),
        doc(4, 'invoice', '2026-04-01'),
        doc(5, 'invoice', '2026-05-01'),
      ],
      [edge(3, 1), edge(3, 2), edge(1, 2, 'cancels'), edge(4, 5, 'cancels'), edge(5, 4, 'cancels')],
      1
    );
    expect(placed.map(p => [p.id, p.column, p.row])).toEqual([
      [3, 0, 0],
      [2, 1, 0],
      [4, 1, 1],
      [5, 1, 2],
      [1, 1, 3],
    ]);
  });
});
