import '@awesome.me/webawesome/dist/components/input/input.js';
import '../address-fields/address-fields.component.js';

import { html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import type { Customer } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { nullable, text } from '../../utils/form.utils.js';

/** The fields of a customer, edits the given object and fires `change` */
@customElement('erng-customer-form')
export class CustomerFormComponent extends ErngElement {
  @property({ attribute: false })
  customer!: Customer;

  private changed() {
    this.dispatchEvent(new CustomEvent('change', { bubbles: true, composed: true }));
  }

  override render() {
    const c = this.customer;
    return html`
      <div class="stack">
        <wa-input
          label="Name"
          required
          .value="${c.name}"
          @input="${(e: Event) => {
            c.name = text(e);
            this.changed();
          }}"
        ></wa-input>
        <erng-address-fields
          .address="${c.address}"
          @change="${() => this.changed()}"
        ></erng-address-fields>
        <div class="grid">
          <wa-input
            label="USt-IdNr."
            .value="${c.vat_id ?? ''}"
            @input="${(e: Event) => {
              c.vat_id = nullable(text(e));
              this.changed();
            }}"
          ></wa-input>
          <wa-input
            label="E-Mail für Rechnungen"
            type="email"
            .value="${c.email ?? ''}"
            @input="${(e: Event) => {
              c.email = nullable(text(e));
              this.changed();
            }}"
          ></wa-input>
          <wa-input
            label="Leitweg-ID (öffentliche Auftraggeber)"
            .value="${c.buyer_reference ?? ''}"
            @input="${(e: Event) => {
              c.buyer_reference = nullable(text(e));
              this.changed();
            }}"
          ></wa-input>
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-customer-form': CustomerFormComponent;
  }
}
