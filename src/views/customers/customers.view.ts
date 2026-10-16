import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/card/card.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '../../common/components/customer-form/customer-form.component.js';

import type { CSSResultGroup } from 'lit';
import { html, nothing } from 'lit';
import { customElement, state } from 'lit/decorators.js';

import type { Customer } from '../../common/utils/api.utils.js';
import { api, errorText } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import styles from './customers.view.css';

const blank = (): Customer => ({
  id: null,
  name: '',
  address: { street: '', post_code: '', city: '', country: 'DE' },
  vat_id: null,
  email: null,
  buyer_reference: null,
});

@customElement('erng-customers')
export class CustomersView extends ErngElement {
  static override styles: CSSResultGroup = [ErngElement.styles, styles];

  @state() private customers: Customer[] = [];
  @state() private editing: Customer | undefined;
  @state() private error = '';

  override connectedCallback() {
    super.connectedCallback();
    void this.load();
  }

  private async load() {
    try {
      this.customers = await api.customersList();
    } catch (error) {
      this.error = errorText(error);
    }
  }

  private edit(customer: Customer) {
    this.error = '';
    this.editing = structuredClone(customer);
  }

  private async save() {
    const customer = this.editing;
    if (!customer) {
      return;
    }
    if (customer.name.trim() === '') {
      this.error = 'Bitte einen Namen eingeben.';
      return;
    }
    try {
      await api.customerSave(customer);
      this.editing = undefined;
      await this.load();
    } catch (error) {
      this.error = errorText(error);
    }
  }

  private async deleteCustomer() {
    const id = this.editing?.id;
    if (id == null) {
      return;
    }
    try {
      await api.customerDelete(id);
      this.editing = undefined;
      await this.load();
    } catch (error) {
      this.error = errorText(error);
    }
  }

  override render() {
    const editing = this.editing;
    return html`
      <div class="stack">
        <div class="row between">
          <h2>Kunden</h2>
          <wa-button variant="brand" @click="${() => this.edit(blank())}">Neuer Kunde</wa-button>
        </div>
        ${this.error && !editing ? html`<wa-callout variant="danger">${this.error}</wa-callout>` : nothing}
        ${
          this.customers.length === 0
            ? html`<wa-callout variant="neutral">Noch keine Kunden angelegt.</wa-callout>`
            : html`
                <div class="customers">
                  ${this.customers.map(
                    c => html`
                      <wa-card @click="${() => this.edit(c)}">
                        <strong>${c.name}</strong>
                        <div class="muted">
                          ${c.address.street}<br>${c.address.post_code} ${c.address.city}
                        </div>
                      </wa-card>
                    `
                  )}
                </div>
              `
        }
      </div>

      <wa-dialog
        ?open="${editing !== undefined}"
        label="${editing?.id == null ? 'Neuer Kunde' : 'Kunde bearbeiten'}"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && (this.editing = undefined)}"
      >
        ${
          editing
            ? html`
                <div class="stack">
                  <erng-customer-form
                    .customer="${editing}"
                    @change="${() => this.requestUpdate()}"
                  ></erng-customer-form>
                  ${this.error ? html`<wa-callout variant="danger">${this.error}</wa-callout>` : nothing}
                </div>
              `
            : nothing
        }
        ${
          editing?.id != null
            ? html`<wa-button
                slot="footer"
                variant="danger"
                appearance="plain"
                @click="${this.deleteCustomer}"
                >Löschen</wa-button
              >`
            : nothing
        }
        <wa-button slot="footer" appearance="plain" @click="${() => (this.editing = undefined)}"
          >Abbrechen</wa-button
        >
        <wa-button slot="footer" variant="brand" @click="${this.save}">Speichern</wa-button>
      </wa-dialog>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-customers': CustomersView;
  }
}
