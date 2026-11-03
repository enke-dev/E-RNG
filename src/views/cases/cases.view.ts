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
import '../../common/components/collect/collect.component.js';
import '../../common/components/new-case/new-case.component.js';
import '../../common/layouts/split/split.layout.js';
import '../invoice-edit/invoice-edit.view.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import { keyed } from 'lit/directives/keyed.js';

import type { Case, CaseStage, Kind, ListEntry } from '../../common/utils/api.utils.js';
import { api } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import type { Selection } from '../../common/utils/cases.utils.js';
import {
  CASE_STAGES,
  caseRoute,
  casesRoute,
  parseSelection,
  STAGE_VARIANTS,
} from '../../common/utils/cases.utils.js';
import {
  activeContext,
  available,
  contextById,
  label,
  ready,
  readyForOffers,
} from '../../common/utils/context.utils.js';
import styles from '../../common/utils/documents.utils.css';
import { yearOf } from '../../common/utils/documents.utils.js';
import { formatDate, money } from '../../common/utils/format.utils.js';
import { haystack, kindRoute, KINDS } from '../../common/utils/kind.utils.js';
import { notifyError } from '../../common/utils/notify.utils.js';
import { goto, redirect } from '../../common/utils/routing.utils.js';

const STAGES = Object.entries(CASE_STAGES) as [CaseStage, string][];

/**
 * What a Vorgang amounts to in the list: what was accepted or invoiced, whichever is more (an
 * Abschlag is less than the offer, extra work more), else what was offered
 */
const sumOf = (vorgang: Case) => {
  const settled = [vorgang.accepted, vorgang.invoiced].reduce((a, b) =>
    Number(b) > Number(a) ? b : a
  );
  return Number(settled) > 0 ? settled : vorgang.offered;
};

/** The Vorgänge of one customer the filters leave, the one changed last first */
interface Group {
  customer: string;
  cases: Case[];
}

/**
 * The Vorgänge, grouped by customer, as a filterable list on the left; on the right the selected
 * Vorgang with its documents, one of its documents (`case-12/7`), or a new one.
 */
@customElement('erng-cases')
export class CasesView extends ErngElement.withStyles(styles) {
  /**
   * From the address: `case-12` for a Vorgang, `case-12/7` for a document of it (a bare `7` finds
   * its Vorgang), `new-offer` or `new-invoice` (`@12` for one in Vorgang 12), `none` (nothing open),
   * or empty for "the newest"
   */
  @property() selected = '';

  @state() private entries: ListEntry[] | undefined;
  @state() private cases: Case[] = [];
  @state() private query = '';
  /** The stages shown, any combination */
  @state() private stages = new Set<CaseStage>(STAGES.map(([stage]) => stage));
  /** Archived Vorgänge are left out unless asked for, whatever their stage */
  @state() private withArchived = false;
  /** The address names a document or Vorgang the list does not know: nothing shown till read again */
  @state() private unknown = false;
  @state() private year = 'all';
  @state() private customer = 'all';
  /** The Vorgänge of every context, not only of the active one */
  @state() private all = false;
  /** A document that got its id while the address still says `new-…` */
  @state() private created: number | undefined;
  /** The dialog that starts a Vorgang is open */
  @state() private starting = false;
  /** The Sammelrechnung dialog is open */
  @state() private collecting = false;
  /** Counts the changes of documents, a Vorgang on show reads itself again */
  @state() private revision = 0;

  override connectedCallback() {
    super.connectedCallback();
    void this.load();
  }

  protected override willUpdate(changed: PropertyValues<this>) {
    if (changed.has('selected')) {
      this.created = undefined;
      this.unknown = this.entries !== undefined && this.namesUnknown();
    }
  }

  protected override updated(changed: PropertyValues<this>) {
    if (changed.has('selected')) {
      // a document or Vorgang the list does not know yet (made elsewhere since, or gone): read the
      // list again, it decides
      if (this.unknown) {
        void this.load();
        return;
      }
      this.settleAddress();
    }
  }

  /** Whether the address names a document or Vorgang that is not in the list */
  private namesUnknown(): boolean {
    const selection = this.selection;
    if (selection.type === 'document') {
      return !this.entries?.some(e => e.id === selection.id);
    }
    const named = this.namedCase;
    return named !== null && !this.cases.some(c => c.id === named);
  }

  private get selection(): Selection {
    return parseSelection(this.selected);
  }

  /**
   * The bare address shows the newest Vorgang and a document found by its id alone (or under
   * another Vorgang) is shown in its Vorgang: either becomes the address, so a reload keeps it
   */
  private settleAddress() {
    const selection = this.selection;
    if (selection.type === 'newest') {
      const newest = this.groups[0]?.cases[0];
      if (newest) {
        redirect(caseRoute(newest.id));
      }
    }
    if (selection.type === 'document') {
      const entry = this.entries?.find(e => e.id === selection.id);
      if (entry && entry.case_id !== selection.caseId) {
        redirect(casesRoute(entry.kind, entry.id, entry.case_id));
      }
    }
  }

  /** The Vorgang the address names, by its own id or as the one of a document */
  private get namedCase(): number | null {
    const selection = this.selection;
    switch (selection.type) {
      case 'case':
        return selection.id;
      case 'new':
      case 'document':
        return selection.caseId;
      default:
        return null;
    }
  }

  /** The Vorgang on show, or the one of the document on show */
  private get caseId(): number | null {
    const selection = this.selection;
    switch (selection.type) {
      case 'case':
        return selection.id;
      case 'new':
        return selection.caseId;
      case 'document':
        return selection.caseId ?? this.entries?.find(e => e.id === selection.id)?.case_id ?? null;
      default:
        return null;
    }
  }

  /**
   * Where the editor goes: a document in its Vorgang, back to the Vorgang when it closes (to
   * nothing for a new one of its own), to the newest Vorgang when it left the list
   */
  private readonly route = (kind: Kind, id?: number | string) => {
    if (id === 'none') {
      const caseId = this.caseId;
      return caseId === null ? casesRoute(kind, 'none') : caseRoute(caseId);
    }
    if (id === undefined || id === 'new') {
      return casesRoute(kind, id);
    }
    const caseId =
      this.entries?.find(e => e.id === Number(id))?.case_id ??
      (this.selection.type === 'new' ? this.selection.caseId : null);
    return casesRoute(kind, id, caseId ?? undefined);
  };

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
        this.unknown = true;
        this.entries = entries;
        this.cases = cases;
        redirect('/cases');
        return;
      }
      // a Vorgang gone (removed, or of data since replaced, as a remembered address): the list
      // instead, its newest Vorgang or that there is none, without an error; a document found is
      // shown in its own Vorgang (`settleAddress`), whatever the address said
      const named = selection.type === 'document' ? null : this.namedCase;
      if (named !== null && !cases.some(c => c.id === named)) {
        const missing = await api.caseGet(named).catch(() => undefined);
        if (missing?.archived && !this.withArchived) {
          this.withArchived = true;
          await this.load();
          return;
        }
        if (!missing) {
          this.unknown = true;
          this.entries = entries;
          this.cases = cases;
          redirect('/cases');
          return;
        }
      }
      this.entries = entries;
      this.cases = cases;
      this.unknown = false;
      this.settleAddress();
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
      stages.add(value as CaseStage);
    } else {
      stages.delete(value as CaseStage);
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

  /** What the status menu says when closed */
  private get statusLabel(): string {
    const picked = STAGES.filter(([stage]) => this.stages.has(stage)).map(([, name]) => name);
    const stages =
      picked.length === STAGES.length ? 'Alle' : picked.length === 0 ? 'Keine' : picked.join(', ');
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
        return undefined;
      case 'document': {
        const entry = this.entries?.find(e => e.id === selection.id);
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

  /**
   * The Vorgänge the filters leave, by customer: a Vorgang is found by its number, title and
   * customer or by any of its documents, a year by its documents' issue dates (one without any by
   * the day it was started). Inside a customer the one changed last comes first, so do customers.
   */
  private get groups(): Group[] {
    const query = this.query.trim().toLowerCase();
    const byCase = new Map<number, ListEntry[]>();
    (this.entries ?? []).forEach(e => byCase.set(e.case_id, [...(byCase.get(e.case_id) ?? []), e]));
    // documents say `2026-10-31 12:00:00`, Vorgänge `2026-10-31T12:00:00Z`: compared alike
    const moment = (at: string) => at.replace('T', ' ').replace('Z', '');
    const last = (vorgang: Case) =>
      (byCase.get(vorgang.id) ?? [])
        .map(e => moment(e.updated_at))
        .reduce((a, b) => (a > b ? a : b), moment(vorgang.created_at));
    const kept = this.cases
      .filter(vorgang => {
        const documents = byCase.get(vorgang.id) ?? [];
        const years =
          documents.length > 0 ? documents.map(yearOf) : [vorgang.created_at.slice(0, 4)];
        return (
          this.stages.has(vorgang.stage) &&
          (this.year === 'all' || years.includes(this.year)) &&
          (this.customer === 'all' || vorgang.customer_name === this.customer) &&
          ([vorgang.number, vorgang.title ?? '', vorgang.customer_name]
            .join(' ')
            .toLowerCase()
            .includes(query) ||
            documents.some(e => haystack(e).includes(query)))
        );
      })
      .map(vorgang => ({ vorgang, last: last(vorgang) }))
      .sort((a, b) => b.last.localeCompare(a.last));
    const groups = new Map<string, Case[]>();
    kept.forEach(({ vorgang }) => {
      const customer = vorgang.customer_name || 'Ohne Kunde';
      groups.set(customer, [...(groups.get(customer) ?? []), vorgang]);
    });
    return [...groups].map(([customer, cases]) => ({ customer, cases }));
  }

  /** What the year and customer filters offer: what the Vorgänge have */
  private get years(): string[] {
    return [
      ...new Set([
        ...(this.entries ?? []).map(yearOf),
        ...this.cases.map(c => c.created_at.slice(0, 4)),
      ]),
    ]
      .sort()
      .reverse();
  }

  private get customers(): string[] {
    return [...new Set(this.cases.map(c => c.customer_name).filter(Boolean))].sort((a, b) =>
      a.localeCompare(b, 'de')
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
      <wa-dropdown slot="filters" class="status" @wa-select="${this.handleStatus}">
        <wa-button slot="trigger" size="s" appearance="outlined" with-caret>
          ${this.statusLabel}
        </wa-button>
        ${STAGES.map(
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
          } else if (value === 'collect') {
            this.collecting = true;
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
        <wa-dropdown-item value="collect">Sammelrechnung …</wa-dropdown-item>
      </wa-dropdown>
    `;
  }

  /** A Vorgang: its number and sum, its title, where it stands; opens it */
  private renderCase(vorgang: Case) {
    const context = contextById(vorgang.context_id);
    return html`
      <erng-list-item
        ?selected="${this.caseId === vorgang.id}"
        ?archived="${vorgang.archived}"
        @click="${() => goto(caseRoute(vorgang.id))}"
      >
        ${vorgang.number}
        <span slot="end">${money(sumOf(vorgang))}</span>
        <span slot="sub">${vorgang.title ?? ''}</span>
        <span slot="sub-end">${formatDate(vorgang.created_at)}</span>
        <span slot="tags">
          ${
            this.all && context
              ? html`<wa-tag variant="neutral" size="s">${label(context)}</wa-tag>`
              : nothing
          }
          <wa-tag size="s" variant="${STAGE_VARIANTS[vorgang.stage]}">
            ${CASE_STAGES[vorgang.stage]}
          </wa-tag>
        </span>
      </erng-list-item>
    `;
  }

  /** A customer and its Vorgänge, the customer named once for all of them */
  private renderGroup({ customer, cases }: Group) {
    return html`
      <erng-list-group heading="${customer}">
        ${cases.map(vorgang => this.renderCase(vorgang))}
      </erng-list-group>
    `;
  }

  private renderDetail() {
    const current = this.current;
    if (!this.entries || this.unknown) {
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
            .route="${this.route}"
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
          ${groups.map(group => this.renderGroup(group))}
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
      <erng-collect
        ?open="${this.collecting}"
        @close="${() => (this.collecting = false)}"
        @case-changed="${() => void this.load()}"
      ></erng-collect>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-cases': CasesView;
  }
}
