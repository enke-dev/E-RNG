import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/input/input.js';

import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { OfferBilling, Position } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { amount, billedPosition } from '../../utils/billing.utils.js';
import { text } from '../../utils/form.utils.js';
import { unitLabel } from '../../utils/units.utils.js';
import styles from './offer-pick.component.css';

/** A picked position: its offer and place, how many of it */
interface Pick {
  offer: number;
  index: number;
  quantity: string;
}

const key = (offer: number, index: number) => `${offer}:${index}`;

/**
 * Positions of the offers of a Vorgang to take into an invoice, with what is open of each: the
 * open ones are picked with all that is open, which can be lowered (a Teilrechnung). Positions the
 * invoice bills already are not offered again.
 *
 * @fires pick - the picked positions as invoice positions referring to theirs are the `detail`
 * @fires close - the dialog closed
 */
@customElement('erng-offer-pick')
export class OfferPickComponent extends ErngElement.withStyles(styles) {
  @property({ attribute: false }) billings: OfferBilling[] = [];

  /** What the invoice bills already: its positions */
  @property({ attribute: false }) positions: Position[] = [];

  @property({ type: Boolean, reflect: true }) open = false;

  @state() private picked = new Map<string, Pick>();

  protected override willUpdate(changed: PropertyValues<this>) {
    if (changed.has('open') && this.open) {
      // everything that is open and not on the invoice yet, with all that is open
      this.picked = new Map(
        this.billings.flatMap(b =>
          b.positions
            .filter(p => Number(p.open) > 0 && !this.taken(b.offer_id, p.index))
            .map(p => [
              key(b.offer_id, p.index),
              { offer: b.offer_id, index: p.index, quantity: p.open },
            ])
        )
      );
    }
  }

  private taken(offer: number, index: number) {
    return this.positions.some(p => p.source?.offer_id === offer && p.source.index === index);
  }

  private toggle(offer: number, index: number, open: string, on: boolean) {
    const picked = new Map(this.picked);
    if (on) {
      picked.set(key(offer, index), { offer, index, quantity: open });
    } else {
      picked.delete(key(offer, index));
    }
    this.picked = picked;
  }

  private setQuantity(offer: number, index: number, typed: string) {
    const current = this.picked.get(key(offer, index));
    if (!current) {
      return;
    }
    this.picked = new Map(this.picked).set(key(offer, index), {
      ...current,
      quantity: typed.trim().replace(',', '.'),
    });
  }

  private take() {
    const positions = [...this.picked.values()].flatMap(({ offer, index, quantity }) => {
      const billing = this.billings.find(b => b.offer_id === offer);
      return billing && Number(quantity) > 0 ? [billedPosition(billing, index, quantity)] : [];
    });
    this.dispatchEvent(new CustomEvent<Position[]>('pick', { detail: positions }));
    this.open = false;
  }

  private renderOffer(billing: OfferBilling) {
    return html`
      <section>
        <h3>${billing.number}</h3>
        ${billing.positions.map(p => {
          const done = Number(p.open) <= 0;
          const taken = this.taken(billing.offer_id, p.index);
          const picked = this.picked.get(key(billing.offer_id, p.index));
          return html`
            <div class="position">
              <wa-checkbox
                size="s"
                ?disabled="${done || taken}"
                ?checked="${picked !== undefined}"
                @change="${(e: Event) =>
                  this.toggle(
                    billing.offer_id,
                    p.index,
                    p.open,
                    (e.target as HTMLInputElement).checked
                  )}"
              >
                ${p.index + 1}. ${p.position.description || '–'}
                <span class="muted">
                  ${p.position.price} € je ${unitLabel(p.position)} ·
                  ${
                    taken
                      ? 'schon auf dieser Rechnung'
                      : done
                        ? 'vollständig berechnet'
                        : `offen ${amount(p.open)} von ${amount(p.position.quantity ?? '1')}`
                  }
                </span>
              </wa-checkbox>
              ${
                picked
                  ? html`
                      <wa-input
                        size="s"
                        inputmode="decimal"
                        label="Menge"
                        .value="${amount(picked.quantity)}"
                        @input="${(e: Event) => this.setQuantity(billing.offer_id, p.index, text(e))}"
                      ></wa-input>
                    `
                  : nothing
              }
            </div>
          `;
        })}
      </section>
    `;
  }

  override render() {
    return html`
      <wa-dialog
        label="Aus Angebot übernehmen"
        ?open="${this.open}"
        @wa-after-hide="${(e: Event) => {
          if (e.target === e.currentTarget) {
            this.open = false;
            this.dispatchEvent(new CustomEvent('close'));
          }
        }}"
      >
        <div class="stack">
          ${
            this.billings.length === 0
              ? html`<p class="muted">
                  Dieser Vorgang hat kein Angebot, das noch berechnet werden kann.
                </p>`
              : this.billings.map(b => this.renderOffer(b))
          }
        </div>
        <div slot="footer" class="dialog-footer">
          <wa-button appearance="outlined" @click="${() => (this.open = false)}"
            >Abbrechen</wa-button
          >
          <wa-button variant="brand" ?disabled="${this.picked.size === 0}" @click="${this.take}">
            Übernehmen
          </wa-button>
        </div>
      </wa-dialog>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-offer-pick': OfferPickComponent;
  }
}
