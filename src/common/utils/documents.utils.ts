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
  | 'lapsed'
  | 'cancelled';

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
    ['cancelled', 'Storniert'],
  ],
  offer: [
    ['draft', 'Entwürfe'],
    ['valid', 'Gültig'],
    ['expired', 'Abgelaufen'],
    ['accepted', 'Angenommen'],
    ['declined', 'Abgelehnt'],
    ['superseded', 'Ersetzt'],
    ['lapsed', 'Alternative angenommen'],
  ],
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
  // a cancelled invoice and its Stornorechnung are settled with each other
  if (entry.cancelled || entry.corrects?.kind === 'cancel') {
    return 'cancelled';
  }
  return entry.paid_at ? 'paid' : 'open';
};

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
export function tagsOf(entry: ListEntry, mixed: boolean) {
  // an offer is not checked against rules, it is only made
  const created = entry.kind === 'offer' ? 'Erstellt' : 'Erstellt und geprüft';
  const status = statusOf(entry);
  return [
    mixed ? html`<wa-tag variant="neutral" size="s">${contextName(entry)}</wa-tag>` : null,
    ...status
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
    entry.status === 'exported'
      ? html`
          <wa-badge pill class="created" variant="brand" title="${created}">
            <wa-icon name="check" label="${created}"></wa-icon>
          </wa-badge>
        `
      : null,
    ...status
      .filter(t => t.mark)
      .map(
        t => html`
          <wa-badge pill class="payed" variant="${t.variant}" title="${t.detail ?? t.label}">
            <wa-icon name="check" label="${t.label}"></wa-icon>
          </wa-badge>
        `
      ),
  ].filter(tag => tag !== null);
}

export const yearOf = (entry: ListEntry) => entry.issue_date.slice(0, 4);
