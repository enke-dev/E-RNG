import { html } from 'lit';

import type { Kind, ListEntry, Relation } from './api.utils.js';
import { contextById, label } from './context.utils.js';
import { today } from './form.utils.js';
import { formatDate } from './format.utils.js';
import { KINDS, otherKind } from './kind.utils.js';

/**
 * Where a document stands: a draft, or created. An invoice is open until it is paid, an offer valid
 * until its day has passed or the customer answered, or until it is out of the running: replaced
 * by a created Ersatzangebot, or lapsed because another offer of its alternatives was accepted.
 */
export type Stage =
  | 'draft'
  | 'open'
  | 'paid'
  | 'valid'
  | 'expired'
  | 'accepted'
  | 'declined'
  | 'superseded'
  | 'lapsed';

/** How an offer that refers to another is called: as a tag, as a document, on the row naming it */
export const RELATIONS: Record<
  Relation,
  { tag: string; noun: string; to: string; of: (number: string) => string; hint: string }
> = {
  supplement: {
    tag: 'Nachtrag',
    noun: 'Nachtragsangebot',
    to: 'zu',
    of: number => `Nachtrag zu ${number}`,
    hint: 'Mehr Leistung zu diesem Angebot',
  },
  alternative: {
    tag: 'Alternative',
    noun: 'Alternativangebot',
    to: 'zu',
    of: number => `Alternative zu ${number}`,
    hint: 'Eine Variante, nur ein Angebot der Gruppe wird angenommen',
  },
  replacement: {
    tag: 'Ersatz',
    noun: 'Ersatzangebot',
    to: 'für',
    of: number => `Ersetzt ${number}`,
    hint: 'Ersetzt dieses Angebot, sobald es erstellt ist',
  },
};

export const STAGES: Record<Kind, [Stage, string][]> = {
  invoice: [
    ['draft', 'Entwürfe'],
    ['open', 'Offen'],
    ['paid', 'Bezahlt'],
  ],
  offer: [
    ['draft', 'Entwürfe'],
    ['valid', 'Gültig'],
    ['expired', 'Abgelaufen'],
    ['accepted', 'Angenommen'],
    ['declined', 'Abgelehnt'],
    ['superseded', 'Ersetzt'],
    ['lapsed', 'Entfallen'],
  ],
};

/**
 * A created offer whose day has passed unanswered; one without a validity never expires, one out of
 * the running does not either
 */
export const expired = (entry: ListEntry) =>
  entry.status === 'exported' &&
  entry.answer === null &&
  !entry.superseded &&
  !entry.lapsed &&
  entry.valid_until !== null &&
  entry.valid_until < today();

export const stageOf = (entry: ListEntry): Stage => {
  if (entry.status === 'draft') {
    return 'draft';
  }
  if (entry.kind === 'offer') {
    return (
      entry.answer ??
      (entry.superseded
        ? 'superseded'
        : entry.lapsed
          ? 'lapsed'
          : expired(entry)
            ? 'expired'
            : 'valid')
    );
  }
  return entry.paid_at ? 'paid' : 'open';
};

/** The context as its tag names it everywhere, else the seller the invoice was made with */
const contextName = (entry: ListEntry) => {
  const context = contextById(entry.context_id);
  return context ? label(context) : entry.seller_name || '–';
};

/**
 * What a document is, as tags and check marks on a row of its own; the context only in a mixed list.
 * The badges are styled by `documents.utils.css`.
 */
export function tagsOf(entry: ListEntry, mixed: boolean) {
  // an offer is not checked against rules, it is only made
  const created = entry.kind === 'offer' ? 'Erstellt' : 'Erstellt und geprüft';
  return [
    mixed ? html`<wa-tag variant="neutral" size="s">${contextName(entry)}</wa-tag>` : null,
    entry.archived_at ? html`<wa-tag variant="neutral" size="s">Archiviert</wa-tag>` : null,
    entry.relation
      ? html`
          <wa-tag
            variant="neutral"
            appearance="outlined"
            size="s"
            title="${RELATIONS[entry.relation].of(entry.related_number ?? '')}"
          >
            ${RELATIONS[entry.relation].tag}
          </wa-tag>
        `
      : null,
    entry.on_account || entry.closing
      ? html`
          <wa-tag variant="neutral" appearance="outlined" size="s">
            ${
              entry.on_account === 'prepayment'
                ? 'Anzahlung'
                : entry.on_account === 'progress'
                  ? 'Abschlag'
                  : 'Schlussrechnung'
            }
          </wa-tag>
        `
      : null,
    expired(entry) ? html`<wa-tag variant="neutral" size="s">Abgelaufen</wa-tag>` : null,
    entry.superseded ? html`<wa-tag variant="neutral" size="s">Ersetzt</wa-tag>` : null,
    entry.lapsed ? html`<wa-tag variant="neutral" size="s">Entfallen</wa-tag>` : null,
    entry.answer === 'accepted'
      ? html`<wa-tag variant="success" size="s">Angenommen</wa-tag>`
      : entry.answer === 'declined'
        ? html`<wa-tag variant="danger" size="s">Abgelehnt</wa-tag>`
        : null,
    entry.paid !== null && entry.paid_at === null
      ? html`<wa-tag variant="warning" size="s">Teilweise bezahlt</wa-tag>`
      : null,
    entry.status === 'exported'
      ? html`
          <wa-badge pill class="created" variant="brand" title="${created}">
            <wa-icon name="check" label="${created}"></wa-icon>
          </wa-badge>
        `
      : null,
    entry.paid_at
      ? html`
          <wa-badge
            pill
            class="payed"
            variant="success"
            title="Bezahlt am ${formatDate(entry.paid_at)}"
          >
            <wa-icon name="check" label="Bezahlt"></wa-icon>
          </wa-badge>
        `
      : null,
    entry.linked > 0
      ? html`
          <wa-badge
            pill
            class="linked"
            variant="neutral"
            title="${KINDS[otherKind(entry.kind)].linkedTo(entry.linked)}"
          >
            <wa-icon
              name="link"
              label="${KINDS[otherKind(entry.kind)].linkedTo(entry.linked)}"
            ></wa-icon>
          </wa-badge>
        `
      : null,
  ].filter(tag => tag !== null);
}

export const yearOf = (entry: ListEntry) => entry.issue_date.slice(0, 4);
