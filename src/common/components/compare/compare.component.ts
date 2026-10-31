import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type {
  DocumentEvent,
  InvoiceDraft,
  Position,
  StoredInvoice,
  Totals,
} from '../../utils/api.utils.js';
import { api } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import type { DraftDiff, FieldChange } from '../../utils/diff.utils.js';
import { diffDrafts, diffWords } from '../../utils/diff.utils.js';
import { formatMoment, money } from '../../utils/format.utils.js';
import styles from './compare.component.css';

/** What can be compared: a kept version by the id of its event, or the draft as it is now */
export type VersionKey = number | 'current';

/** A version as it is offered to pick */
export const versionLabel = (event: DocumentEvent) =>
  event.kind === 'baked'
    ? `Erstellt als ${event.number ?? ''}`
    : event.kind === 'created'
      ? `Angelegt · ${formatMoment(event.at)}`
      : `Version ${event.version ?? '?'} · ${formatMoment(event.at)}`;

/** One side of a comparison: the document's data and what it adds up to */
interface Side {
  draft: InvoiceDraft;
  totals: Totals | null;
}

/**
 * Two versions of a document side by side, as what changed from the one to the other: the fields
 * word by word, the positions added, removed and changed, the totals. The versions are the ones
 * kept when it was created, with the draft exports and the baking, and while it is a draft the
 * document as it is now.
 */
@customElement('erng-compare')
export class CompareComponent extends ErngElement.withStyles(styles) {
  @property({ attribute: false }) invoice: StoredInvoice | undefined;

  /** The events that keep a version, oldest first */
  @property({ attribute: false }) versions: DocumentEvent[] = [];

  /** The draft as it is now, offered as the newest version; none for a created document */
  @property({ attribute: false }) current: InvoiceDraft | undefined;

  /** What the versions hold where it is known already, by the id of their event */
  @property({ attribute: false }) snapshots = new Map<number, InvoiceDraft>();

  @property({ type: Boolean, reflect: true }) open = false;
  @property({ attribute: false }) from: VersionKey | undefined;
  @property({ attribute: false }) to: VersionKey | undefined;

  @state() private diff: DraftDiff | undefined;
  @state() private totals: [Totals | null, Totals | null] = [null, null];

  protected override willUpdate(changed: PropertyValues<this>) {
    if (
      this.open &&
      (changed.has('open') || changed.has('from') || changed.has('to') || changed.has('current'))
    ) {
      void this.compare();
    }
  }

  private async snapshot(id: number): Promise<InvoiceDraft> {
    return this.snapshots.get(id) ?? (await api.invoiceSnapshot(id));
  }

  private async side(key: VersionKey | undefined): Promise<Side | undefined> {
    const { invoice } = this;
    const draft =
      key === 'current' ? this.current : key === undefined ? undefined : await this.snapshot(key);
    if (!invoice || !draft) {
      return undefined;
    }
    const totals = await api
      .invoiceCheck(invoice.context_id, invoice.kind, draft)
      .then(check => check.totals)
      .catch(() => null);
    return { draft, totals };
  }

  private async compare() {
    const { from, to } = this;
    try {
      const [before, after] = await Promise.all([this.side(from), this.side(to)]);
      if (from !== this.from || to !== this.to) {
        return;
      }
      this.diff = before && after ? diffDrafts(before.draft, after.draft) : undefined;
      this.totals = [before?.totals ?? null, after?.totals ?? null];
    } catch {
      this.diff = undefined;
    }
  }

  private pick(side: 'from' | 'to', event: Event) {
    const value = String((event.target as WaSelect).value);
    this[side] = value === 'current' ? 'current' : Number(value);
  }

  private renderSelect(label: string, side: 'from' | 'to') {
    const value = this[side];
    return html`
      <wa-select
        label="${label}"
        size="s"
        .value="${value === undefined ? '' : String(value)}"
        @change="${(e: Event) => this.pick(side, e)}"
      >
        ${this.versions.map(
          v => html`<wa-option value="${String(v.id)}">${versionLabel(v)}</wa-option>`
        )}
        ${this.current ? html`<wa-option value="current">Aktueller Stand</wa-option>` : nothing}
      </wa-select>
    `;
  }

  /** A before and an after: what went is struck through, what came is marked; a text word by word */
  private renderChange({ before, after, words }: Omit<FieldChange, 'label'>) {
    if (!words) {
      return html`
        ${before ? html`<del>${before}</del>` : nothing}
        ${before && after ? html`<span class="to">→</span>` : nothing}
        ${after ? html`<ins>${after}</ins>` : nothing}
      `;
    }
    return diffWords(before, after).map(s =>
      s.change === 'removed'
        ? html`<del>${s.text}</del>`
        : s.change === 'added'
          ? html`<ins>${s.text}</ins>`
          : s.text
    );
  }

  private renderFields(fields: FieldChange[]) {
    return html`
      <dl>
        ${fields.map(
          f => html`
            <dt>${f.label}</dt>
            <dd class="${f.words ? 'text' : ''}">${this.renderChange(f)}</dd>
          `
        )}
      </dl>
    `;
  }

  private renderTotals() {
    const [before, after] = this.totals;
    if (!before || !after || (before.net === after.net && before.gross === after.gross)) {
      return nothing;
    }
    return html`
      <section>
        <h3>Summe</h3>
        ${this.renderFields([
          { label: 'Netto', before: money(before.net), after: money(after.net) },
          { label: 'Brutto', before: money(before.gross), after: money(after.gross) },
        ])}
      </section>
    `;
  }

  /** A position only one of the versions has, as a whole */
  private renderWhole(position: Position, change: 'added' | 'removed') {
    const unit = position.unit_label ?? position.unit ?? '';
    const quantity = [position.quantity ?? '', unit].filter(Boolean).join(' ');
    const [before, after] =
      change === 'added' ? ['', position.description] : [position.description, ''];
    return html`
      <li class="${change}">
        <span class="what">${change === 'added' ? 'Neu' : 'Entfernt'}</span>
        <strong>${this.renderChange({ before, after, words: true })}</strong>
        <span class="muted">${quantity} × ${money(position.price)}</span>
      </li>
    `;
  }

  private renderPositions(diff: DraftDiff) {
    if (diff.positions.length === 0) {
      return nothing;
    }
    return html`
      <section>
        <h3>Positionen</h3>
        <ul class="positions">
          ${diff.positions.map(p =>
            p.change === 'changed'
              ? html`
                  <li class="changed">
                    <span class="what">Geändert</span>
                    <strong>${p.after.description || p.before.description}</strong>
                    ${this.renderFields(p.fields)}
                  </li>
                `
              : this.renderWhole(p.change === 'added' ? p.after : p.before, p.change)
          )}
        </ul>
        ${diff.unchanged > 0 ? html`<p class="muted">${diff.unchanged} unverändert</p>` : nothing}
      </section>
    `;
  }

  private renderDiff() {
    const { diff } = this;
    if (!diff) {
      return nothing;
    }
    if (diff.fields.length === 0 && diff.positions.length === 0) {
      return html`<p class="muted">Keine Unterschiede.</p>`;
    }
    return html`
      ${
        diff.fields.length > 0
          ? html`
              <section>
                <h3>Angaben</h3>
                ${this.renderFields(diff.fields)}
              </section>
            `
          : nothing
      }
      ${this.renderPositions(diff)} ${this.renderTotals()}
    `;
  }

  override render() {
    return html`
      <wa-dialog
        label="Versionen vergleichen"
        ?open="${this.open}"
        @wa-after-hide="${(e: Event) => {
          if (e.target === e.currentTarget) {
            this.open = false;
            this.dispatchEvent(new CustomEvent('close'));
          }
        }}"
      >
        <div class="stack">
          <div class="pick">
            ${this.renderSelect('Von', 'from')} ${this.renderSelect('Bis', 'to')}
          </div>
          ${this.renderDiff()}
        </div>
        <div slot="footer" class="dialog-footer">
          <wa-button appearance="outlined" @click="${() => (this.open = false)}"
            >Schließen</wa-button
          >
        </div>
      </wa-dialog>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-compare': CompareComponent;
  }
}
