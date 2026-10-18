import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';
import '../../common/components/filter-bar/filter-bar.component.js';
import '../invoice-edit/invoice-edit.view.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import type { CSSResultGroup, PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { ListEntry, Settings } from '../../common/utils/api.utils.js';
import { api, errorText } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import { formatDate, money } from '../../common/utils/format.utils.js';
import { goto, redirect } from '../../common/utils/routing.utils.js';
import styles from './invoices.view.css';

type StatusFilter = 'all' | 'draft' | 'exported';

/** The text a search looks in: number, customer, date as shown, amount as shown */
function haystack(entry: ListEntry): string {
  return [
    entry.number ?? 'entwurf',
    entry.customer_name,
    formatDate(entry.issue_date),
    money(entry.total),
  ]
    .join(' ')
    .toLowerCase();
}

/**
 * Invoices: a small filterable list on the left, the selected invoice on the right.
 * Without a selection in the address the newest invoice is shown.
 */
@customElement('erng-invoices')
export class InvoicesView extends ErngElement {
  static override styles: CSSResultGroup = [ErngElement.styles, styles];

  /** From the address: `new`, an id, or empty for "the newest" */
  @property() selected = '';

  @state() private entries: ListEntry[] | undefined;
  @state() private settings: Settings | undefined;
  @state() private error = '';
  @state() private query = '';
  @state() private status: StatusFilter = 'all';
  /** An invoice that got its id while the address still says `new` */
  @state() private created: number | undefined;

  override connectedCallback() {
    super.connectedCallback();
    void this.load();
  }

  protected override willUpdate(changed: PropertyValues<this>) {
    if (changed.has('selected')) {
      this.created = undefined;
    }
  }

  protected override updated(changed: PropertyValues<this>) {
    if (changed.has('selected')) {
      this.selectNewest();
    }
  }

  /** The bare address shows the newest invoice: make it the address, so a reload keeps the selection */
  private selectNewest() {
    const newest = this.entries?.[0];
    if (this.selected === '' && newest) {
      redirect(`/invoices/${newest.id}`);
    }
  }

  private async load() {
    try {
      const [entries, settings] = await Promise.all([api.invoicesList(), api.settingsGet()]);
      this.entries = entries;
      this.settings = settings;
      this.error = '';
      this.selectNewest();
    } catch (error) {
      this.error = errorText(error);
    }
  }

  private async handleChanged(event: CustomEvent<{ id: number | null; deleted: boolean }>) {
    // the deleted invoice leaves the list at once: what shows next is picked from what is left
    if (event.detail.deleted && this.entries) {
      this.entries = this.entries.filter(e => e.id !== event.detail.id);
    }
    if (!event.detail.deleted && event.detail.id !== null && this.selected === 'new') {
      this.created = event.detail.id;
    }
    await this.load();
  }

  /** What the detail shows: the address wins, then the newest invoice, then a blank one */
  private get current(): string {
    if (this.selected !== '') {
      return this.selected;
    }
    const newest = this.entries?.[0];
    return newest ? String(newest.id) : 'new';
  }

  private get highlighted(): string {
    return this.created === undefined ? this.current : String(this.created);
  }

  /** A new invoice exists only once it is saved as a draft, until then there is no second new one */
  private get unsaved() {
    return this.current === 'new' && this.created === undefined;
  }

  /** Without a seller and a bank account no invoice can be valid */
  private get incomplete() {
    const seller = this.settings?.seller;
    return seller !== undefined && (seller.name.trim() === '' || seller.iban.trim() === '');
  }

  private get shown(): ListEntry[] {
    const query = this.query.trim().toLowerCase();
    return (this.entries ?? []).filter(
      e => (this.status === 'all' || e.status === this.status) && haystack(e).includes(query)
    );
  }

  override render() {
    const shown = this.shown;
    return html`
      <aside>
        <erng-filter-bar
          .query="${this.query}"
          placeholder="Rechnungen suchen …"
          @query-change="${(e: CustomEvent<string>) => (this.query = e.detail)}"
        >
          <wa-select
            slot="filters"
            aria-label="Status"
            .value="${this.status}"
            @change="${(e: Event) => (this.status = String((e.target as WaSelect).value) as StatusFilter)}"
          >
            <wa-option value="all">Alle</wa-option>
            <wa-option value="draft">Entwürfe</wa-option>
            <wa-option value="exported">Exportiert</wa-option>
          </wa-select>
          <wa-button
            slot="actions"
            variant="brand"
            ?disabled="${this.incomplete || this.unsaved}"
            @click="${() => goto('/invoices/new')}"
          >
            Neue Rechnung
          </wa-button>
        </erng-filter-bar>

        ${this.error ? html`<wa-callout variant="danger">${this.error}</wa-callout>` : nothing}
        ${
          this.incomplete
            ? html`
                <wa-callout variant="warning">
                  Bitte zuerst <a href="/settings">Unternehmen und Bankverbindung</a> in den
                  Einstellungen eintragen.
                </wa-callout>
              `
            : nothing
        }
        ${
          this.entries && shown.length === 0
            ? html`<wa-callout variant="neutral"
                >${this.entries.length === 0 ? 'Noch keine Rechnungen.' : 'Keine Treffer.'}</wa-callout
              >`
            : nothing
        }

        <ul>
          ${shown.map(
            e => html`
              <li>
                <button
                  class="item"
                  type="button"
                  aria-current="${String(String(e.id) === this.highlighted)}"
                  @click="${() => goto(`/invoices/${e.id}`)}"
                >
                  <span class="name">${e.customer_name || html`<span class="muted">–</span>`}</span>
                  <span class="amount">${money(e.total)}</span>
                  <span class="muted">
                    ${e.number ?? html`<wa-tag variant="neutral" size="s">Entwurf</wa-tag>`}
                  </span>
                  <span class="muted amount">${formatDate(e.issue_date)}</span>
                </button>
              </li>
            `
          )}
        </ul>
      </aside>

      <article class="detail" @invoice-changed="${this.handleChanged}">
        ${
          this.entries
            ? html`<erng-invoice-edit invoice-id="${this.current}"></erng-invoice-edit>`
            : nothing
        }
      </article>
    `;
  }
}

declare global {
  interface HTMLElementEventMap {
    'invoice-changed': CustomEvent<{ id: number | null; deleted: boolean }>;
  }

  interface HTMLElementTagNameMap {
    'erng-invoices': InvoicesView;
  }
}
