import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/icon/icon.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';

import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { DocumentEvent, InvoiceDraft, Kind, StoredInvoice } from '../../utils/api.utils.js';
import { api } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { casesRoute } from '../../utils/cases.utils.js';
import { diffDrafts } from '../../utils/diff.utils.js';
import { RELATIONS } from '../../utils/documents.utils.js';
import { formatDate, formatMoment, money } from '../../utils/format.utils.js';
import { KINDS } from '../../utils/kind.utils.js';
import { goto } from '../../utils/routing.utils.js';
import type { VersionKey } from '../compare/compare.component.js';
import { versionLabel } from '../compare/compare.component.js';
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
 * In the history of one document each kept version can be compared with the next newer one, the
 * newest of a draft with the draft as it is now; only where they differ. `compareNewest` opens the
 * comparison of the two newest, for a button beside the history.
 *
 * @fires comparable-change - whether two versions can be compared (`detail`), after each read
 */
@customElement('erng-history')
export class HistoryComponent extends ErngElement.withStyles(styles) {
  @property({ type: Number, attribute: 'invoice-id' }) invoiceId: number | null = null;

  /** The history of a whole Vorgang instead of one document */
  @property({ type: Number, attribute: 'case-id' }) caseId: number | null = null;

  /** Raised by the editor whenever something may have happened, the history is read again */
  @property({ type: Number }) revision = 0;

  /** Where a document named by an entry opens: by its id in the Vorgänge, or as the view says */
  @property({ attribute: false }) route: (kind: Kind, id: number) => string = casesRoute;

  /** The draft as it is now, to compare the kept versions with; none for a created document */
  @property({ attribute: false }) current: InvoiceDraft | undefined;

  @state() private events: DocumentEvent[] | undefined;
  /** The documents events were made from or belong to (in a Vorgang), by id */
  @state() private sources = new Map<number, StoredInvoice | undefined>();
  /** The document itself, in the history of one */
  @state() private invoice: StoredInvoice | undefined;
  /** What the kept versions hold, by the id of their event */
  @state() private snapshots = new Map<number, InvoiceDraft>();
  /** The two versions being compared, while they are */
  @state() private comparing: [VersionKey, VersionKey] | undefined;

  protected override willUpdate(changed: PropertyValues<this>) {
    if (changed.has('invoiceId') || changed.has('caseId') || changed.has('revision')) {
      void this.load();
    }
  }

  // untyped: `events` is private
  protected override updated(changed: PropertyValues) {
    if (changed.has('events') || changed.has('current')) {
      this.dispatchEvent(
        new CustomEvent<boolean>('comparable-change', { detail: this.keys.length >= 2 })
      );
    }
  }

  /** What can be compared, oldest first: the kept versions and, while a draft, the draft now */
  private get keys(): VersionKey[] {
    return [...this.versions.map(v => v.id), ...(this.current ? ['current' as const] : [])];
  }

  /** Compares the two newest versions, the dialog picks any others */
  compareNewest() {
    const [from, to] = this.keys.slice(-2);
    if (from !== undefined && to !== undefined) {
      this.comparing = [from, to];
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
      const invoice =
        caseId === null && invoiceId !== null
          ? await api.invoiceGet(invoiceId).catch(() => undefined)
          : undefined;
      // the versions are read up front: only those that differ are offered to compare
      const kept = caseId === null ? events.filter(e => e.has_snapshot) : [];
      const snapshots = new Map(
        (
          await Promise.all(
            kept.map(
              async e => [e.id, await api.invoiceSnapshot(e.id).catch(() => undefined)] as const
            )
          )
        ).filter((entry): entry is [number, InvoiceDraft] => entry[1] !== undefined)
      );
      if (invoiceId === this.invoiceId && caseId === this.caseId) {
        this.events = events;
        this.sources = sources;
        this.invoice = invoice;
        this.snapshots = snapshots;
      }
    } catch {
      // the history is extra, the document shows without it
      this.events = [];
    }
  }

  /** A document an entry names, opening it; a deleted draft as gone */
  private named(id: number) {
    const document = this.sources.get(id);
    return document
      ? html`
          <a
            class="ref"
            href="${this.route(document.kind, document.id)}"
            @click="${(e: Event) => {
              e.preventDefault();
              goto(this.route(document.kind, document.id));
            }}"
          >
            ${nameOf(document)}
          </a>
        `
      : nameOf(document);
  }

  /** What happened; in a Vorgang with the kind of the document it happened to */
  private label(event: DocumentEvent) {
    const inCase = this.caseId !== null;
    const document = this.sources.get(event.invoice_id);
    const noun = document ? KINDS[document.kind].noun : 'Dokument';
    const from = event.from === undefined ? '' : html` aus ${this.named(event.from)}`;
    // an offer made for another says how it refers to it
    const relation = (inCase ? document : this.invoice)?.relation;
    switch (event.kind) {
      case 'created':
        if (relation && event.from !== undefined) {
          const { noun: as, to } = RELATIONS[relation];
          const source = this.named(event.from);
          return inCase
            ? html`${as} angelegt ${to} ${source}`
            : html`Angelegt als ${as} ${to} ${source}`;
        }
        return inCase ? html`${noun} angelegt${from}` : html`Angelegt${from}`;
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

  /** In a Vorgang: which document, by its number, a draft as such, opening it; a deleted one as gone */
  private tag(event: DocumentEvent) {
    const document = this.sources.get(event.invoice_id);
    if (!document) {
      return html`<wa-tag size="small" variant="neutral" appearance="outlined">gelöscht</wa-tag>`;
    }
    return html`
      <wa-tag
        size="small"
        variant="neutral"
        appearance="outlined"
        class="ref"
        title="${nameOf(document)} öffnen"
        @click="${() => goto(this.route(document.kind, document.id))}"
      >
        ${document.number ?? 'Entwurf'}
      </wa-tag>
    `;
  }

  /** The events that keep a version of the document, oldest first */
  private get versions(): DocumentEvent[] {
    return this.caseId === null ? (this.events ?? []).filter(e => e.has_snapshot) : [];
  }

  /** What a kept version is compared with: the next newer one, for the newest the draft now */
  private nextOf(event: DocumentEvent): VersionKey | undefined {
    const versions = this.versions;
    const next = versions[versions.findIndex(v => v.id === event.id) + 1];
    return next ? next.id : this.current ? 'current' : undefined;
  }

  private draftOf(key: VersionKey): InvoiceDraft | undefined {
    return key === 'current' ? this.current : this.snapshots.get(key);
  }

  private renderCompare(event: DocumentEvent) {
    const next = event.has_snapshot && this.caseId === null ? this.nextOf(event) : undefined;
    const [before, after] = [
      this.draftOf(event.id),
      next === undefined ? undefined : this.draftOf(next),
    ];
    if (next === undefined || !before || !after) {
      return nothing;
    }
    const diff = diffDrafts(before, after);
    if (diff.fields.length === 0 && diff.positions.length === 0) {
      return nothing;
    }
    const label = `Vergleichen mit ${next === 'current' ? 'dem aktuellen Stand' : versionLabel(this.versions.find(v => v.id === next) as DocumentEvent)}`;
    return html`
      <wa-button
        class="compare"
        appearance="plain"
        size="s"
        title="${label}"
        @click="${() => (this.comparing = [event.id, next])}"
      >
        <wa-icon name="compare" label="${label}"></wa-icon>
      </wa-button>
    `;
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
              <span class="label">${this.label(e)} ${this.renderCompare(e)}</span>
              ${this.caseId !== null ? this.tag(e) : nothing}
              <span class="when">${this.when(e)}</span>
            </li>
          `
        )}
      </ol>
      <erng-compare
        .invoice="${this.invoice}"
        .versions="${this.versions}"
        .current="${this.current}"
        .snapshots="${this.snapshots}"
        .from="${this.comparing?.[0]}"
        .to="${this.comparing?.[1]}"
        ?open="${this.comparing !== undefined}"
        @close="${() => (this.comparing = undefined)}"
      ></erng-compare>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-history': HistoryComponent;
  }
}
