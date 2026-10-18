import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/button-group/button-group.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/card/card.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '../../common/components/customer-form/customer-form.component.js';
import '../../common/components/filter-bar/filter-bar.component.js';
import '../../common/layouts/cards/cards.layout.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import { html, nothing } from 'lit';
import { customElement, state } from 'lit/decorators.js';

import type { Customer } from '../../common/utils/api.utils.js';
import { api, errorText } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';

const blank = (): Customer => ({
  id: null,
  name: '',
  address: { street: '', post_code: '', city: '', country: 'DE' },
  vat_id: null,
  email: null,
  buyer_reference: null,
});

const GROUPS = [
  { key: 'all', label: 'Alle' },
  { key: 'public', label: 'Öffentlich' },
  { key: 'private', label: 'Privat' },
];

@customElement('erng-customers')
export class CustomersView extends ErngElement {
  @state() private customers: Customer[] = [];
  @state() private editing: Customer | undefined;
  @state() private error = '';
  @state() private query = '';
  @state() private groups = 'all';

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

  private get shown(): Customer[] {
    const query = this.query.trim().toLowerCase();
    return this.customers.filter(
      c =>
        (this.groups === 'all' ||
          (this.groups === 'public' && c.buyer_reference !== null) ||
          (this.groups === 'private' && c.buyer_reference === null)) &&
        [c.name, c.address.city, c.address.post_code, c.email ?? '', c.vat_id ?? '']
          .join(' ')
          .toLowerCase()
          .includes(query)
    );
  }

  override render() {
    const editing = this.editing;
    const shown = this.shown;
    return html`
      <erng-cards>
        <erng-filter-bar
          slot="header"
          .query="${this.query}"
          placeholder="Kunden suchen"
          @query-change="${(e: CustomEvent<string>) => (this.query = e.detail)}"
        >
          <wa-select
            slot="filters"
            aria-label="Gruppen"
            .value="${this.groups}"
            @change="${(e: Event) => (this.groups = String((e.target as WaSelect).value))}"
          >
            ${GROUPS.map(u => html`<wa-option value="${u.key}">${u.label}</wa-option>`)}
          </wa-select>
          <wa-button slot="actions" variant="brand" @click="${() => this.edit(blank())}">
            Neuer Kunde
          </wa-button>
        </erng-filter-bar>

        ${this.error && !editing ? html`<wa-callout slot="notice" variant="danger">${this.error}</wa-callout>` : nothing}
        ${
          shown.length === 0
            ? html`
                <wa-callout slot="notice" variant="neutral">
                  ${this.customers.length === 0 ? 'Noch keine Kunden angelegt.' : 'Keine Treffer.'}
                </wa-callout>
              `
            : html`
                ${shown.map(
                  c => html`
                    <wa-card slot="cards" @click="${() => this.edit(c)}">
                      <strong>${c.name}</strong>
                      <div class="muted">
                        ${c.address.street}<br>${c.address.post_code} ${c.address.city}
                      </div>
                    </wa-card>
                  `
                )}
              `
        }

        <wa-dialog
          slot="dialog"
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
              ? html`
                  <wa-button
                    slot="footer"
                    variant="danger"
                    appearance="outlined"
                    @click="${this.deleteCustomer}"
                  >
                    Löschen
                  </wa-button>
                `
              : nothing
          }
          <wa-button-group slot="footer" label="Bearbeiten">
            <wa-button appearance="outlined" @click="${() => (this.editing = undefined)}">
              Abbrechen
            </wa-button>
            <wa-button variant="brand" @click="${this.save}">Speichern</wa-button>
          </wa-button-group>
        </wa-dialog>
      </erng-cards>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-customers': CustomersView;
  }
}
