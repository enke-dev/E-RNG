import type { Position, Rate } from './api.utils.js';
import { percent } from './format.utils.js';

type Named = Pick<Position, 'tax_rate' | 'tax_label'>;

const numeric = (rate: string) => Number(rate.trim().replace(',', '.'));

/** Whether two rates are the same number, `19` is `19,0` */
export const sameRate = (a: string, b: string) =>
  a.trim() !== '' && b.trim() !== '' && numeric(a) === numeric(b);

/** `Regelsatz (19 %)`, how the pickers and the lists write a rate with its label */
export const rateText = (rate: Pick<Rate, 'label' | 'rate'>): string =>
  `${rate.label} (${percent(rate.rate)})`;

/** The entry of the list that a context's default rate names: the rates of a list are unique */
export const defaultRateOf = (rates: Rate[], taxRate: string): Rate | undefined =>
  rates.find(r => sameRate(r.rate, taxRate));

/** The label a position was picked with, blank when it has none */
export const rateLabel = (position: Named): string => position.tax_label?.trim() ?? '';

/** The rate of a list that a position picked: value and label; none when it takes the default */
export const rateOf = (rates: Rate[], position: Named): Rate | undefined => {
  const picked = position.tax_rate;
  if (picked == null || picked.trim() === '') {
    return undefined;
  }
  return rates.find(r => sameRate(r.rate, picked) && r.label === rateLabel(position));
};

/** Writes a rate into a position the way it is stored, none: the position takes the default */
export function pickRate(position: Named, rate: Rate | undefined) {
  position.tax_rate = rate?.rate ?? null;
  position.tax_label = rate?.label ?? null;
}

/** What a position that picked a rate the list does not have shows: its own text, as it was picked */
export const pickedText = (position: Named): string => {
  const label = rateLabel(position);
  const rate = percent((position.tax_rate ?? '').trim());
  return label === '' ? rate : `${label} (${rate})`;
};

/**
 * What a position's picker shows: the entry of the list it picked, or the default's when it picked
 * none. A rate the list lacks (deleted, changed since, from an older document) is `missing`, the
 * picker gives it an entry of its own.
 */
export const rateChoice = (
  rates: Rate[],
  position: Named,
  defaultRate: string
): { selected?: Rate; missing?: { percent: string; text: string } } => {
  const picked = position.tax_rate?.trim() ?? '';
  const selected = picked === '' ? defaultRateOf(rates, defaultRate) : rateOf(rates, position);
  if (selected) {
    return { selected };
  }
  const own = picked === '' ? defaultRate.trim() : picked;
  return {
    missing: { percent: percent(own), text: picked === '' ? percent(own) : pickedText(position) },
  };
};

/** Whether the context charges VAT: a blank rate is none */
export const chargesVat = (taxRate: string): boolean => taxRate.trim() !== '';
