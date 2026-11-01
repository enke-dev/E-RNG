import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import type { PropertyValues } from 'lit';
import { html } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { OfferBilling, OnAccount, Position } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { sharePositions, typedMoney } from '../../utils/billing.utils.js';
import { text } from '../../utils/form.utils.js';
import styles from './offer-share.component.css';

/**
 * A share of an offer of the Vorgang for an invoice on account: the offer and a percentage give
 * one lump sum per VAT rate of the offer (`Abschlag 30 % auf Angebot …`), shown before it is taken.
 *
 * @fires pick - the positions it makes are the `detail`
 * @fires close - the dialog closed
 */
@customElement('erng-offer-share')
export class OfferShareComponent extends ErngElement.withStyles(styles) {
  @property({ attribute: false }) billings: OfferBilling[] = [];
  @property({ attribute: false }) onAccount: OnAccount = 'progress';
  @property({ type: Boolean, reflect: true }) open = false;

  @state() private offer: number | undefined;
  @state() private percent = '30';

  protected override willUpdate(changed: PropertyValues<this>) {
    if (changed.has('open') && this.open) {
      this.offer = this.billings[0]?.offer_id;
    }
  }

  private get positions(): Position[] {
    const billing = this.billings.find(b => b.offer_id === this.offer);
    const percent = Number(this.percent.trim().replace(',', '.'));
    return billing && percent > 0 && percent <= 100
      ? sharePositions(billing, percent, this.onAccount)
      : [];
  }

  private take() {
    this.dispatchEvent(new CustomEvent<Position[]>('pick', { detail: this.positions }));
    this.open = false;
  }

  override render() {
    const positions = this.positions;
    return html`
      <wa-dialog
        label="Anteil eines Angebots"
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
            <wa-select
              label="Angebot"
              size="s"
              .value="${this.offer === undefined ? '' : String(this.offer)}"
              @change="${(e: Event) => (this.offer = Number((e.target as WaSelect).value))}"
            >
              ${this.billings.map(
                b => html`<wa-option value="${String(b.offer_id)}">${b.number}</wa-option>`
              )}
            </wa-select>
            <wa-input
              label="Anteil (%)"
              size="s"
              inputmode="decimal"
              .value="${this.percent}"
              @input="${(e: Event) => (this.percent = text(e))}"
            ></wa-input>
          </div>
          ${
            positions.length === 0
              ? html`<p class="muted">Ein Anteil zwischen 0 und 100 Prozent.</p>`
              : html`
                  <ul>
                    ${positions.map(
                      p => html`
                        <li>
                          <span>${p.description}</span>
                          <span class="amount">${typedMoney(p.price)}</span>
                        </li>
                      `
                    )}
                  </ul>
                  <p class="muted">Netto, je Steuersatz des Angebots eine Position.</p>
                `
          }
        </div>
        <div slot="footer" class="dialog-footer">
          <wa-button appearance="outlined" @click="${() => (this.open = false)}"
            >Abbrechen</wa-button
          >
          <wa-button variant="brand" ?disabled="${positions.length === 0}" @click="${this.take}">
            Übernehmen
          </wa-button>
        </div>
      </wa-dialog>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-offer-share': OfferShareComponent;
  }
}
