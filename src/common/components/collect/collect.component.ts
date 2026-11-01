import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { Case } from '../../utils/api.utils.js';
import { api } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { parsed } from '../../utils/billing.utils.js';
import { CASE_STAGES, caseName, casesRoute } from '../../utils/cases.utils.js';
import { notifyError } from '../../utils/notify.utils.js';
import { goto } from '../../utils/routing.utils.js';
import styles from './collect.component.css';

/** Whose Vorgänge a Sammelrechnung may join: one customer in one context */
const ownerOf = (c: Case) => `${c.context_id}:${c.customer_id}`;

/**
 * A Sammelrechnung: Vorgänge of one customer that still have something open in their offers, joined
 * by a new Vorgang with one invoice. From a Vorgang's page that Vorgang is in it for sure; from the
 * list the customer is picked first. A Vorgang that collects others is never collected again.
 *
 * @fires close - the dialog closed
 */
@customElement('erng-collect')
export class CollectComponent extends ErngElement.withStyles(styles) {
  /** The Vorgang it starts from, always part of it */
  @property({ type: Number, attribute: 'case-id' }) caseId: number | undefined;
  @property({ type: Boolean, reflect: true }) open = false;

  /** Vorgänge with something open in their offers, collecting none */
  @state() private candidates: Case[] | undefined;
  @state() private owner = '';
  @state() private picked = new Set<number>();

  protected override willUpdate(changed: PropertyValues<this>) {
    if (changed.has('open') && this.open) {
      void this.load();
    }
  }

  private async load() {
    this.candidates = undefined;
    try {
      const all = (await api.casesList(null)).filter(
        c => c.customer_id !== null && !c.archived && !c.sources.some(r => r.link === 'collect')
      );
      const open = await Promise.all(
        all.map(async c =>
          (await api.caseBilling(c.id)).some(b => b.positions.some(p => parsed(p.open) > 0))
        )
      );
      this.candidates = all.filter((_, i) => open[i]);
      const start = this.candidates.find(c => c.id === this.caseId);
      this.owner = start ? ownerOf(start) : (this.owners[0]?.key ?? '');
      this.picked = new Set(start ? [start.id] : []);
    } catch (error) {
      notifyError(error);
    }
  }

  /** The customers with at least two Vorgänge to join */
  private get owners() {
    const counted = (this.candidates ?? []).reduce((by, c) => {
      const key = ownerOf(c);
      by.set(key, { key, name: c.customer_name, count: (by.get(key)?.count ?? 0) + 1 });
      return by;
    }, new Map<string, { key: string; name: string; count: number }>());
    return [...counted.values()].filter(o => o.count > 1);
  }

  private close() {
    this.open = false;
    this.dispatchEvent(new CustomEvent('close'));
  }

  private toggle(id: number, on: boolean) {
    const picked = new Set(this.picked);
    if (on) {
      picked.add(id);
    } else {
      picked.delete(id);
    }
    this.picked = picked;
  }

  private async collect() {
    try {
      const invoice = await api.casesCollect([...this.picked]);
      this.close();
      this.dispatchEvent(new CustomEvent('case-changed', { bubbles: true, composed: true }));
      goto(casesRoute('invoice', invoice.id));
    } catch (error) {
      notifyError(error);
    }
  }

  private renderChoices() {
    if (this.candidates === undefined) {
      return html`<p class="muted">Vorgänge werden gelesen …</p>`;
    }
    const shown = this.candidates.filter(c => ownerOf(c) === this.owner);
    if (shown.length < 2) {
      return html`
        <p>
          ${
            this.caseId === undefined
              ? 'Kein Kunde hat zwei Vorgänge mit offenen Angebotspositionen.'
              : 'Der Kunde hat keinen weiteren Vorgang mit offenen Angebotspositionen.'
          }
        </p>
      `;
    }
    return html`
      <div class="choices">
        ${shown.map(
          c => html`
            <wa-checkbox
              ?checked="${this.picked.has(c.id)}"
              ?disabled="${c.id === this.caseId}"
              @change="${(e: Event) => this.toggle(c.id, (e.target as HTMLInputElement).checked)}"
            >
              ${c.number} · ${caseName(c)}
              <span class="muted">${CASE_STAGES[c.stage]}</span>
            </wa-checkbox>
          `
        )}
      </div>
    `;
  }

  override render() {
    const owners = this.owners;
    return html`
      <wa-dialog
        label="Sammelrechnung"
        ?open="${this.open}"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && this.close()}"
      >
        <div class="stack">
          <p class="muted">
            Ein neuer Vorgang fasst die gewählten zusammen, mit einer Rechnung über alles, was in
            ihren Angeboten noch offen ist.
          </p>
          ${
            this.caseId === undefined && owners.length > 0
              ? html`
                  <wa-select
                    label="Kunde"
                    size="s"
                    .value="${this.owner}"
                    @change="${(e: Event) => {
                      this.owner = String((e.target as WaSelect).value);
                      this.picked = new Set();
                    }}"
                  >
                    ${owners.map(o => html`<wa-option value="${o.key}">${o.name}</wa-option>`)}
                  </wa-select>
                `
              : nothing
          }
          ${this.renderChoices()}
        </div>
        <div slot="footer" class="dialog-footer">
          <wa-button appearance="outlined" @click="${this.close}">Abbrechen</wa-button>
          <wa-button variant="brand" ?disabled="${this.picked.size < 2}" @click="${this.collect}">
            Sammelrechnung anlegen
          </wa-button>
        </div>
      </wa-dialog>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-collect': CollectComponent;
  }
}
