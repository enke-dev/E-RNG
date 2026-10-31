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
import '../../common/components/case-detail/case-detail.component.js';
import '../../common/components/filter-bar/filter-bar.component.js';
import '../../common/components/list/list/list.component.js';
import '../../common/components/list/list-group/list-group.component.js';
import '../../common/components/list/list-item/list-item.component.js';
import '../../common/components/new-case/new-case.component.js';
import '../../common/layouts/split/split.layout.js';
import '../invoice-edit/invoice-edit.view.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import { keyed } from 'lit/directives/keyed.js';

import type { Case, Kind, ListEntry } from '../../common/utils/api.utils.js';
import { api } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import type { Selection } from '../../common/utils/cases.utils.js';
import { caseName, caseRoute, casesRoute, parseSelection } from '../../common/utils/cases.utils.js';
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
  ['accepted', 'Angebote angenommen'],
  ['declined', 'Angebote abgelehnt'],
  ['open', 'Rechnungen offen'],
  ['paid', 'Rechnungen bezahlt'],
];

/** The stages a kind can be in; a draft is either */
const STAGES_OF: Record<Kind, Stage[]> = {
  offer: ['draft', 'valid', 'expired', 'accepted', 'declined'],
  invoice: ['draft', 'open', 'paid'],
};

/** What a document is called in the list: its kind and number, or the kind's draft */
const titleOf = (entry: ListEntry) =>
  entry.number ? `${KINDS[entry.kind].noun} ${entry.number}` : KINDS[entry.kind].draftTitle;

/** A Vorgang with the documents of it the filters leave */
interface Group {
  vorgang: Case;
  documents: ListEntry[];
}

/**
 * Offers and invoices together, each in its Vorgang: a filterable list of Vorgänge with their
 * documents inset on the left; on the right the selected document, a new one, or a Vorgang itself.
 */
@customElement('erng-cases')
export class CasesView extends ErngElement.withStyles(styles) {
  /**
   * From the address: a document's id, `new-offer` or `new-invoice` (`@12` for one in Vorgang 12),
   * `case-12` for a Vorgang, `none` (nothing open), or empty for "the newest"
   */
  @property() selected = '';

  @state() private entries: ListEntry[] | undefined;
  @state() private cases: Case[] = [];
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
  /** The dialog that starts a Vorgang is open */
  @state() private starting = false;
  /** Counts the changes of documents, a Vorgang on show reads itself again */
  @state() private revision = 0;

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

  private get selection(): Selection {
    return parseSelection(this.selected);
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
      const context = this.all ? null : (activeContext.get()?.id ?? null);
      const [entries, cases] = await Promise.all([
        api.invoicesList(context, null, this.withArchived),
        api.casesList(context, this.withArchived),
      ]);
      // opened by its address, the document may be archived: list it anyway
      const selection = this.selection;
      if (selection.type === 'document' && !entries.some(e => e.id === selection.id)) {
        const missing = await api.invoiceGet(selection.id).catch(() => undefined);
        if (missing?.archived_at && !this.withArchived) {
          this.withArchived = true;
          await this.load();
          return;
        }
        this.entries = entries;
        this.cases = cases;
        const newest = entries[0];
        redirect(newest ? casesRoute(newest.kind, newest.id) : '/cases/none');
        return;
      }
      this.entries = entries;
      this.cases = cases;
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
    if (!event.detail.deleted && event.detail.id !== null && this.selection.type === 'new') {
      this.created = event.detail.id;
    }
    this.revision++;
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

  /**
   * A new document starts here, in a Vorgang or one of its own, unless its context lacks what the
   * kind needs: its route sends the user there
   */
  private create(kind: Kind, inCase?: number) {
    const fit = kind === 'offer' ? readyForOffers.get() : ready.get();
    goto(fit ? casesRoute(kind, 'new', inCase) : kindRoute(kind, 'new'));
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

  /** What the detail shows: a Vorgang, a new document (perhaps in a Vorgang) or a stored one */
  private get current():
    | { type: 'case'; id: number }
    | { type: 'document'; kind: Kind; id: string; caseId: number | null }
    | undefined {
    const selection = this.selection;
    switch (selection.type) {
      case 'none':
        return undefined;
      case 'case':
        return selection;
      case 'new':
        return {
          type: 'document',
          kind: selection.kind,
          id: this.created === undefined ? 'new' : String(this.created),
          caseId: selection.caseId,
        };
      case 'newest':
      case 'document': {
        const id = selection.type === 'newest' ? this.entries?.[0]?.id : selection.id;
        const entry = this.entries?.find(e => e.id === id);
        return entry
          ? { type: 'document', kind: entry.kind, id: String(entry.id), caseId: null }
          : undefined;
      }
    }
  }

  /** A new document exists only once it is saved as a draft, until then there is no second new one */
  private get unsaved() {
    return this.selection.type === 'new' && this.created === undefined;
  }

  /** Whether a Vorgang is what the search and the customer filter look for */
  private found(vorgang: Case, query: string) {
    return (
      (this.customer === 'all' || vorgang.customer_name === this.customer) &&
      [vorgang.number, vorgang.title ?? '', vorgang.customer_name]
        .join(' ')
        .toLowerCase()
        .includes(query)
    );
  }

  /**
   * The Vorgänge with the documents the filters leave, the one changed last first; inside a
   * Vorgang by issue date. A Vorgang found by its own name shows all its documents the other
   * filters leave, one without documents shows as long as nothing filters by documents.
   */
  private get groups(): Group[] {
    const query = this.query.trim().toLowerCase();
    const kept = (e: ListEntry) =>
      (this.kind === 'all' || e.kind === this.kind) &&
      this.stages.has(stageOf(e)) &&
      (this.year === 'all' || yearOf(e) === this.year) &&
      (this.customer === 'all' || e.customer_name === this.customer);
    const byCase = new Map<number, ListEntry[]>();
    (this.entries ?? []).forEach(e => byCase.set(e.case_id, [...(byCase.get(e.case_id) ?? []), e]));
    const filtering =
      this.kind !== 'all' || this.year !== 'all' || this.stages.size < STAGES.length;
    const groups = this.cases.flatMap((vorgang): (Group & { last: string })[] => {
      const all = byCase.get(vorgang.id) ?? [];
      const named = this.found(vorgang, query);
      const documents = all
        .filter(e => kept(e) && (named || haystack(e).includes(query)))
        .sort((a, b) => a.issue_date.localeCompare(b.issue_date) || a.id - b.id);
      const empty = all.length === 0 && !filtering && named;
      if (documents.length === 0 && !empty) {
        return [];
      }
      // documents say `2026-10-31 12:00:00`, Vorgänge `2026-10-31T12:00:00Z`: compared alike
      const moment = (at: string) => at.replace('T', ' ').replace('Z', '');
      const last = all
        .map(e => moment(e.updated_at))
        .reduce((a, b) => (a > b ? a : b), moment(vorgang.created_at));
      return [{ vorgang, documents, last }];
    });
    return groups.sort((a, b) => b.last.localeCompare(a.last));
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

  /** "Neu": an offer or invoice of its own, or a Vorgang to put them in */
  private renderCreate(slot: string) {
    return html`
      <wa-dropdown
        slot="${slot}"
        @wa-select="${(e: CustomEvent<{ item: { value: string } }>) => {
          const value = e.detail.item.value;
          if (value === 'case') {
            this.starting = true;
          } else {
            this.create(value as Kind);
          }
        }}"
      >
        <wa-button slot="trigger" size="s" variant="brand" with-caret ?disabled="${this.unsaved}">
          Neu
        </wa-button>
        <wa-dropdown-item value="offer">${KINDS.offer.newLabel}</wa-dropdown-item>
        <wa-dropdown-item value="invoice">${KINDS.invoice.newLabel}</wa-dropdown-item>
        <wa-divider></wa-divider>
        <wa-dropdown-item value="case">Neuer Vorgang …</wa-dropdown-item>
      </wa-dropdown>
    `;
  }

  /** One document inset in its Vorgang */
  private renderEntry(entry: ListEntry, highlighted: string | undefined) {
    const tags = tagsOf(entry, this.all);
    return html`
      <erng-list-item
        ?selected="${String(entry.id) === highlighted}"
        @click="${() => goto(casesRoute(entry.kind, entry.id))}"
      >
        ${titleOf(entry)}
        <span slot="end">${money(entry.total)}</span>
        <span slot="sub">${formatDate(entry.issue_date)}</span>
        ${tags.length > 0 ? html`<span slot="tags">${tags}</span>` : nothing}
      </erng-list-item>
    `;
  }

  /** A Vorgang: its number and name open it (the name is cut, not the number), its documents below */
  private renderGroup({ vorgang, documents }: Group, highlighted: string | undefined) {
    const current = this.current;
    const newHere = this.unsaved && this.selection.type === 'new' ? this.selection : undefined;
    return html`
      <erng-list-group
        pickable
        heading="${vorgang.number} · ${caseName(vorgang)}"
        ?selected="${current?.type === 'case' && current.id === vorgang.id}"
        @group-pick="${() => goto(caseRoute(vorgang.id))}"
      >
        <wa-dropdown
          slot="action"
          @wa-select="${(e: CustomEvent<{ item: { value: string } }>) =>
            this.create(e.detail.item.value as Kind, vorgang.id)}"
        >
          <wa-button slot="trigger" size="xs" appearance="outlined" ?disabled="${this.unsaved}">
            Neu
          </wa-button>
          <wa-dropdown-item value="offer">${KINDS.offer.newLabel}</wa-dropdown-item>
          <wa-dropdown-item value="invoice">${KINDS.invoice.newLabel}</wa-dropdown-item>
        </wa-dropdown>
        ${documents.map(e => this.renderEntry(e, highlighted))}
        ${
          newHere?.caseId === vorgang.id
            ? html`
                <erng-list-item placeholder selected>
                  ${KINDS[newHere.kind].newLabel}
                </erng-list-item>
              `
            : nothing
        }
      </erng-list-group>
    `;
  }

  private renderDetail() {
    const current = this.current;
    if (!this.entries) {
      return nothing;
    }
    if (current?.type === 'case') {
      return html`
        <erng-case-detail
          slot="detail"
          case-id="${current.id}"
          revision="${this.revision}"
        ></erng-case-detail>
      `;
    }
    if (current?.type === 'document') {
      // an editor is for one kind and one new document for good, another gets one of its own
      return keyed(
        `${current.kind}:${current.caseId ?? ''}`,
        html`
          <erng-invoice-edit
            slot="detail"
            kind="${current.kind}"
            invoice-id="${current.id}"
            case-id="${current.caseId ?? nothing}"
            .route="${casesRoute}"
          ></erng-invoice-edit>
        `
      );
    }
    return html`
      <div slot="detail" class="placeholder">
        <span>
          ${this.entries.length === 0 && this.cases.length === 0 ? 'Noch keine Vorgänge.' : 'Nichts ausgewählt.'}
        </span>
        ${this.renderCreate('')}
      </div>
    `;
  }

  override render() {
    const groups = this.groups;
    const current = this.current;
    const highlighted = current?.type === 'document' ? current.id : undefined;
    const selection = this.selection;
    const newAlone = this.unsaved && selection.type === 'new' && selection.caseId === null;
    return html`
      <erng-split
        position-key="split:cases"
        @invoice-changed="${this.handleChanged}"
        @case-changed="${() => void this.load()}"
      >
        <erng-filter-bar
          slot="list"
          .query="${this.query}"
          placeholder="Vorgänge suchen"
          @query-change="${(e: CustomEvent<string>) => (this.query = e.detail)}"
        >
          ${this.renderFilters()} ${this.renderCreate('actions')}
        </erng-filter-bar>
        ${
          this.entries && groups.length === 0 && !newAlone
            ? html`
                <wa-callout slot="list" variant="neutral">
                  ${this.cases.length > 0 ? 'Keine Treffer.' : 'Noch keine Vorgänge.'}
                </wa-callout>
              `
            : nothing
        }

        <erng-list slot="list">
          ${
            newAlone && selection.type === 'new'
              ? html`
                  <erng-list-group heading="Neuer Vorgang" placeholder>
                    <erng-list-item placeholder selected>
                      ${KINDS[selection.kind].newLabel}
                    </erng-list-item>
                  </erng-list-group>
                `
              : nothing
          }
          ${groups.map(group => this.renderGroup(group, highlighted))}
        </erng-list>

        ${this.renderDetail()}
      </erng-split>
      ${
        this.starting
          ? html`
              <erng-new-case
                context-id="${activeContext.get()?.id ?? 0}"
                @case-created="${(e: CustomEvent<Case>) => {
                  void this.load();
                  goto(caseRoute(e.detail.id));
                }}"
                @closed="${() => (this.starting = false)}"
              ></erng-new-case>
            `
          : nothing
      }
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-cases': CasesView;
  }
}
