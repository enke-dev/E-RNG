import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/button-group/button-group.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/card/card.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';
import '../../common/components/customer-form/customer-form.component.js';
import '../../common/components/filter-bar/filter-bar.component.js';
import '../../common/components/scope-select/scope-select.component.js';
import '../../common/layouts/cards/cards.layout.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import { html, nothing } from 'lit';
import { customElement, state } from 'lit/decorators.js';

import type { Customer } from '../../common/utils/api.utils.js';
import { api } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import { activeContext, contextsOf, label } from '../../common/utils/context.utils.js';
import { notify, notifyError } from '../../common/utils/notify.utils.js';
import styles from './customers.view.css';

const blank = (): Customer => ({
  id: null,
  context_id: null,
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
export class CustomersView extends ErngElement.withStyles(styles) {
  @state() private customers: Customer[] = [];
  @state() private editing: Customer | undefined;
  @state() private query = '';
  @state() private groups = 'all';

  override connectedCallback() {
    super.connectedCallback();
    void this.load();
  }

  private async load() {
    try {
      const context = activeContext.get()?.id;
      if (context != null) {
        this.customers = await api.customersList(context);
      }
    } catch (error) {
      notifyError(error);
    }
  }

  private edit(customer: Customer) {
    this.editing = structuredClone(customer);
  }

  private async save() {
    const customer = this.editing;
    if (!customer) {
      return;
    }
    if (customer.name.trim() === '') {
      notify('Bitte einen Namen eingeben.', 'danger');
      return;
    }
    try {
      await api.customerSave(customer);
      this.editing = undefined;
      await this.load();
    } catch (error) {
      notifyError(error);
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
      notifyError(error);
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
                      ${
                        contextsOf(c.context_id).length > 0
                          ? html`
                              <div class="contexts">
                                ${contextsOf(c.context_id).map(
                                  context => html`
                                    <wa-tag size="s" variant="neutral">${label(context)}</wa-tag>
                                  `
                                )}
                              </div>
                            `
                          : nothing
                      }
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
                    <erng-scope-select
                      .value="${editing.context_id}"
                      @scope-change="${(e: CustomEvent<number | null>) => {
                        editing.context_id = e.detail;
                        this.requestUpdate();
                      }}"
                    ></erng-scope-select>
                  </div>
                `
              : nothing
          }
          <div slot="footer" class="dialog-footer">
            <wa-button appearance="outlined" @click="${() => (this.editing = undefined)}">
              Abbrechen
            </wa-button>
            <div class="end">
              ${
                editing?.id != null
                  ? html`<wa-button
                      variant="danger"
                      appearance="outlined"
                      @click="${this.deleteCustomer}"
                    >
                      Löschen
                    </wa-button>`
                  : nothing
              }
              <wa-button variant="brand" @click="${this.save}">Speichern</wa-button>
            </div>
          </div>
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
