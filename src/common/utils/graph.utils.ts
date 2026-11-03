import type { EdgeKind, GraphEdge, Kind } from './api.utils.js';

/** What the layout needs of a document */
export interface GraphDocument {
  id: number;
  kind: Kind;
  issue_date: string;
  case_id: number;
}

/** Where a document sits: its column (offers, invoices), its row in it and how deep in its tree */
export interface Placed {
  id: number;
  column: 0 | 1;
  row: number;
  depth: number;
}

export interface Layout {
  placed: Placed[];
  /** Drawn as the branches of a tree: a Nachtrag, Alternative or Ersatzangebot under its offer, a Storno or Ersatzrechnung under its invoice */
  branches: GraphEdge[];
  /** Drawn as lines: what bills, shares or deducts what, without what a correction repeats */
  lines: GraphEdge[];
}

/** The relations a tree is made of, the child under its parent */
const BRANCHES = new Set<EdgeKind>([
  'supplement',
  'alternative',
  'replacement',
  'cancels',
  'replaces',
]);

const byDate = (a: GraphDocument, b: GraphDocument) =>
  a.issue_date.localeCompare(b.issue_date) || a.id - b.id;

/**
 * Two columns, offers and invoices, each a list of trees by date: an offer with its Nachträge,
 * Alternativen and Ersatzangebote under it, an invoice with its Storno and Ersatzrechnung under it;
 * the documents of other Vorgänge after the own ones. A correction repeats what it corrects (an
 * Ersatzrechnung bills the same offers, a Storno takes back the same deductions): such a line is
 * left out, the branch to the corrected invoice says it.
 */
export function layoutGraph(
  documents: GraphDocument[],
  edges: GraphEdge[],
  caseId: number
): Layout {
  const known = new Map(documents.map(d => [d.id, d]));
  const valid = edges.filter(e => known.has(e.from) && known.has(e.to) && e.from !== e.to);

  // one parent per document, of the same column and Vorgang, the first one named
  const parent = new Map<number, number>();
  valid
    .filter(e => BRANCHES.has(e.kind))
    .filter(e => {
      const [from, to] = [known.get(e.from), known.get(e.to)];
      return from?.kind === to?.kind && from?.case_id === to?.case_id;
    })
    .forEach(e => !parent.has(e.to) && parent.set(e.to, e.from));
  // a loop of parents is cut where it closes
  const rooted = (id: number, seen = new Set<number>()): boolean => {
    const up = parent.get(id);
    if (up === undefined) {
      return true;
    }
    if (seen.has(id)) {
      return false;
    }
    return rooted(up, new Set([...seen, id]));
  };
  [...parent.keys()].filter(id => !rooted(id)).forEach(id => parent.delete(id));
  const branches = valid.filter(e => BRANCHES.has(e.kind) && parent.get(e.to) === e.from);

  const children = (id: number) => documents.filter(d => parent.get(d.id) === id).sort(byDate);
  const tree = (doc: GraphDocument, depth: number): { id: number; depth: number }[] => [
    { id: doc.id, depth },
    ...children(doc.id).flatMap(child => tree(child, depth + 1)),
  ];
  const column = (offers: boolean) => {
    const roots = documents
      .filter(d => (d.kind === 'offer') === offers && !parent.has(d.id))
      .sort((a, b) => Number(a.case_id !== caseId) - Number(b.case_id !== caseId) || byDate(a, b));
    return roots.flatMap(root => tree(root, 0));
  };
  const placed: Placed[] = [
    ...column(true).map(({ id, depth }, row) => ({ id, column: 0 as const, row, depth })),
    ...column(false).map(({ id, depth }, row) => ({ id, column: 1 as const, row, depth })),
  ];

  // the invoice a correction corrects, and so on up: what it says, they said already
  const corrected = (id: number): number[] => {
    const up = valid.find(e => e.to === id && (e.kind === 'cancels' || e.kind === 'replaces'));
    return up && parent.get(id) === up.from ? [up.from, ...corrected(up.from)] : [];
  };
  const lines = valid
    .filter(e => !BRANCHES.has(e.kind) || parent.get(e.to) !== e.from)
    .filter(
      e =>
        !corrected(e.to).some(up =>
          valid.some(o => o.from === e.from && o.to === up && o.kind === e.kind)
        )
    );
  return { placed, branches, lines };
}
