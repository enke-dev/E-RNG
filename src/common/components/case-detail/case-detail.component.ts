import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/card/card.js';
import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';
import '../history/history.component.js';
import '../page/page.component.js';

import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { Case, CaseRef, Kind } from '../../utils/api.utils.js';
import { api } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import {
  CASE_STAGES,
  caseName,
  caseRoute,
  casesRoute,
  STAGE_VARIANTS,
} from '../../utils/cases.utils.js';
import { text } from '../../utils/form.utils.js';
import { formatDate, money } from '../../utils/format.utils.js';
import { KINDS } from '../../utils/kind.utils.js';
import { notifyError } from '../../utils/notify.utils.js';
import { goto } from '../../utils/routing.utils.js';
import styles from './case-detail.component.css';

/**
 * A Vorgang itself: its name, where it stands and its sums, the Vorgänge it follows or collects and
 * those following or collecting it, a new offer or invoice for it, a Sammelrechnung with other
 * Vorgänge of its customer, and the history of all its documents together.
 *
 * @fires case-changed - the Vorgang was renamed or removed, the list reloads
 */
@customElement('erng-case-detail')
export class CaseDetailComponent extends ErngElement.withStyles(styles) {
  @property({ type: Number, attribute: 'case-id' }) caseId!: number;

  /** Raised by the list when its documents changed, the Vorgang is read again */
  @property({ type: Number }) revision = 0;

  @state() private case: Case | undefined;
  /** Other Vorgänge of the customer, while the Sammelrechnung picks from them */
  @state() private others: Case[] | undefined;
  @state() private picked = new Set<number>();

  protected override willUpdate(changed: PropertyValues<this>) {
    if (changed.has('caseId') || changed.has('revision')) {
      void this.load();
    }
  }

  private async load() {
    try {
      this.case = await api.caseGet(this.caseId);
    } catch (error) {
      notifyError(error);
    }
  }

  private announce() {
    this.dispatchEvent(new CustomEvent('case-changed', { bubbles: true, composed: true }));
  }

  private async rename(title: string) {
    try {
      this.case = await api.caseSetTitle(this.caseId, title.trim() || null);
      this.announce();
    } catch (error) {
      notifyError(error);
    }
  }

  private async deleteCase() {
    try {
      await api.caseDelete(this.caseId);
      this.announce();
      goto('/cases/none');
    } catch (error) {
      notifyError(error);
    }
  }

  private create(kind: Kind) {
    goto(casesRoute(kind, 'new', this.caseId));
  }

  /** The Vorgänge of the customer a Sammelrechnung can take besides this one, not archived ones */
  private async openCollect() {
    const vorgang = this.case;
    if (!vorgang) {
      return;
    }
    try {
      const all = await api.casesList(vorgang.context_id);
      this.others = all.filter(
        c => c.id !== vorgang.id && c.customer_id !== null && c.customer_id === vorgang.customer_id
      );
      this.picked = new Set();
    } catch (error) {
      notifyError(error);
    }
  }

  private async collect() {
    try {
      const invoice = await api.casesCollect([this.caseId, ...this.picked]);
      this.others = undefined;
      this.announce();
      goto(casesRoute('invoice', invoice.id));
    } catch (error) {
      notifyError(error);
    }
  }

  /** Vorgänge it refers to or that refer to it, each opens by its number */
  private renderRefs(label: string, refs: CaseRef[]) {
    return refs.length === 0
      ? nothing
      : html`
          <span class="muted">${label}</span>
          <span>
            ${refs.map(
              r => html`
                <wa-tag
                  size="s"
                  appearance="outlined"
                  class="ref"
                  @click="${() => goto(caseRoute(r.id))}"
                >
                  ${r.number}
                </wa-tag>
              `
            )}
          </span>
        `;
  }

  private renderCollect() {
    const others = this.others ?? [];
    return html`
      <wa-dialog
        label="Sammelrechnung"
        ?open="${this.others !== undefined}"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && (this.others = undefined)}"
      >
        <div class="stack">
          <p class="muted">
            Ein neuer Vorgang fasst diesen und die gewählten zusammen, mit einer Rechnung über
            alles, was in ihren Angeboten noch offen ist.
          </p>
          ${
            others.length === 0
              ? html`<p>Der Kunde hat keinen weiteren Vorgang.</p>`
              : others.map(
                  c => html`
                    <wa-checkbox
                      ?checked="${this.picked.has(c.id)}"
                      @change="${(e: Event) => {
                        const picked = new Set(this.picked);
                        if ((e.target as HTMLInputElement).checked) {
                          picked.add(c.id);
                        } else {
                          picked.delete(c.id);
                        }
                        this.picked = picked;
                      }}"
                    >
                      ${c.number} · ${caseName(c)}
                      <span class="muted">${CASE_STAGES[c.stage]}</span>
                    </wa-checkbox>
                  `
                )
          }
        </div>
        <div slot="footer" class="dialog-footer">
          <wa-button appearance="outlined" @click="${() => (this.others = undefined)}"
            >Abbrechen</wa-button
          >
          <wa-button variant="brand" ?disabled="${this.picked.size === 0}" @click="${this.collect}">
            Sammelrechnung anlegen
          </wa-button>
        </div>
      </wa-dialog>
    `;
  }

  override render() {
    const vorgang = this.case;
    if (!vorgang) {
      return nothing;
    }
    const sums: [string, string][] = [
      ['Angeboten', vorgang.offered],
      ['Angenommen', vorgang.accepted],
      ['Berechnet', vorgang.invoiced],
      ['Bezahlt', vorgang.paid],
      ['Offen', vorgang.open],
    ];
    return html`
      <erng-page heading="Vorgang ${vorgang.number}">
        <wa-card>
          <div class="stack">
            <div class="grid">
              <wa-input
                label="Bezeichnung"
                placeholder="z. B. Badsanierung"
                hint="Interne Bezeichnung"
                .value="${vorgang.title ?? ''}"
                @change="${(e: Event) => void this.rename(text(e))}"
              ></wa-input>
              <div class="facts">
                <span class="muted">Kunde</span>
                <span>${vorgang.customer_name || '–'}</span>
                <span class="muted">Angelegt</span>
                <span>${formatDate(vorgang.created_at)}</span>
                <span class="muted">Stand</span>
                <span>
                  <wa-tag size="s" variant="${STAGE_VARIANTS[vorgang.stage]}">
                    ${CASE_STAGES[vorgang.stage]}
                  </wa-tag>
                </span>
                ${this.renderRefs(
                  'Folgt auf',
                  vorgang.sources.filter(r => r.link === 'follow')
                )}
                ${this.renderRefs(
                  'Fasst zusammen',
                  vorgang.sources.filter(r => r.link === 'collect')
                )}
                ${this.renderRefs(
                  'Folgevorgänge',
                  vorgang.followers.filter(r => r.link === 'follow')
                )}
                ${this.renderRefs(
                  'Gesammelt in',
                  vorgang.followers.filter(r => r.link === 'collect')
                )}
              </div>
            </div>
            <dl class="sums">
              ${sums.map(
                ([label, value]) => html`
                  <div>
                    <dt class="muted">${label}</dt>
                    <dd>${money(value)}</dd>
                  </div>
                `
              )}
            </dl>
          </div>
        </wa-card>

        <wa-card>
          <div slot="header">Verlauf</div>
          <erng-history case-id="${vorgang.id}" revision="${this.revision}"></erng-history>
        </wa-card>

        ${
          vorgang.documents === 0
            ? html`
                <wa-button
                  slot="actions"
                  variant="danger"
                  appearance="outlined"
                  @click="${this.deleteCase}"
                >
                  Vorgang löschen
                </wa-button>
              `
            : nothing
        }
        ${
          vorgang.customer_id !== null && vorgang.documents > 0
            ? html`
                <wa-button slot="actions" appearance="outlined" @click="${this.openCollect}">
                  Sammelrechnung…
                </wa-button>
              `
            : nothing
        }
        <wa-button slot="primary" appearance="outlined" @click="${() => this.create('offer')}">
          ${KINDS.offer.newLabel}
        </wa-button>
        <wa-button slot="primary" variant="brand" @click="${() => this.create('invoice')}">
          ${KINDS.invoice.newLabel}
        </wa-button>
      </erng-page>
      ${this.renderCollect()}
    `;
  }
}

declare global {
  interface HTMLElementEventMap {
    'case-changed': CustomEvent<void>;
  }

  interface HTMLElementTagNameMap {
    'erng-case-detail': CaseDetailComponent;
  }
}
