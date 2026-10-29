import '@awesome.me/webawesome/dist/components/badge/badge.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/divider/divider.js';
import '@awesome.me/webawesome/dist/components/dropdown/dropdown.js';
import '@awesome.me/webawesome/dist/components/dropdown-item/dropdown-item.js';
import '@awesome.me/webawesome/dist/components/icon/icon.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';
import '../../common/components/filter-bar/filter-bar.component.js';
import '../../common/components/list/list/list.component.js';
import '../../common/components/list/list-item/list-item.component.js';
import '../../common/layouts/split/split.layout.js';
import '../invoice-edit/invoice-edit.view.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { Kind, ListEntry } from '../../common/utils/api.utils.js';
import { api } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import { activeContext, available, contextById, label } from '../../common/utils/context.utils.js';
import { today } from '../../common/utils/form.utils.js';
import { formatDate, money } from '../../common/utils/format.utils.js';
import { haystack, kindRoute, KINDS, otherKind } from '../../common/utils/kind.utils.js';
import { notifyError } from '../../common/utils/notify.utils.js';
import { goto, redirect } from '../../common/utils/routing.utils.js';
import styles from './invoices.view.css';

/**
 * Where a document stands: a draft, or created. An invoice is open until it is paid, an offer valid
 * until its day has passed.
 */
type Stage = 'draft' | 'open' | 'paid' | 'valid' | 'expired';

const STAGES: Record<Kind, [Stage, string][]> = {
  invoice: [
    ['draft', 'Entwürfe'],
    ['open', 'Offen'],
    ['paid', 'Bezahlt'],
  ],
  offer: [
    ['draft', 'Entwürfe'],
    ['valid', 'Gültig'],
    ['expired', 'Abgelaufen'],
  ],
};

/** A created offer whose day has passed; one without a validity never expires */
const expired = (entry: ListEntry) =>
  entry.status === 'exported' && entry.valid_until !== null && entry.valid_until < today();

const stageOf = (entry: ListEntry): Stage => {
  if (entry.status === 'draft') {
    return 'draft';
  }
  if (entry.kind === 'offer') {
    return expired(entry) ? 'expired' : 'valid';
  }
  return entry.paid_at ? 'paid' : 'open';
};

/** The context as its tag names it everywhere, else the seller the invoice was made with */
const contextName = (entry: ListEntry) => {
  const context = contextById(entry.context_id);
  return context ? label(context) : entry.seller_name || '–';
};

/** What a document is, as tags and check marks on a row of its own; the context only in a mixed list */
function tagsOf(entry: ListEntry, mixed: boolean) {
  // an offer is not checked against rules, it is only made
  const created = entry.kind === 'offer' ? 'Erstellt' : 'Erstellt und geprüft';
  return [
    mixed ? html`<wa-tag variant="neutral" size="s">${contextName(entry)}</wa-tag>` : null,
    entry.archived_at ? html`<wa-tag variant="neutral" size="s">Archiviert</wa-tag>` : null,
    expired(entry) ? html`<wa-tag variant="neutral" size="s">Abgelaufen</wa-tag>` : null,
    entry.status === 'exported'
      ? html`
          <wa-badge pill class="created" variant="brand" title="${created}">
            <wa-icon name="check" label="${created}"></wa-icon>
          </wa-badge>
        `
      : null,
    entry.paid_at
      ? html`
          <wa-badge
            pill
            class="payed"
            variant="success"
            title="Bezahlt am ${formatDate(entry.paid_at)}"
          >
            <wa-icon name="check" label="Bezahlt"></wa-icon>
          </wa-badge>
        `
      : null,
    entry.linked > 0
      ? html`
          <wa-badge
            pill
            class="linked"
            variant="neutral"
            title="${KINDS[otherKind(entry.kind)].linkedTo(entry.linked)}"
          >
            <wa-icon
              name="link"
              label="${KINDS[otherKind(entry.kind)].linkedTo(entry.linked)}"
            ></wa-icon>
          </wa-badge>
        `
      : null,
  ].filter(tag => tag !== null);
}

const yearOf = (entry: ListEntry) => entry.issue_date.slice(0, 4);

/**
 * Invoices or offers: a small filterable list on the left, the selected document on the right.
 * Without a selection in the address the newest one is shown.
 */
@customElement('erng-invoices')
export class InvoicesView extends ErngElement.withStyles(styles) {
  /** From the address: `new`, an id, `none` (nothing open), or empty for "the newest" */
  @property() selected = '';

  /** What the list holds; an element has one kind for good, the routes make a separate one for each */
  @property() kind: Kind = 'invoice';

  @state() private entries: ListEntry[] | undefined;
  @state() private query = '';
  /** The stages shown, any combination */
  @state() private stages = new Set<Stage>();
  /** Archived invoices are left out unless asked for, whatever their stage */
  @state() private withArchived = false;
  @state() private year = 'all';
  @state() private customer = 'all';
  /** The invoices of every context, not only of the active one */
  @state() private all = false;
  /** An invoice that got its id while the address still says `new` */
  @state() private created: number | undefined;

  override connectedCallback() {
    super.connectedCallback();
    this.stages = new Set(STAGES[this.kind].map(([stage]) => stage));
    void this.load(true);
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
      redirect(kindRoute(this.kind, newest.id));
    }
  }

  /** `open`: the address names a document to show, even one the filters leave out */
  private async load(open = false) {
    try {
      const context = activeContext.get()?.id ?? null;
      const entries = await api.invoicesList(
        this.all ? null : context,
        this.kind,
        this.withArchived
      );
      // the selected one may not be in this list (the filter changed): show the newest instead
      const id = Number(this.selected);
      if (this.selected !== '' && Number.isInteger(id) && !entries.some(e => e.id === id)) {
        // a document of the other kind belongs to the other list
        const other = await api.invoiceGet(id).catch(() => undefined);
        if (other && other.kind !== this.kind) {
          redirect(kindRoute(other.kind, id));
          return;
        }
        // opened by its address (a link from another document): an archived one is listed as well
        if (open && other?.archived_at && !this.withArchived) {
          this.withArchived = true;
          await this.load();
          return;
        }
        this.entries = entries;
        const newest = entries[0];
        redirect(newest ? kindRoute(this.kind, newest.id) : kindRoute(this.kind));
        return;
      }
      this.entries = entries;
      this.selectNewest();
    } catch (error) {
      notifyError(error);
    }
  }

  private async handleChanged(
    event: CustomEvent<{ id: number | null; deleted: boolean; restored: boolean }>
  ) {
    // the deleted invoice leaves the list at once: what shows next is picked from what is left
    if (event.detail.deleted && this.entries) {
      this.entries = this.entries.filter(e => e.id !== event.detail.id);
    }
    if (!event.detail.deleted && event.detail.id !== null && this.selected === 'new') {
      this.created = event.detail.id;
    }
    await this.load();
  }

  /** The checkboxes of the status menu: the menu stays open, so several can be picked in a row */
  private handleStatus(
    event: CustomEvent<{ item: HTMLElement & { value: string; checked: boolean } }>
  ) {
    event.preventDefault();
    const { value, checked } = event.detail.item;
    if (value === 'archived') {
      this.withArchived = checked;
      void this.load();
      return;
    }
    const stages = new Set(this.stages);
    if (checked) {
      stages.add(value as Stage);
    } else {
      stages.delete(value as Stage);
    }
    this.stages = stages;
  }

  /** What the status menu says when closed */
  private get statusLabel(): string {
    const all = STAGES[this.kind];
    const picked = all.filter(([stage]) => this.stages.has(stage)).map(([, name]) => name);
    const stages =
      picked.length === all.length ? 'Alle' : picked.length === 0 ? 'Keine' : picked.join(', ');
    return this.withArchived ? `${stages} + archivierte` : stages;
  }

  /** What the detail shows: the address wins, then the newest invoice, then a blank one */
  private get current(): string {
    // closed: the user put the invoice away, nothing is open until they pick one
    if (this.selected === 'none') {
      return '';
    }
    if (this.selected !== '') {
      return this.selected;
    }
    const newest = this.entries?.[0];
    // without documents nothing is open, a blank form only comes with "Neue Rechnung" or "Neues Angebot"
    return newest ? String(newest.id) : '';
  }

  private get highlighted(): string {
    return this.created === undefined ? this.current : String(this.created);
  }

  /** A new invoice exists only once it is saved as a draft, until then there is no second new one */
  private get unsaved() {
    return this.current === 'new' && this.created === undefined;
  }

  private get shown(): ListEntry[] {
    const query = this.query.trim().toLowerCase();
    return (this.entries ?? []).filter(
      e =>
        this.stages.has(stageOf(e)) &&
        (this.year === 'all' || yearOf(e) === this.year) &&
        (this.customer === 'all' || e.customer_name === this.customer) &&
        haystack(e).includes(query)
    );
  }

  /** What the year and customer filters offer: what the invoices have */
  private get years(): string[] {
    return [...new Set((this.entries ?? []).map(yearOf))].sort().reverse();
  }

  private get customers(): string[] {
    return [...new Set((this.entries ?? []).map(e => e.customer_name).filter(Boolean))].sort(
      (a, b) => a.localeCompare(b, 'de')
    );
  }

  override render() {
    const shown = this.shown;
    const texts = KINDS[this.kind];
    return html`
      <erng-split position-key="split:${this.kind}s" @invoice-changed="${this.handleChanged}">
        <erng-filter-bar
          slot="list"
          .query="${this.query}"
          placeholder="${texts.search}"
          @query-change="${(e: CustomEvent<string>) => (this.query = e.detail)}"
        >
          ${
            available.get().length > 1
              ? html`<wa-select
                  slot="filters"
                  size="s"
                  aria-label="Kontext"
                  .value="${this.all ? 'all' : 'active'}"
                  @change="${(e: Event) => {
                    this.all = String((e.target as WaSelect).value) === 'all';
                    void this.load();
                  }}"
                >
                  <wa-option value="active">Dieser Kontext</wa-option>
                  <wa-option value="all">Alle Kontexte</wa-option>
                </wa-select>`
              : nothing
          }
          <wa-dropdown slot="filters" @wa-select="${this.handleStatus}">
            <wa-button slot="trigger" size="s" appearance="outlined" with-caret>
              ${this.statusLabel}
            </wa-button>
            ${STAGES[this.kind].map(
              ([stage, name]) => html`
                <wa-dropdown-item
                  type="checkbox"
                  value="${stage}"
                  ?checked="${this.stages.has(stage)}"
                >
                  ${name}
                </wa-dropdown-item>
              `
            )}
            <wa-divider></wa-divider>
            <wa-dropdown-item type="checkbox" value="archived" ?checked="${this.withArchived}">
              Archivierte
            </wa-dropdown-item>
          </wa-dropdown>
          <wa-select
            slot="filters"
            size="s"
            aria-label="Jahr"
            .value="${this.year}"
            @change="${(e: Event) => (this.year = String((e.target as WaSelect).value))}"
          >
            <wa-option value="all">Alle Jahre</wa-option>
            ${this.years.map(year => html`<wa-option value="${year}">${year}</wa-option>`)}
          </wa-select>
          <wa-select
            slot="filters"
            size="s"
            aria-label="Kunde"
            .value="${this.customer}"
            @change="${(e: Event) => (this.customer = String((e.target as WaSelect).value))}"
          >
            <wa-option value="all">Alle Kunden</wa-option>
            ${this.customers.map(name => html`<wa-option value="${name}">${name}</wa-option>`)}
          </wa-select>
          <wa-button
            slot="actions"
            size="s"
            variant="brand"
            ?disabled="${this.unsaved}"
            @click="${() => goto(kindRoute(this.kind, 'new'))}"
          >
            ${texts.newLabel}
          </wa-button>
        </erng-filter-bar>
        ${
          this.entries && shown.length === 0
            ? html`
                <wa-callout slot="list" variant="neutral">
                  ${this.entries.length > 0 ? 'Keine Treffer.' : texts.empty}
                </wa-callout>
              `
            : nothing
        }

        <erng-list slot="list">
          ${
            this.unsaved
              ? html`<erng-list-item placeholder selected>${texts.newLabel}</erng-list-item>`
              : nothing
          }
          ${shown.map(
            e => html`
              <erng-list-item
                ?selected="${String(e.id) === this.highlighted}"
                @click="${() => goto(kindRoute(this.kind, e.id))}"
              >
                ${e.customer_name || '–'}
                <span slot="sub">
                  ${e.number ?? html`<wa-tag variant="neutral" size="s">Entwurf</wa-tag>`}
                </span>
                <span slot="sub-end">${formatDate(e.issue_date)}</span>
                ${
                  tagsOf(e, this.all).length > 0
                    ? html`<span slot="tags">${tagsOf(e, this.all)}</span>`
                    : nothing
                }
                <span slot="tags-end">${money(e.total)}</span>
              </erng-list-item>
            `
          )}
        </erng-list>

        ${
          this.entries
            ? this.current === ''
              ? html`<div slot="detail" class="placeholder">
                  <span> ${this.entries.length === 0 ? texts.empty : texts.none} </span>
                  <wa-button variant="brand" @click="${() => goto(kindRoute(this.kind, 'new'))}">
                    ${texts.newLabel}
                  </wa-button>
                </div>`
              : html`
                  <erng-invoice-edit
                    slot="detail"
                    kind="${this.kind}"
                    invoice-id="${this.current}"
                  ></erng-invoice-edit>
                `
            : nothing
        }
      </erng-split>
    `;
  }
}

declare global {
  interface HTMLElementEventMap {
    'invoice-changed': CustomEvent<{ id: number | null; deleted: boolean; restored: boolean }>;
  }

  interface HTMLElementTagNameMap {
    'erng-invoices': InvoicesView;
  }
}
