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

/**
 * Without time, only the date is shown
 * `2026-10-17` → `17.10.2026`
 * `2026-10-17 15:32:09` → `17.10.2026`
 * `2026-10-17T15:32:09.765Z` → `17.10.2026`
 *
 * With time, the date and time are shown as `DD.MM.YYYY HH:MM`
 * `2026-10-17 15:32:09` → `17.10.2026 um 15:32`
 * `2026-10-17T15:32:09.765Z` → `17.10.2026 um 15:32`
 */
export function formatDate(iso: string, withTime = false): string {
  const [datePart = '', timePart = ''] = iso.split(/[\sT]/);
  const [year, month, day] = datePart.split('-');
  const date = year && month && day ? `${day}.${month}.${year}` : datePart;
  if (!withTime) {
    return date;
  }

  const [hours = '00', minutes = '00'] = timePart.split(':');
  return `${date} um ${hours}:${minutes}`;
}

export const UNITS = [
  { code: 'C62', label: 'Stk.' },
  { code: 'HUR', label: 'Std.' },
  { code: 'DAY', label: 'Tag(e)' },
  { code: 'MON', label: 'Monat(e)' },
  { code: 'LS', label: 'pauschal' },
] as const;
