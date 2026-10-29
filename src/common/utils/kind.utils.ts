import type { Kind, Settings } from './api.utils.js';

/** What a kind of document is called in the UI: invoices and offers share their views, not their words */
export interface KindTexts {
  /** The list in the menu and the heading of its views */
  list: string;
  /** What one document is called, for headings and dialogs */
  noun: string;
  newLabel: string;
  search: string;
  empty: string;
  none: string;
  dateLabel: string;
  /** The default headline of a draft, the file name of its PDF; the settings may write another */
  draftTitle: string;
  /** The button that makes the draft a document */
  create: string;
  createTitle: string;
  confirm: (number: string) => string;
  created: (number: string) => string;
  locked: string;
  archived: string;
  restored: string;
  cancelTitle: string;
  cancelText: string;
  missingContext: string;
}

export const KINDS: Record<Kind, KindTexts> = {
  invoice: {
    list: 'Rechnungen',
    noun: 'Rechnung',
    newLabel: 'Neue Rechnung',
    search: 'Rechnungen suchen',
    empty: 'Noch keine Rechnungen.',
    none: 'Keine Rechnung ausgewählt.',
    dateLabel: 'Rechnungsdatum',
    draftTitle: 'Rechnungsentwurf',
    create: 'Prüfen und erstellen',
    createTitle: 'Rechnung erstellen',
    confirm: number => `Rechnung ${number} erstellen? Danach ist sie gesperrt.`,
    created: number => `Rechnung ${number} erstellt und gesperrt.`,
    locked: 'Erstellte Rechnungen sind für alle Änderungen gesperrt.',
    archived: 'Die Rechnung ist archiviert und steht nicht mehr in der Liste.',
    restored: 'Die Rechnung ist wiederhergestellt und steht wieder in der Liste.',
    cancelTitle: 'Neue Rechnung abbrechen',
    cancelText: 'Die Eingaben dieser Rechnung gehen verloren.',
    missingContext: 'Für die Rechnung fehlt der Kontext.',
  },
  offer: {
    list: 'Angebote',
    noun: 'Angebot',
    newLabel: 'Neues Angebot',
    search: 'Angebote suchen',
    empty: 'Noch keine Angebote.',
    none: 'Kein Angebot ausgewählt.',
    dateLabel: 'Angebotsdatum',
    draftTitle: 'Angebotsentwurf',
    create: 'Angebot erstellen',
    createTitle: 'Angebot erstellen',
    confirm: number => `Angebot ${number} erstellen? Danach ist es gesperrt.`,
    created: number => `Angebot ${number} erstellt und gesperrt.`,
    locked: 'Erstellte Angebote sind für alle Änderungen gesperrt.',
    archived: 'Das Angebot ist archiviert und steht nicht mehr in der Liste.',
    restored: 'Das Angebot ist wiederhergestellt und steht wieder in der Liste.',
    cancelTitle: 'Neues Angebot abbrechen',
    cancelText: 'Die Eingaben dieses Angebots gehen verloren.',
    missingContext: 'Für das Angebot fehlt der Kontext.',
  },
};

/** The address of a kind's list, or of one of its documents (`new`, `none`, an id); `/` stays the invoices */
export function kindRoute(kind: Kind, id?: number | string): string {
  const base = kind === 'offer' ? '/offers' : '/invoices';
  if (id === undefined) {
    return kind === 'offer' ? base : '/';
  }
  return `${base}/${id}`;
}

/** The headline of a draft as the settings of its kind write it, else the default */
export function draftTitle(settings: Settings | undefined, kind: Kind): string {
  const own = (kind === 'offer' ? settings?.offer.wording : settings?.invoice_wording)?.draft_title;
  return own?.trim() || KINDS[kind].draftTitle;
}

const NOON = 'T12:00:00Z';

/** The day an offer dated `issue` is valid until, none without validity days or with no date to count from */
export function validUntil(issue: string, days: number | null): string | null {
  const start = new Date(`${issue}${NOON}`);
  if (days === null || Number.isNaN(start.getTime())) {
    return null;
  }
  start.setUTCDate(start.getUTCDate() + days);
  return start.toISOString().slice(0, 10);
}

/** The validity days a field holds: a whole number from zero, none while it is blank */
export function parseDays(value: string): number | null {
  const days = value.trim();
  return days === '' ? null : Math.max(0, Math.trunc(Number(days)) || 0);
}
