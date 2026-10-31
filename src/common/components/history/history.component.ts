import '@awesome.me/webawesome/dist/components/tag/tag.js';

import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { DocumentEvent, StoredInvoice } from '../../utils/api.utils.js';
import { api } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { formatDate, formatMoment, money } from '../../utils/format.utils.js';
import { KINDS } from '../../utils/kind.utils.js';
import styles from './history.component.css';

/** What a document an event points at is called: its kind and number, or its kind's draft */
const nameOf = (document: StoredInvoice | undefined) =>
  document
    ? document.number
      ? `${KINDS[document.kind].noun} ${document.number}`
      : KINDS[document.kind].draftTitle
    : 'einem gelöschten Entwurf';

/**
 * What happened to a document, or to all documents of a Vorgang (`case-id`), as a vertical timeline,
 * the latest first; a dashed line above it runs on to now. Each entry says what it was and on which
 * day, the day it is about first (a payment received earlier). In a Vorgang each entry says which
 * kind of document it was about ("Angebot angelegt") and shows the document's number as a tag.
 */
@customElement('erng-history')
export class HistoryComponent extends ErngElement.withStyles(styles) {
  @property({ type: Number, attribute: 'invoice-id' }) invoiceId: number | null = null;

  /** The history of a whole Vorgang instead of one document */
  @property({ type: Number, attribute: 'case-id' }) caseId: number | null = null;

  /** Raised by the editor whenever something may have happened, the history is read again */
  @property({ type: Number }) revision = 0;

  @state() private events: DocumentEvent[] | undefined;
  /** The documents events were made from or belong to (in a Vorgang), by id */
  @state() private sources = new Map<number, StoredInvoice | undefined>();

  protected override willUpdate(changed: PropertyValues<this>) {
    if (changed.has('invoiceId') || changed.has('caseId') || changed.has('revision')) {
      void this.load();
    }
  }

  private async load() {
    const { invoiceId, caseId } = this;
    try {
      const events =
        caseId !== null
          ? await api.caseEvents(caseId)
          : invoiceId !== null
            ? await api.invoiceEvents(invoiceId)
            : [];
      // what a copy or an invoice was made from is named, a deleted draft is gone
      const froms = [
        ...new Set(
          events.flatMap(e => [
            ...(e.from === undefined ? [] : [e.from]),
            ...(caseId === null ? [] : [e.invoice_id]),
          ])
        ),
      ];
      const sources = new Map(
        await Promise.all(
          froms.map(
            async from => [from, await api.invoiceGet(from).catch(() => undefined)] as const
          )
        )
      );
      if (invoiceId === this.invoiceId && caseId === this.caseId) {
        this.events = events;
        this.sources = sources;
      }
    } catch {
      // the history is extra, the document shows without it
      this.events = [];
    }
  }

  /** What happened; in a Vorgang with the kind of the document it happened to */
  private label(event: DocumentEvent): string {
    const inCase = this.caseId !== null;
    const document = this.sources.get(event.invoice_id);
    const noun = document ? KINDS[document.kind].noun : 'Beleg';
    const from = event.from === undefined ? '' : ` aus ${nameOf(this.sources.get(event.from))}`;
    switch (event.kind) {
      case 'created':
        return inCase ? `${noun} angelegt${from}` : `Angelegt${from}`;
      case 'draft-exported': {
        const what = inCase && document ? KINDS[document.kind].draftTitle : 'Entwurf';
        return `${what} exportiert · Version ${event.version ?? '?'}`;
      }
      case 'baked':
        return inCase ? `${noun} erstellt` : `Erstellt als ${event.number ?? ''}`;
      case 'payment':
        return `Zahlung über ${money(event.amount)} eingegangen`;
      case 'payment-removed':
        return `Zahlung über ${money(event.amount)} zurückgenommen`;
      case 'accepted':
        return inCase ? 'Angebot angenommen' : 'Angenommen';
      case 'declined':
        return inCase ? 'Angebot abgelehnt' : 'Abgelehnt';
      case 'answer-removed':
        return 'Antwort zurückgenommen';
      case 'archived':
        return inCase ? `${noun} archiviert` : 'Archiviert';
      case 'restored':
        return inCase ? `${noun} wiederhergestellt` : 'Wiederhergestellt';
    }
  }

  /** In a Vorgang: which document, by its number, a draft as such, a deleted one as gone */
  private tag(event: DocumentEvent) {
    const document = this.sources.get(event.invoice_id);
    const text = document ? (document.number ?? 'Entwurf') : 'gelöscht';
    return html`<wa-tag size="small" variant="neutral" appearance="outlined">${text}</wa-tag>`;
  }

  /** The day it is about where it has one, then when it was noted */
  private when(event: DocumentEvent): string {
    return event.on
      ? `${formatDate(event.on)} · erfasst ${formatMoment(event.at)}`
      : formatMoment(event.at);
  }

  override render() {
    if (!this.events) {
      return nothing;
    }
    if (this.events.length === 0) {
      return html`<p class="muted">Noch nichts verzeichnet.</p>`;
    }
    return html`
      <ol>
        ${[...this.events].reverse().map(
          e => html`
            <li class="${e.kind}">
              <span class="label">${this.label(e)}</span>
              ${this.caseId !== null ? this.tag(e) : nothing}
              <span class="when">${this.when(e)}</span>
            </li>
          `
        )}
      </ol>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-history': HistoryComponent;
  }
}
