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
import '../../common/components/list/list-group/list-group.component.js';
import '../../common/components/list/list-item/list-item.component.js';
import '../../common/layouts/split/split.layout.js';
import '../invoice-edit/invoice-edit.view.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import { keyed } from 'lit/directives/keyed.js';

import type { Kind, ListEntry } from '../../common/utils/api.utils.js';
import { api } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import {
  activeContext,
  available,
  ready,
  readyForOffers,
} from '../../common/utils/context.utils.js';
import styles from '../../common/utils/documents.utils.css';
import type { Stage } from '../../common/utils/documents.utils.js';
import { stageOf, tagsOf, yearOf } from '../../common/utils/documents.utils.js';
import { formatDate, money } from '../../common/utils/format.utils.js';
import { haystack, kindRoute, KINDS } from '../../common/utils/kind.utils.js';
import { notifyError } from '../../common/utils/notify.utils.js';
import { goto, redirect } from '../../common/utils/routing.utils.js';

/** The stages of both kinds, a draft is one for both */
const STAGES: [Stage, string][] = [
  ['draft', 'Entwürfe'],
  ['valid', 'Angebote gültig'],
  ['expired', 'Angebote abgelaufen'],
  ['open', 'Rechnungen offen'],
  ['paid', 'Rechnungen bezahlt'],
];

/** The stages a kind can be in; a draft is either */
const STAGES_OF: Record<Kind, Stage[]> = {
  offer: ['draft', 'valid', 'expired'],
  invoice: ['draft', 'open', 'paid'],
};

/**
 * The address of the Vorgänge, or of a document in them: `new` becomes `new-offer` or
 * `new-invoice`, the kind is all a new one has
 */
export function casesRoute(kind: Kind, id?: number | string): string {
  if (id === undefined) {
    return '/cases';
  }
  return id === 'new' ? `/cases/new-${kind}` : `/cases/${id}`;
}

/** What a document is called in the list: its kind and number, or the kind's draft */
const titleOf = (entry: ListEntry) =>
  entry.number ? `${KINDS[entry.kind].noun} ${entry.number}` : KINDS[entry.kind].draftTitle;

/**
 * Offers and invoices together, each in its Vorgang: a filterable list of groups on the left, the
 * selected document on the right. Until Vorgänge are stored each document is a group of its own.
 */
@customElement('erng-cases')
export class CasesView extends ErngElement.withStyles(styles) {
  /** From the address: `new-offer`, `new-invoice`, an id, `none` (nothing open), or empty for "the newest" */
  @property() selected = '';

  @state() private entries: ListEntry[] | undefined;
  @state() private query = '';
  @state() private kind: Kind | 'all' = 'all';
  /** The stages shown, any combination */
  @state() private stages = new Set<Stage>(STAGES.map(([stage]) => stage));
  /** Archived documents are left out unless asked for, whatever their stage */
  @state() private withArchived = false;
  @state() private year = 'all';
  @state() private customer = 'all';
  /** The documents of every context, not only of the active one */
  @state() private all = false;
  /** A document that got its id while the address still says `new-…` */
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

  /** The bare address shows the newest document: make it the address, so a reload keeps the selection */
  private selectNewest() {
    const newest = this.entries?.[0];
    if (this.selected === '' && newest) {
      redirect(casesRoute(newest.kind, newest.id));
    }
  }

  private async load() {
    try {
      const context = activeContext.get()?.id ?? null;
      const entries = await api.invoicesList(this.all ? null : context, null, this.withArchived);
      // opened by its address, the document may be archived or of another context: list it anyway
      const id = Number(this.selected);
      if (this.selected !== '' && Number.isInteger(id) && !entries.some(e => e.id === id)) {
        const missing = await api.invoiceGet(id).catch(() => undefined);
        if (missing?.archived_at && !this.withArchived) {
          this.withArchived = true;
          await this.load();
          return;
        }
        this.entries = entries;
        const newest = entries[0];
        redirect(newest ? casesRoute(newest.kind, newest.id) : casesRoute('invoice'));
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
    // the deleted document leaves the list at once: what shows next is picked from what is left
    if (event.detail.deleted && this.entries) {
      this.entries = this.entries.filter(e => e.id !== event.detail.id);
    }
    if (!event.detail.deleted && event.detail.id !== null && this.newKind) {
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

  /** A new document starts here, unless its context lacks what the kind needs: its route sends the user there */
  private create(kind: Kind) {
    const fit = kind === 'offer' ? readyForOffers.get() : ready.get();
    goto(fit ? casesRoute(kind, 'new') : kindRoute(kind, 'new'));
  }

  /** The stages the status menu offers: those of the kind filtered for */
  private get offered(): [Stage, string][] {
    return this.kind === 'all'
      ? STAGES
      : STAGES.filter(([stage]) => STAGES_OF[this.kind as Kind].includes(stage));
  }

  /** What the status menu says when closed */
  private get statusLabel(): string {
    const offered = this.offered;
    const picked = offered.filter(([stage]) => this.stages.has(stage)).map(([, name]) => name);
    const stages =
      picked.length === offered.length ? 'Alle' : picked.length === 0 ? 'Keine' : picked.join(', ');
    return this.withArchived ? `${stages} + archivierte` : stages;
  }

  /** The kind of the new document the address asks for, if it does */
  private get newKind(): Kind | undefined {
    const [, kind] = /^new-(offer|invoice)$/.exec(this.selected) ?? [];
    return kind as Kind | undefined;
  }

  /** What the detail shows: the address wins, then the newest document */
  private get current(): { kind: Kind; id: string } | undefined {
    // closed: the user put the document away, nothing is open until they pick one
    if (this.selected === 'none') {
      return undefined;
    }
    const kind = this.newKind;
    if (kind) {
      return { kind, id: this.created === undefined ? 'new' : String(this.created) };
    }
    const id = this.selected === '' ? this.entries?.[0]?.id : Number(this.selected);
    const entry = this.entries?.find(e => e.id === id);
    return entry ? { kind: entry.kind, id: String(entry.id) } : undefined;
  }

  /** A new document exists only once it is saved as a draft, until then there is no second new one */
  private get unsaved() {
    return this.newKind !== undefined && this.created === undefined;
  }

  private get shown(): ListEntry[] {
    const query = this.query.trim().toLowerCase();
    return (this.entries ?? []).filter(
      e =>
        (this.kind === 'all' || e.kind === this.kind) &&
        this.stages.has(stageOf(e)) &&
        (this.year === 'all' || yearOf(e) === this.year) &&
        (this.customer === 'all' || e.customer_name === this.customer) &&
        haystack(e).includes(query)
    );
  }

  /** What the year and customer filters offer: what the documents have */
  private get years(): string[] {
    return [...new Set((this.entries ?? []).map(yearOf))].sort().reverse();
  }

  private get customers(): string[] {
    return [...new Set((this.entries ?? []).map(e => e.customer_name).filter(Boolean))].sort(
      (a, b) => a.localeCompare(b, 'de')
    );
  }

  private renderFilters() {
    return html`
      ${
        available.get().length > 1
          ? html`
              <wa-select
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
              </wa-select>
            `
          : nothing
      }
      <wa-select
        slot="filters"
        size="s"
        aria-label="Art"
        .value="${this.kind}"
        @change="${(e: Event) => (this.kind = String((e.target as WaSelect).value) as Kind | 'all')}"
      >
        <wa-option value="all">Alle Dokumente</wa-option>
        <wa-option value="offer">${KINDS.offer.list}</wa-option>
        <wa-option value="invoice">${KINDS.invoice.list}</wa-option>
      </wa-select>
      <wa-dropdown slot="filters" class="status" @wa-select="${this.handleStatus}">
        <wa-button slot="trigger" size="s" appearance="outlined" with-caret>
          ${this.statusLabel}
        </wa-button>
        ${this.offered.map(
          ([stage, name]) => html`
            <wa-dropdown-item type="checkbox" value="${stage}" ?checked="${this.stages.has(stage)}">
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
    `;
  }

  /** "Neu" offers both kinds */
  private renderCreate(slot: string) {
    return html`
      <wa-dropdown
        slot="${slot}"
        @wa-select="${(e: CustomEvent<{ item: { value: string } }>) =>
          this.create(e.detail.item.value as Kind)}"
      >
        <wa-button slot="trigger" size="s" variant="brand" with-caret ?disabled="${this.unsaved}">
          Neu
        </wa-button>
        <wa-dropdown-item value="offer">${KINDS.offer.newLabel}</wa-dropdown-item>
        <wa-dropdown-item value="invoice">${KINDS.invoice.newLabel}</wa-dropdown-item>
      </wa-dropdown>
    `;
  }

  /** One document inset in its group */
  private renderEntry(entry: ListEntry, highlighted: string | undefined) {
    const tags = tagsOf(entry, this.all);
    return html`
      <erng-list-item
        ?selected="${String(entry.id) === highlighted}"
        @click="${() => goto(casesRoute(entry.kind, entry.id))}"
      >
        ${titleOf(entry)}
        <span slot="sub-end">${formatDate(entry.issue_date)}</span>
        ${tags.length > 0 ? html`<span slot="tags">${tags}</span>` : nothing}
        <span slot="tags-end">${money(entry.total)}</span>
      </erng-list-item>
    `;
  }

  override render() {
    const shown = this.shown;
    const current = this.current;
    const newKind = this.newKind;
    return html`
      <erng-split position-key="split:cases" @invoice-changed="${this.handleChanged}">
        <erng-filter-bar
          slot="list"
          .query="${this.query}"
          placeholder="Vorgänge suchen"
          @query-change="${(e: CustomEvent<string>) => (this.query = e.detail)}"
        >
          ${this.renderFilters()} ${this.renderCreate('actions')}
        </erng-filter-bar>
        ${
          this.entries && shown.length === 0
            ? html`
                <wa-callout slot="list" variant="neutral">
                  ${this.entries.length > 0 ? 'Keine Treffer.' : 'Noch keine Vorgänge.'}
                </wa-callout>
              `
            : nothing
        }

        <erng-list slot="list">
          ${
            this.unsaved && newKind
              ? html`
                  <erng-list-group heading="Neuer Vorgang" placeholder>
                    <erng-list-item placeholder selected>${KINDS[newKind].newLabel}</erng-list-item>
                  </erng-list-group>
                `
              : nothing
          }
          ${shown.map(
            e => html`
              <erng-list-group heading="${e.customer_name || '–'}">
                ${this.renderEntry(e, current?.id)}
              </erng-list-group>
            `
          )}
        </erng-list>

        ${
          this.entries
            ? current
              ? // an editor is for one kind for good, another kind gets one of its own
                keyed(
                  current.kind,
                  html`
                    <erng-invoice-edit
                      slot="detail"
                      kind="${current.kind}"
                      invoice-id="${current.id}"
                      .route="${casesRoute}"
                    ></erng-invoice-edit>
                  `
                )
              : html`
                  <div slot="detail" class="placeholder">
                    <span>
                      ${this.entries.length === 0 ? 'Noch keine Vorgänge.' : 'Kein Beleg ausgewählt.'}
                    </span>
                    ${this.renderCreate('')}
                  </div>
                `
            : nothing
        }
      </erng-split>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-cases': CasesView;
  }
}
