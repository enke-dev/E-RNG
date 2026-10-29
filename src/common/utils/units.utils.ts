import type { CatalogEntry, Position, Unit } from './api.utils.js';

type Named = Pick<Position, 'unit' | 'unit_label'>;

/** What a position calls its unit: the label it was picked with */
export const unitLabel = (position: Named): string =>
  position.unit_label?.trim() || position.unit || '';

/** The unit of a list that a position names: code and label, a blank code is C62 */
export const unitOf = (units: Unit[], position: Named): Unit | undefined =>
  units.find(u => u.code === (position.unit?.trim() || 'C62') && u.label === unitLabel(position));

/** Writes a unit into a position, its code and its label the way they are stored */
export function pick(position: Named, unit: Unit | undefined) {
  if (unit) {
    position.unit = unit.code;
    position.unit_label = unit.label;
  }
}

/** The unit a context's default names by its label; none, or a label the list lacks: the first */
export const defaultUnitOf = (units: Unit[], label: string | null | undefined): Unit | undefined =>
  units.find(u => u.label === label) ?? units[0];

/** The unit a new position starts with: the default of the context */
export const startUnit = (units: Unit[], label: string | null | undefined): Named => {
  const unit = defaultUnitOf(units, label);
  return { unit: unit?.code ?? null, unit_label: unit?.label ?? null };
};

/** What a code is called: the German name of a common code, else the English one */
export const catalogName = (entry: CatalogEntry): string => entry.de ?? entry.name;

/** `HUR – Stunde`, how the picker and the list write a code with its name */
export const describe = (entry: CatalogEntry): string => `${entry.code} – ${catalogName(entry)}`;

const fold = (value: string) => value.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();

interface Folded {
  code: string;
  names: string[];
}

// the folding of 2000 names is done once per entry, not on every key
const folded = new WeakMap<CatalogEntry, Folded>();

const foldedOf = (entry: CatalogEntry): Folded => {
  let key = folded.get(entry);
  if (!key) {
    key = {
      code: fold(entry.code),
      names: [entry.de, entry.name].flatMap(n => (n === undefined ? [] : [fold(n)])),
    };
    folded.set(entry, key);
  }
  return key;
};

const WORD_BREAKS = /[\s()[\],/-]+/;

/** Lower is better, undefined: no match. Every word of the query has to be found in the code or a name. */
function rank({ code, names }: Folded, query: string, words: string[]): number | undefined {
  const haystack = [code, ...names].join(' ');
  if (!words.every(word => haystack.includes(word))) {
    return undefined;
  }
  if (code === query) {
    return 0;
  }
  if (code.startsWith(query)) {
    return 1;
  }
  if (names.some(name => name.startsWith(query))) {
    return 2;
  }
  if (names.some(name => name.split(WORD_BREAKS).some(word => word.startsWith(query)))) {
    return 3;
  }
  if (names.some(name => name.includes(query))) {
    return 4;
  }
  return 5;
}

/**
 * The codes that match a search in the code, the German or the English name (case and umlauts
 * aside), best first and the common ones before others of the same quality. Nothing asked for:
 * the common ones.
 */
export function searchCatalog(catalog: CatalogEntry[], search: string): CatalogEntry[] {
  const query = fold(search.trim());
  if (query === '') {
    return catalog.filter(entry => entry.de !== undefined);
  }
  const words = query.split(/\s+/);
  return catalog
    .flatMap(entry => {
      const score = rank(foldedOf(entry), query, words);
      return score === undefined ? [] : [{ entry, score, common: entry.de === undefined ? 1 : 0 }];
    })
    .sort((a, b) => a.score - b.score || a.common - b.common)
    .map(({ entry }) => entry);
}
