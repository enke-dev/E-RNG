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
 * What happened to a document, as a vertical timeline in the order it was recorded: each entry
 * says what it was and on which day, the day it is about first (a payment received earlier).
 */
@customElement('erng-history')
export class HistoryComponent extends ErngElement.withStyles(styles) {
  @property({ type: Number, attribute: 'invoice-id' }) invoiceId!: number;

  /** Raised by the editor whenever something may have happened, the history is read again */
  @property({ type: Number }) revision = 0;

  @state() private events: DocumentEvent[] | undefined;
  /** The documents events were made from, by id */
  @state() private sources = new Map<number, StoredInvoice | undefined>();

  protected override willUpdate(changed: PropertyValues<this>) {
    if (changed.has('invoiceId') || changed.has('revision')) {
      void this.load();
    }
  }

  private async load() {
    const id = this.invoiceId;
    try {
      const events = await api.invoiceEvents(id);
      // what a copy or an invoice was made from is named, a deleted draft is gone
      const froms = [...new Set(events.flatMap(e => (e.from === undefined ? [] : [e.from])))];
      const sources = new Map(
        await Promise.all(
          froms.map(
            async from => [from, await api.invoiceGet(from).catch(() => undefined)] as const
          )
        )
      );
      if (id === this.invoiceId) {
        this.events = events;
        this.sources = sources;
      }
    } catch {
      // the history is extra, the document shows without it
      this.events = [];
    }
  }

  private label(event: DocumentEvent): string {
    switch (event.kind) {
      case 'created':
        return event.from === undefined
          ? 'Angelegt'
          : `Angelegt aus ${nameOf(this.sources.get(event.from))}`;
      case 'draft-exported':
        return `Entwurf exportiert · Version ${event.version ?? '?'}`;
      case 'baked':
        return `Erstellt als ${event.number ?? ''}`;
      case 'payment':
        return `Zahlung über ${money(event.amount)} eingegangen`;
      case 'payment-removed':
        return `Zahlung über ${money(event.amount)} zurückgenommen`;
      case 'accepted':
        return 'Angenommen';
      case 'declined':
        return 'Abgelehnt';
      case 'answer-removed':
        return 'Antwort zurückgenommen';
      case 'archived':
        return 'Archiviert';
      case 'restored':
        return 'Wiederhergestellt';
    }
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
        ${this.events.map(
          e => html`
            <li class="${e.kind}">
              <span class="label">${this.label(e)}</span>
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
