import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import { html } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { Case, Customer } from '../../utils/api.utils.js';
import { api } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { text } from '../../utils/form.utils.js';
import { notifyError } from '../../utils/notify.utils.js';

/**
 * Starts a Vorgang: for which customer, and what it is about. It has no documents yet.
 *
 * @fires case-created - the new Vorgang is the `detail`
 * @fires closed - the dialog is gone, with or without a new Vorgang
 */
@customElement('erng-new-case')
export class NewCaseComponent extends ErngElement {
  @property({ type: Number, attribute: 'context-id' }) contextId!: number;

  @state() private open = true;
  @state() private customers: Customer[] = [];
  @state() private customer = '';
  @state() private name = '';
  @state() private busy = false;

  override connectedCallback() {
    super.connectedCallback();
    void api
      .customersList(this.contextId)
      .then(customers => (this.customers = customers))
      .catch(notifyError);
  }

  private async create() {
    if (this.customer === '' || this.name.trim() === '') {
      return;
    }
    this.busy = true;
    try {
      const created = await api.caseCreate(this.contextId, Number(this.customer), this.name.trim());
      this.dispatchEvent(new CustomEvent<Case>('case-created', { detail: created }));
      this.open = false;
    } catch (error) {
      notifyError(error);
    } finally {
      this.busy = false;
    }
  }

  override render() {
    return html`
      <wa-dialog
        label="Neuer Vorgang"
        ?open="${this.open}"
        @wa-after-hide="${(e: Event) =>
          e.target === e.currentTarget && this.dispatchEvent(new CustomEvent('closed'))}"
      >
        <div class="stack">
          <wa-select
            label="Kunde"
            placeholder="Kunde auswählen"
            required
            hint="Alle Dokumente des Vorgangs gehen an ihn"
            .value="${this.customer}"
            @change="${(e: Event) => (this.customer = String((e.target as WaSelect).value))}"
          >
            ${this.customers.map(c => html`<wa-option value="${String(c.id)}">${c.name}</wa-option>`)}
          </wa-select>
          <wa-input
            label="Bezeichnung"
            placeholder="z. B. Badsanierung"
            hint="Interne Bezeichnung, nennt den Vorgang in der Liste"
            required
            .value="${this.name}"
            @input="${(e: Event) => (this.name = text(e))}"
          ></wa-input>
        </div>
        <div slot="footer" class="dialog-footer">
          <wa-button appearance="outlined" @click="${() => (this.open = false)}"
            >Abbrechen</wa-button
          >
          <div class="end">
            <wa-button
              variant="brand"
              ?loading="${this.busy}"
              ?disabled="${this.customer === '' || this.name.trim() === ''}"
              @click="${this.create}"
            >
              Anlegen
            </wa-button>
          </div>
        </div>
      </wa-dialog>
    `;
  }
}

declare global {
  interface HTMLElementEventMap {
    'case-created': CustomEvent<Case>;
  }

  interface HTMLElementTagNameMap {
    'erng-new-case': NewCaseComponent;
  }
}
