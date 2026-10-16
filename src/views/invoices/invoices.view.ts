import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';

import type { CSSResultGroup } from 'lit';
import { html, nothing } from 'lit';
import { customElement, state } from 'lit/decorators.js';

import type { ListEntry, Settings } from '../../common/utils/api.utils.js';
import { api, errorText } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import { date, money } from '../../common/utils/format.utils.js';
import { goto } from '../../common/utils/routing.utils.js';
import styles from './invoices.view.css';

@customElement('erng-invoices')
export class InvoicesView extends ErngElement {
  static override styles: CSSResultGroup = [ErngElement.styles, styles];

  @state() private entries: ListEntry[] | undefined;
  @state() private settings: Settings | undefined;
  @state() private error = '';

  override connectedCallback() {
    super.connectedCallback();
    Promise.all([api.invoicesList(), api.settingsGet()]).then(
      ([entries, settings]) => {
        this.entries = entries;
        this.settings = settings;
      },
      (error: unknown) => (this.error = errorText(error))
    );
  }

  private async duplicate(event: Event, id: number) {
    event.stopPropagation();
    try {
      goto(`/invoices/${(await api.invoiceDuplicate(id)).id}`);
    } catch (error) {
      this.error = errorText(error);
    }
  }

  /** Without a seller and a bank account no invoice can be valid */
  private get incomplete() {
    const seller = this.settings?.seller;
    return seller !== undefined && (seller.name.trim() === '' || seller.iban.trim() === '');
  }

  override render() {
    return html`
      <div class="stack">
        <div class="row between">
          <h2>Rechnungen</h2>
          <wa-button
            variant="brand"
            ?disabled="${this.incomplete}"
            @click="${() => goto('/invoices/new')}"
          >
            Neue Rechnung
          </wa-button>
        </div>

        ${this.error ? html`<wa-callout variant="danger">${this.error}</wa-callout>` : nothing}
        ${
          this.incomplete
            ? html`<wa-callout variant="warning">
                Bitte zuerst
                <a href="/settings">Unternehmen und Bankverbindung in den Einstellungen</a>
                eintragen.
              </wa-callout>`
            : nothing
        }
        ${
          this.entries?.length === 0
            ? html`<wa-callout variant="neutral">Noch keine Rechnungen.</wa-callout>`
            : this.entries
              ? html`
                  <table>
                    <thead>
                      <tr>
                        <th>Nummer</th>
                        <th>Kunde</th>
                        <th>Datum</th>
                        <th class="amount">Betrag</th>
                        <th></th>
                      </tr>
                    </thead>
                    <tbody>
                      ${this.entries.map(
                        e => html`
                          <tr @click="${() => goto(`/invoices/${e.id}`)}">
                            <td>
                              ${e.number ?? html`<wa-tag variant="neutral" size="s">Entwurf</wa-tag>`}
                            </td>
                            <td>${e.customer_name || html`<span class="muted">–</span>`}</td>
                            <td>${date(e.issue_date)}</td>
                            <td class="amount">${money(e.total)}</td>
                            <td class="amount">
                              <wa-button
                                size="s"
                                appearance="plain"
                                @click="${(ev: Event) => this.duplicate(ev, e.id)}"
                              >
                                Duplizieren
                              </wa-button>
                            </td>
                          </tr>
                        `
                      )}
                    </tbody>
                  </table>
                `
              : nothing
        }
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-invoices': InvoicesView;
  }
}
