import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/card/card.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';
import '../history/history.component.js';
import '../page/page.component.js';

import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { Case, Kind } from '../../utils/api.utils.js';
import { api } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { CASE_STAGES, casesRoute, STAGE_VARIANTS } from '../../utils/cases.utils.js';
import { text } from '../../utils/form.utils.js';
import { formatDate, money } from '../../utils/format.utils.js';
import { KINDS } from '../../utils/kind.utils.js';
import { notifyError } from '../../utils/notify.utils.js';
import { goto } from '../../utils/routing.utils.js';
import styles from './case-detail.component.css';

/**
 * A Vorgang itself: its name, where it stands and its sums, a new offer or invoice for it, and the
 * history of all its documents together.
 *
 * @fires case-changed - the Vorgang was renamed or removed, the list reloads
 */
@customElement('erng-case-detail')
export class CaseDetailComponent extends ErngElement.withStyles(styles) {
  @property({ type: Number, attribute: 'case-id' }) caseId!: number;

  /** Raised by the list when its documents changed, the Vorgang is read again */
  @property({ type: Number }) revision = 0;

  @state() private case: Case | undefined;

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
        <wa-button slot="primary" appearance="outlined" @click="${() => this.create('offer')}">
          ${KINDS.offer.newLabel}
        </wa-button>
        <wa-button slot="primary" variant="brand" @click="${() => this.create('invoice')}">
          ${KINDS.invoice.newLabel}
        </wa-button>
      </erng-page>
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
