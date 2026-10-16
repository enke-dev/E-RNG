/** `1181.08` → `1.181,08 €`, the backend sends exact decimal strings */
export function money(amount: string | null | undefined): string {
  if (amount === null || amount === undefined || amount === '') {
    return '–';
  }
  const [int = '0', frac = ''] = amount.replace('-', '').split('.');
  const grouped = int.replace(/\B(?=(\d{3})+(?!\d))/g, '.');
  const sign = amount.startsWith('-') ? '-' : '';
  return `${sign}${grouped},${frac.padEnd(2, '0')} €`;
}

/** `2026-10-17` → `17.10.2026` */
export function date(iso: string): string {
  const [year, month, day] = iso.split('-');
  return year && month && day ? `${day}.${month}.${year}` : iso;
}

export const UNITS = [
  { code: 'C62', label: 'Stk.' },
  { code: 'HUR', label: 'Std.' },
  { code: 'DAY', label: 'Tag(e)' },
  { code: 'MON', label: 'Monat(e)' },
  { code: 'LS', label: 'pauschal' },
] as const;
