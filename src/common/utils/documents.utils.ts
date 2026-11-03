import { html, nothing } from 'lit';

import type {
  DocumentRef,
  Kind,
  ListEntry,
  Related,
  Relation,
  StoredInvoice,
} from './api.utils.js';
import { isClosing, parsed } from './billing.utils.js';
import { contextById, label } from './context.utils.js';
import { today } from './form.utils.js';
import { formatDate } from './format.utils.js';
import { validUntil } from './kind.utils.js';

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

/**
 * A created offer whose day has passed unanswered; one without a validity never expires, one out of
 * the running does not either
 */
export const expired = (entry: Standing) =>
  entry.status === 'exported' &&
  entry.answer === null &&
  !entry.superseded &&
  !entry.lapsed &&
  entry.valid_until !== null &&
  entry.valid_until < today();

/** The context as its tag names it everywhere, else the seller the invoice was made with */
const contextName = (entry: ListEntry) => {
  const context = contextById(entry.context_id);
  return context ? label(context) : entry.seller_name || '–';
};

/** What the status of a document is read from: a row of a list, or the document (`standingOf`) */
export type Standing = Related &
  Pick<
    ListEntry,
    | 'kind'
    | 'status'
    | 'archived_at'
    | 'on_account'
    | 'closing'
    | 'partial'
    | 'corrects'
    | 'valid_until'
    | 'paid_at'
    | 'paid'
    | 'answer'
    | 'answered_on'
  >;

/** A document read like a row of a list */
export const standingOf = (stored: StoredInvoice): Standing => ({
  ...stored,
  on_account: stored.draft.on_account ?? null,
  closing: isClosing(stored.draft),
  corrects: stored.draft.corrects ?? null,
  valid_until:
    stored.kind === 'offer'
      ? validUntil(stored.draft.issue_date, stored.draft.validity_days)
      : null,
  paid:
    stored.payments.length > 0
      ? String(stored.payments.reduce((sum, p) => sum + parsed(p.amount), 0))
      : null,
});

/**
 * One fact of a document's status. A row of a list says what (`label`), the document also says
 * which document or day (`detail`) and opens the document it names (`to`).
 */
export interface StatusTag {
  label: string;
  detail?: string;
  variant: 'neutral' | 'success' | 'warning' | 'danger';
  /** What it is rather than where it stands */
  outlined?: boolean;
  /** A row shows it as a round check mark */
  mark?: boolean;
  title?: string;
  to?: { kind: Kind; id: number };
}

/** Where a document stands, in the same words for a row of a list and for the document itself */
export function statusOf(doc: Standing): StatusTag[] {
  const named = (ref: DocumentRef | null | undefined, kind: Kind) =>
    ref ? { kind, id: ref.id } : undefined;
  const unanswered =
    doc.status === 'exported' && doc.answer === null && !doc.superseded && !doc.lapsed;
  const tags: (StatusTag | false | null)[] = [
    doc.archived_at !== null && { label: 'Archiviert', variant: 'neutral' },
    doc.relation !== null && {
      label: RELATIONS[doc.relation].tag,
      detail: RELATIONS[doc.relation].of(doc.related_number ?? ''),
      variant: 'neutral',
      outlined: true,
      title: RELATIONS[doc.relation].hint,
      to: doc.related_id === null ? undefined : { kind: 'offer', id: doc.related_id },
    },
    (doc.on_account !== null || doc.closing) && {
      label:
        doc.on_account === 'prepayment'
          ? 'Anzahlung'
          : doc.on_account === 'progress'
            ? 'Abschlag'
            : 'Schlussrechnung',
      variant: 'neutral',
      outlined: true,
    },
    doc.partial && { label: 'Teilrechnung', variant: 'neutral', outlined: true },
    doc.corrects !== null && {
      label: doc.corrects.kind === 'cancel' ? 'Storno' : 'Ersatz',
      detail: `${doc.corrects.kind === 'cancel' ? 'Storno zu' : 'Ersetzt'} ${doc.corrects.number}`,
      variant: 'neutral',
      outlined: true,
      to: { kind: 'invoice', id: doc.corrects.invoice_id },
    },
    unanswered &&
      doc.valid_until !== null && {
        label: expired(doc) ? 'Abgelaufen' : `Gültig bis ${formatDate(doc.valid_until)}`,
        detail: expired(doc) ? `Abgelaufen, gültig bis ${formatDate(doc.valid_until)}` : undefined,
        variant: expired(doc) ? 'warning' : 'neutral',
      },
    doc.superseded && {
      label: 'Ersetzt',
      detail: doc.superseded_by?.number ? `Ersetzt durch ${doc.superseded_by.number}` : undefined,
      variant: 'neutral',
      to: named(doc.superseded_by, 'offer'),
    },
    doc.lapsed && {
      label: 'Alternative angenommen',
      detail: doc.taken_by?.number ? `Alternative angenommen: ${doc.taken_by.number}` : undefined,
      variant: 'neutral',
      to: named(doc.taken_by, 'offer'),
    },
    doc.cancelled && {
      label: 'Storniert',
      detail: doc.cancelled_by?.number ? `Storniert durch ${doc.cancelled_by.number}` : undefined,
      variant: 'neutral',
      to: named(doc.cancelled_by, 'invoice'),
    },
    doc.cancel_draft != null && {
      label: 'Storno in Vorbereitung',
      variant: 'warning',
      title: 'Storniert ist sie erst, wenn die Stornorechnung erstellt ist',
      to: { kind: 'invoice', id: doc.cancel_draft },
    },
    doc.answer !== null && {
      label: doc.answer === 'accepted' ? 'Angenommen' : 'Abgelehnt',
      detail: `${doc.answer === 'accepted' ? 'Angenommen' : 'Abgelehnt'} am ${formatDate(doc.answered_on ?? '')}`,
      variant: doc.answer === 'accepted' ? 'success' : 'danger',
    },
    doc.paid !== null && doc.paid_at === null && { label: 'Teilweise bezahlt', variant: 'warning' },
    doc.paid_at !== null && {
      label: 'Bezahlt',
      detail: `Bezahlt am ${formatDate(doc.paid_at)}`,
      variant: 'success',
      mark: true,
    },
  ];
  return tags.filter(tag => tag !== false && tag !== null);
}

/** The status of a document on itself: with what it names, opening that */
export const detailTags = (tags: StatusTag[], open: (kind: Kind, id: number) => void) =>
  tags.map(
    t => html`
      <wa-tag
        variant="${t.variant}"
        appearance="${t.outlined ? 'outlined' : nothing}"
        class="${t.to ? 'ref' : nothing}"
        title="${t.title ?? nothing}"
        @click="${() => t.to && open(t.to.kind, t.to.id)}"
      >
        ${t.detail ?? t.label}
      </wa-tag>
    `
  );

/**
 * What a document is, as tags and check marks on a row of its own; the context only in a mixed list.
 * The badges are styled by `documents.utils.css`.
 */
/** The check marks of a document: created (and checked), paid or answered */
export function marksOf(entry: ListEntry) {
  // an offer is not checked against rules, it is only made
  const created = entry.kind === 'offer' ? 'Erstellt' : 'Erstellt und geprüft';
  return [
    entry.status === 'exported'
      ? html`
          <wa-badge pill class="created" variant="brand" title="${created}">
            <wa-icon name="check" label="${created}"></wa-icon>
          </wa-badge>
        `
      : null,
    ...statusOf(entry)
      .filter(t => t.mark)
      .map(
        t => html`
          <wa-badge pill class="payed" variant="${t.variant}" title="${t.detail ?? t.label}">
            <wa-icon name="check" label="${t.label}"></wa-icon>
          </wa-badge>
        `
      ),
  ].filter(mark => mark !== null);
}

/** The tags of a document's row, its check marks last unless `marks` is off (shown elsewhere) */
export function tagsOf(entry: ListEntry, mixed: boolean, marks = true) {
  return [
    mixed ? html`<wa-tag variant="neutral" size="s">${contextName(entry)}</wa-tag>` : null,
    ...statusOf(entry)
      .filter(t => !t.mark)
      .map(
        t => html`
          <wa-tag
            variant="${t.variant}"
            appearance="${t.outlined ? 'outlined' : nothing}"
            size="s"
            title="${t.detail ?? t.title ?? nothing}"
          >
            ${t.label}
          </wa-tag>
        `
      ),
    ...(marks ? marksOf(entry) : []),
  ].filter(tag => tag !== null);
}

export const yearOf = (entry: ListEntry) => entry.issue_date.slice(0, 4);
