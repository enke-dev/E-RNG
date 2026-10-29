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

/** `19` → `19 %`, `5.5` → `5,5 %`: the German default of the documents, with a no-break space */
export function percent(rate: string): string {
  return `${rate.replace('.', ',')}%`;
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

/** A point in time as the user's clock shows it: `2026-10-29T21:15:00Z` → `29.10.2026 um 22:15` (in CET) */
export function formatMoment(iso: string): string {
  const moment = new Date(iso);
  if (Number.isNaN(moment.getTime())) {
    return iso;
  }
  const time = moment.toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });
  return `${moment.toLocaleDateString('de-DE')} um ${time}`;
}

/** `1536000` → `1,5MB`, for the size of a file */
export function megabytes(bytes: number): string {
  const size = bytes / (1024 * 1024);
  return `${size.toLocaleString('de-DE', { maximumFractionDigits: size < 10 ? 1 : 0 })}MB`;
}

export const UNITS = [
  { code: 'C62', label: 'Stk.' },
  { code: 'HUR', label: 'Std.' },
  { code: 'DAY', label: 'Tag(e)' },
  { code: 'MON', label: 'Monat(e)' },
  { code: 'LS', label: 'pauschal' },
] as const;
