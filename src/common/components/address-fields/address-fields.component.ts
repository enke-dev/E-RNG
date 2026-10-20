import '@awesome.me/webawesome/dist/components/input/input.js';

import { html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import type { Address } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { text } from '../../utils/form.utils.js';

/** Street, post code, city and country of an address, edits the given object and fires `change` */
@customElement('erng-address-fields')
export class AddressFieldsComponent extends ErngElement {
  @property({ attribute: false })
  address!: Address;

  @property({ type: Boolean })
  disabled = false;

  /** Marks every field as required */
  @property({ type: Boolean })
  required = false;

  private set(key: keyof Address, value: string) {
    this.address[key] = value;
    this.dispatchEvent(new CustomEvent('change', { bubbles: true, composed: true }));
  }

  override render() {
    const a = this.address;
    return html`
      <div class="stack">
        <wa-input
          label="Straße und Hausnummer"
          ?required="${this.required}"
          ?disabled="${this.disabled}"
          .value="${a.street}"
          @input="${(e: Event) => this.set('street', text(e))}"
        ></wa-input>
        <div class="grid">
          <wa-input
            label="PLZ"
            ?required="${this.required}"
            ?disabled="${this.disabled}"
            .value="${a.post_code}"
            @input="${(e: Event) => this.set('post_code', text(e))}"
          ></wa-input>
          <wa-input
            label="Ort"
            ?required="${this.required}"
            ?disabled="${this.disabled}"
            .value="${a.city}"
            @input="${(e: Event) => this.set('city', text(e))}"
          ></wa-input>
          <wa-input
            label="Land (Code)"
            maxlength="2"
            ?required="${this.required}"
            ?disabled="${this.disabled}"
            .value="${a.country}"
            @input="${(e: Event) => this.set('country', text(e).toUpperCase())}"
          ></wa-input>
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-address-fields': AddressFieldsComponent;
  }
}
