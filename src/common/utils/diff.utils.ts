import type { Address, InvoiceDraft, Position } from './api.utils.js';
import { formatDate, money, percent } from './format.utils.js';

/** A piece of a compared text: in both, or only in the newer (`added`) or older one (`removed`) */
export interface Segment {
  text: string;
  change?: 'added' | 'removed';
}

/** A field that differs between two versions, as it is shown: before and after, empty for none */
export interface FieldChange {
  label: string;
  before: string;
  after: string;
  /** A text to compare word by word; a short value is shown whole, old and new */
  words?: true;
}

/** A position of the newer version against the older one; unchanged ones are only counted */
export type PositionChange =
  | { change: 'added'; after: Position }
  | { change: 'removed'; before: Position }
  | { change: 'changed'; before: Position; after: Position; fields: FieldChange[] };

/** What differs between two versions of a document */
export interface DraftDiff {
  fields: FieldChange[];
  positions: PositionChange[];
  /** Positions the same in both */
  unchanged: number;
}

/** The longest common subsequence of two lists as pairs of indices, in order */
function common<T>(a: T[], b: T[], same: (x: T, y: T) => boolean): [number, number][] {
  // lengths[i][j]: the longest common run of a[i..] and b[j..], filled from the ends
  const lengths = a
    .reduceRight<number[][]>(
      (rows, x) => {
        const below = rows.at(-1) ?? [];
        const row = b
          .reduceRight<number[]>(
            (cells, y, j) => {
              const longest = same(x, y)
                ? (below[j + 1] ?? 0) + 1
                : Math.max(below[j] ?? 0, cells.at(-1) ?? 0);
              cells.push(longest);
              return cells;
            },
            [0]
          )
          .reverse();
        rows.push(row);
        return rows;
      },
      [new Array<number>(b.length + 1).fill(0)]
    )
    .reverse();
  const length = (i: number, j: number) => lengths[i]?.[j] ?? 0;
  const walk = (i: number, j: number): [number, number][] => {
    if (i >= a.length || j >= b.length) {
      return [];
    }
    if (same(a[i] as T, b[j] as T)) {
      return [[i, j], ...walk(i + 1, j + 1)];
    }
    return length(i + 1, j) >= length(i, j + 1) ? walk(i + 1, j) : walk(i, j + 1);
  };
  return walk(0, 0);
}

/** A run of words both texts have, or what one has in their place */
type Hunk = { same: string } | { removed: string; added: string };

/** Two texts word by word: the words both keep, the ones dropped and the ones new */
export function diffWords(before: string, after: string): Segment[] {
  // whitespace is a token of its own, so a changed line break shows as well
  const tokens = (text: string) => text.match(/\s+|[^\s]+/g) ?? [];
  const [a, b] = [tokens(before), tokens(after)];
  const pairs = [...common(a, b, (x, y) => x === y), [a.length, b.length] as [number, number]];
  const hunks = pairs.reduce<{ from: [number, number]; hunks: Hunk[] }>(
    ({ from: [i, j], hunks }, [k, l]) => {
      const [removed, added] = [a.slice(i, k).join(''), b.slice(j, l).join('')];
      const kept = a[k];
      return {
        from: [k + 1, l + 1],
        hunks: [
          ...hunks,
          ...(removed || added ? [{ removed, added }] : []),
          ...(kept === undefined ? [] : [{ same: kept }]),
        ],
      };
    },
    { from: [0, 0], hunks: [] }
  ).hunks;
  const merged = hunks.reduce<Hunk[]>((out, hunk, n) => {
    const [last, next] = [out.at(-1), hunks[n + 1]];
    // a lone space between two changes goes with them, so the words changed read as one
    const between = 'same' in hunk && /^\s+$/.test(hunk.same) && next && !('same' in next);
    if (last && !('same' in last) && (between || !('same' in hunk))) {
      last.removed += 'same' in hunk ? hunk.same : hunk.removed;
      last.added += 'same' in hunk ? hunk.same : hunk.added;
    } else if (last && 'same' in last && 'same' in hunk) {
      last.same += hunk.same;
    } else {
      out.push({ ...hunk });
    }
    return out;
  }, []);
  return merged.flatMap((hunk): Segment[] =>
    'same' in hunk
      ? [{ text: hunk.same }]
      : [
          ...(hunk.removed ? [{ text: hunk.removed, change: 'removed' as const }] : []),
          ...(hunk.added ? [{ text: hunk.added, change: 'added' as const }] : []),
        ]
  );
}

const address = ({ street, post_code, city, country }: Address) =>
  [street, [post_code, city].filter(Boolean).join(' '), country].filter(Boolean).join(', ');

const service = (d: InvoiceDraft) =>
  d.delivery_end === null
    ? formatDate(d.delivery_date)
    : `${formatDate(d.delivery_date)} bis ${d.delivery_end ? formatDate(d.delivery_end) : '…'}`;

/** A field by its label and how it is shown, a text compared word by word */
type Field<T> = [string, (x: T) => string, 'words'?];

/** The fields of a document compared, in the order of the editor, each as it is shown */
const FIELDS: Field<InvoiceDraft>[] = [
  ['Kunde', d => d.customer.name],
  ['Anschrift', d => address(d.customer.address)],
  ['USt-IdNr.', d => d.customer.vat_id ?? ''],
  ['E-Mail für Rechnungen', d => d.customer.email ?? ''],
  ['Datum', d => formatDate(d.issue_date)],
  ['Leistung', service],
  ['Gültigkeit (Tage)', d => (d.validity_days === null ? '' : String(d.validity_days))],
  ['XRechnung', d => (d.profile === 'xrechnung' ? 'ja' : 'nein')],
  ['Leitweg-ID / Käuferreferenz', d => d.buyer_reference ?? ''],
  ['Angebotsnummer angeben', d => (d.offer_reference ? 'ja' : 'nein')],
  ['Layout', d => d.layout ?? 'aus den Einstellungen'],
  ['Einleitung', d => d.intro ?? '', 'words'],
  ['Schlusstext', d => d.closing ?? '', 'words'],
  ['Anmerkung', d => d.note ?? '', 'words'],
];

/** The fields of a position compared, each as it is shown */
const POSITION_FIELDS: Field<Position>[] = [
  ['Beschreibung', p => p.description, 'words'],
  ['Menge', p => [p.quantity ?? '', p.unit_label ?? p.unit ?? ''].filter(Boolean).join(' ')],
  ['Preis (netto)', p => money(p.price)],
  ['USt.', p => p.tax_label ?? (p.tax_rate === null ? 'Standard' : percent(p.tax_rate))],
];

const changes = <T>(fields: Field<T>[], before: T, after: T): FieldChange[] =>
  fields
    .map(([label, show, words]): FieldChange => {
      const change = { label, before: show(before), after: show(after) };
      return words ? { ...change, words: true } : change;
    })
    .filter(f => f.before !== f.after);

const samePosition = (a: Position, b: Position) => changes(POSITION_FIELDS, a, b).length === 0;

/**
 * What changed from one version of a document to another. Positions have no identity of their
 * own: the ones both keep are matched in order, of the rest between two kept ones the first
 * removed and the first added are one changed position, and so on, the rest is removed or added.
 */
export function diffDrafts(before: InvoiceDraft, after: InvoiceDraft): DraftDiff {
  const pairs = common(before.positions, after.positions, samePosition);
  const positions = [...pairs, [before.positions.length, after.positions.length] as const].reduce<{
    from: [number, number];
    found: PositionChange[];
  }>(
    ({ from: [i, j], found }, [k, l]) => {
      const removed = before.positions.slice(i, k);
      const added = after.positions.slice(j, l);
      const gap = Array.from(
        { length: Math.max(removed.length, added.length) },
        (_, n): PositionChange => {
          const [was, is] = [removed[n], added[n]];
          if (was && is) {
            return {
              change: 'changed',
              before: was,
              after: is,
              fields: changes(POSITION_FIELDS, was, is),
            };
          }
          // past the end of one side, the other has the position
          return was
            ? { change: 'removed', before: was }
            : { change: 'added', after: is as Position };
        }
      );
      return { from: [k + 1, l + 1], found: [...found, ...gap] };
    },
    { from: [0, 0], found: [] }
  ).found;
  return { fields: changes(FIELDS, before, after), positions, unchanged: pairs.length };
}
