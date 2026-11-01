import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/button-group/button-group.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/card/card.js';
import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/dropdown/dropdown.js';
import '@awesome.me/webawesome/dist/components/dropdown-item/dropdown-item.js';
import '@awesome.me/webawesome/dist/components/icon/icon.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/spinner/spinner.js';
import '@awesome.me/webawesome/dist/components/switch/switch.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';
import '@awesome.me/webawesome/dist/components/textarea/textarea.js';
import '../../common/components/customer-form/customer-form.component.js';
import '../../common/components/filter-bar/filter-bar.component.js';
import '../../common/components/history/history.component.js';
import '../../common/components/list/list/list.component.js';
import '../../common/components/list/list-item/list-item.component.js';
import '../../common/components/offer-answer/offer-answer.component.js';
import '../../common/components/payments/payments.component.js';
import '../../common/components/placeholders/placeholders.component.js';
import '../../common/components/sheet/sheet.component.js';
import '../../common/layouts/editor/editor.layout.js';
import '../../common/components/billing/billing.component.js';
import '../../common/components/offer-pick/offer-pick.component.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import type WaSwitch from '@awesome.me/webawesome/dist/components/switch/switch.js';
import { save } from '@tauri-apps/plugin-dialog';
import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import { ask, choose } from '../../common/components/ask/ask.component.js';
import type {
  Case,
  Check,
  Customer,
  InvoiceDraft,
  Kind,
  LayoutInfo,
  Linked,
  OfferBilling,
  Position,
  PositionTemplate,
  Preview,
  ProduceError,
  Rate,
  Relation,
  Settings,
  StoredInvoice,
  Unit,
} from '../../common/utils/api.utils.js';
import { api, errorText } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import { amount, quantityOf, sourceName } from '../../common/utils/billing.utils.js';
import { activeContext, contextById, ensureContexts } from '../../common/utils/context.utils.js';
import { RELATIONS } from '../../common/utils/documents.utils.js';
import { nullable, text, today } from '../../common/utils/form.utils.js';
import { formatDate, percent } from '../../common/utils/format.utils.js';
import {
  draftTitle,
  kindRoute,
  KINDS,
  otherKind,
  parseDays,
  validUntil,
} from '../../common/utils/kind.utils.js';
import { latest } from '../../common/utils/latest.utils.js';
import { notify, notifyError } from '../../common/utils/notify.utils.js';
import { LETTER_PLACEHOLDERS } from '../../common/utils/placeholders.utils.js';
import { chargesVat, pickRate, rateChoice, rateText } from '../../common/utils/rates.utils.js';
import reorderStyles from '../../common/utils/reorder.utils.css';
import { Reorder } from '../../common/utils/reorder.utils.js';
import { goto, redirect } from '../../common/utils/routing.utils.js';
import { pick, startUnit, unitLabel, unitOf } from '../../common/utils/units.utils.js';
import styles from './invoice-edit.view.css';

const blankCustomer = (): Customer => ({
  id: null,
  context_id: null,
  name: '',
  address: { street: '', post_code: '', city: '', country: 'DE' },
  vat_id: null,
  email: null,
  buyer_reference: null,
});

/** The value of the unit and rate selects for one that is not in the list */
const NOT_LISTED = 'not-listed';

@customElement('erng-invoice-edit')
export class InvoiceEditView extends ErngElement.withStyles(reorderStyles, styles) {
  /** `new` or the id of a stored invoice */
  @property({ attribute: 'invoice-id' }) invoiceId = 'new';

  /** What is edited, for good: the list makes one element per kind */
  @property() kind: Kind = 'invoice';

  /** A new document goes into this Vorgang and takes its customer; else it gets one of its own */
  @property({ type: Number, attribute: 'case-id' }) caseId: number | null = null;

  /** Where a document is shown: its kind's list, or the Vorgänge when opened from there */
  @property({ attribute: false }) route: (kind: Kind, id?: number | string) => string = kindRoute;

  private readonly reorder = new Reorder(this, {
    rows: '.position:not(.head)',
    noun: 'Position',
    count: () => this.draft?.positions.length ?? 0,
    move: (from, to) => this.movePosition(from, to),
  });

  @state() private stored: StoredInvoice | undefined;
  /** Counts what may have happened to the document, its history is read again on each */
  @state() private revision = 0;
  /** The Vorgang of the document, or the one a new document goes into */
  @state() private case: Case | undefined;
  @state() private draft: InvoiceDraft | undefined;
  @state() private settings: Settings | undefined;
  /** The context the invoice is made under: its own when stored, else the active one */
  @state() private contextId: number | undefined;
  @state() private customers: Customer[] = [];
  /** The units of the invoice's context, what the positions pick from */
  @state() private units: Unit[] = [];
  /** The VAT rates of the invoice's context, what the positions pick from */
  @state() private rates: Rate[] = [];
  @state() private layouts: LayoutInfo[] = [];
  @state() private check: Check | undefined;
  @state() private busy = false;
  @state() private newCustomer: Customer | undefined;
  /** An invoice is being created: checked, numbered, locked, its files kept */
  @state() private creating = false;
  /** Why the last attempt to create the invoice was refused, shown above the findings */
  @state() private createError: ProduceError | string | undefined;
  /** The originals are being copied out, or made again for an older invoice */
  @state() private outputting = false;
  /** What "Exportieren als" writes: the PDF, or for an XRechnung the XML */
  @state() private outputKind: 'pdf' | 'xml' = 'pdf';
  @state() private templates: PositionTemplate[] | undefined;
  @state() private templateQuery = '';
  @state() private preview: Preview | undefined;
  @state() private previewError = '';
  /** The documents of the other kind this one is billed with: the offers it bills, the invoices billing it */
  @state() private links: Linked[] = [];
  /** A created offer: what is billed of it */
  @state() private billing: OfferBilling | undefined;
  /** An invoice draft in a Vorgang: what is open of its offers, this draft left out */
  @state() private billings: OfferBilling[] = [];
  /** While offer positions are being picked */
  @state() private picking = false;
  /** An invoice draft is being made from this offer */
  @state() private converting = false;

  /** Edits that are not saved yet: a stored draft saves them shortly, a new one when it is left */
  #dirty = false;

  /** Counts the edits, a save only makes the form clean when nothing was typed while it ran */
  #revision = 0;

  #saveTimer: ReturnType<typeof setTimeout> | undefined;

  /** Each edit renders at once (a render takes milliseconds), one at a time */
  readonly #preview = latest(() => this.renderPreview());

  /** An answer for the invoice that was open before does not count */
  #previewRequest = 0;

  #timer: ReturnType<typeof setTimeout> | undefined;

  protected override willUpdate(changed: PropertyValues<this>) {
    if (changed.has('invoiceId')) {
      void this.switchTo();
    }
  }

  override disconnectedCallback() {
    super.disconnectedCallback();
    // leaving the view for another one keeps what was typed, like opening another invoice
    clearTimeout(this.#saveTimer);
    if (this.#dirty && this.draft && !this.locked) {
      void this.persist({ updateUrl: false }).catch(notifyError);
    }
    clearTimeout(this.#timer);
  }

  /** Opening another invoice keeps what was typed into a draft instead of dropping it */
  private async switchTo() {
    clearTimeout(this.#saveTimer);
    if (this.#dirty && this.draft && !this.locked) {
      try {
        await this.persist({ updateUrl: false });
        this.announce();
      } catch {
        // the draft stays as it was stored last, the new invoice opens anyway
      }
    }
    this.#dirty = false;
    // what was said about the last invoice (saved, exported) must not show up on the next one
    this.#previewRequest++;
    this.preview = undefined;
    await this.load();
  }

  /** Tells the list around us that invoices changed, so it can reload */
  private announce(options: { id?: number | null; deleted?: boolean; restored?: boolean } = {}) {
    this.revision++;
    void this.refreshCase();
    this.dispatchEvent(
      new CustomEvent('invoice-changed', {
        bubbles: true,
        composed: true,
        detail: {
          id: options.id ?? this.stored?.id ?? null,
          deleted: options.deleted ?? false,
          restored: options.restored ?? false,
        },
      })
    );
  }

  /** Whether the Vorgang locked its customer, read again after every change */
  private async refreshCase() {
    const id = this.stored?.case_id;
    if (id !== undefined) {
      // a Vorgang gone with its last draft keeps what was shown
      this.case = await api.caseGet(id).catch(() => this.case);
    }
  }

  /** The customer is the Vorgang's: it has another document or a created one (see `cases`) */
  private get customerFixed() {
    const owner = this.case;
    if (!owner || owner.customer_id === null) {
      return false;
    }
    return owner.locked || (this.stored === undefined && owner.documents > 0);
  }

  private get locked() {
    return this.stored?.status === 'exported';
  }

  private get texts() {
    return KINDS[this.kind];
  }

  /** The texts and layout of the settings that apply to the kind */
  private get defaults() {
    return this.kind === 'offer' ? this.settings?.offer : this.settings;
  }

  private async load() {
    try {
      const stored =
        this.invoiceId === 'new' ? undefined : await api.invoiceGet(Number(this.invoiceId));
      await ensureContexts();
      const caseId = stored?.case_id ?? this.caseId;
      const owner = caseId === null ? undefined : await api.caseGet(caseId);
      const contextId = stored?.context_id ?? owner?.context_id ?? activeContext.get()?.id;
      const context = contextById(contextId);
      if (contextId == null || !context) {
        throw new Error(this.texts.missingContext);
      }
      // a document of the other kind belongs to the other list
      if (stored && stored.kind !== this.kind) {
        redirect(this.route(stored.kind, stored.id));
        return;
      }
      const settings = context.settings;
      const [customers, layouts, units, rates] = await Promise.all([
        api.customersList(contextId),
        api.layoutsList(contextId),
        api.unitsList(contextId),
        api.ratesList(contextId),
      ]);
      this.contextId = contextId;
      this.layouts = layouts;
      this.units = units;
      this.rates = rates;
      this.settings = settings;
      this.customers = customers;
      this.case = owner;
      if (stored === undefined) {
        this.stored = undefined;
        // with the default layout and texts of its kind, which it can change for itself
        this.draft = await api.invoiceBlank(contextId, this.kind);
        // within a Vorgang it is for the Vorgang's customer
        const customer = customers.find(c => c.id !== null && c.id === owner?.customer_id);
        if (customer) {
          this.draft.customer = structuredClone(customer);
          this.draft.buyer_reference = customer.buyer_reference;
        }
        // a new position starts with the default unit of the context
        this.draft.positions.forEach(position =>
          Object.assign(position, startUnit(units, settings.unit))
        );
      } else {
        this.stored = stored;
        this.draft = stored.draft;
        this.outputKind = stored.draft.profile === 'xrechnung' ? 'xml' : 'pdf';
        // a draft without customer in a Vorgang that has one takes it, the select is locked then
        if (stored.status === 'draft' && owner?.locked && stored.draft.customer.id === null) {
          const customer = customers.find(c => c.id !== null && c.id === owner.customer_id);
          if (customer) {
            this.selectCustomer(String(customer.id));
          }
        }
      }
      this.links = stored ? await api.documentLinks(stored.id) : [];
      await this.loadBilling();
      await this.runCheck();
      void this.#preview();
    } catch (error) {
      notifyError(error);
    }
  }

  /** What is billed of a created offer; what is open of the offers of an invoice draft's Vorgang */
  private async loadBilling() {
    const { stored, kind } = this;
    this.billing =
      kind === 'offer' && stored?.status === 'exported'
        ? await api.offerBilling(stored.id)
        : undefined;
    const caseId = stored?.case_id ?? this.case?.id;
    this.billings =
      kind === 'invoice' && !this.locked && caseId !== undefined
        ? await api.caseBilling(caseId, stored?.id ?? null)
        : [];
  }

  /** Asks the real template renderer for the pages of the current form state */
  private async renderPreview() {
    if (!this.draft) {
      return;
    }
    const request = ++this.#previewRequest;
    try {
      const preview = await api.invoicePreview(
        this.draft,
        this.stored?.id ?? null,
        this.contextId as number,
        this.kind
      );
      if (request !== this.#previewRequest) {
        return;
      }
      this.preview = preview;
      this.previewError = '';
    } catch (error) {
      if (request === this.#previewRequest) {
        this.previewError = errorText(error);
      }
    }
  }

  private async runCheck() {
    if (this.draft) {
      this.check = await api.invoiceCheck(this.contextId as number, this.kind, this.draft);
    }
  }

  /** Call after every edit: renders the preview now, asks the backend for totals and findings shortly after */
  private changed() {
    this.#dirty = true;
    this.#revision++;
    clearTimeout(this.#saveTimer);
    if (this.stored && !this.locked) {
      this.#saveTimer = setTimeout(() => void this.autosave(), 500);
    }
    this.createError = undefined;
    this.requestUpdate();
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => void this.runCheck().catch(() => undefined), 300);
    void this.#preview();
  }

  private edit(change: (draft: InvoiceDraft) => void) {
    if (!this.draft) {
      return;
    }
    change(this.draft);
    this.changed();
  }

  private editPosition(index: number, change: (position: Position) => void) {
    this.edit(draft => {
      const position = draft.positions[index];
      if (position) {
        change(position);
      }
    });
  }

  private selectCustomer(id: string) {
    const customer = this.customers.find(c => String(c.id) === id);
    if (!customer) {
      return;
    }
    this.edit(draft => {
      draft.customer = structuredClone(customer);
      draft.buyer_reference = customer.buyer_reference;
    });
  }

  private async saveNewCustomer() {
    const customer = this.newCustomer;
    if (!customer) {
      return;
    }
    if (customer.name.trim() === '') {
      notify('Bitte einen Namen für den Kunden eingeben.', 'danger');
      return;
    }
    try {
      const saved = await api.customerSave(customer);
      this.customers = await api.customersList(this.contextId as number);
      this.newCustomer = undefined;
      this.selectCustomer(String(saved.id));
    } catch (error) {
      notifyError(error);
    }
  }

  private async persist({ updateUrl = true } = {}): Promise<StoredInvoice | undefined> {
    if (!this.draft || this.locked) {
      return this.stored;
    }
    const isNew = this.stored === undefined;
    const revision = this.#revision;
    this.stored = await api.invoiceSave(
      this.stored?.id ?? null,
      this.contextId ?? null,
      this.kind,
      this.draft,
      this.stored ? null : (this.case?.id ?? null)
    );
    // typed while saving: that is for the next save
    if (revision === this.#revision) {
      this.#dirty = false;
    }
    // the first save gives the new invoice its address, without reloading the form
    if (updateUrl && isNew && this.invoiceId === 'new') {
      history.replaceState({}, '', this.route(this.kind, this.stored.id));
    }
    return this.stored;
  }

  /** A stored draft keeps every edit by itself, shortly after the typing stops */
  private async autosave() {
    if (!this.#dirty || !this.stored || this.locked) {
      return;
    }
    try {
      await this.persist({ updateUrl: false });
      this.announce();
    } catch (error) {
      notifyError(error);
    }
  }

  private async saveDraft() {
    this.busy = true;
    try {
      await this.persist();
      this.announce();
      notify('Entwurf gespeichert.');
    } catch (error) {
      notifyError(error);
    } finally {
      this.busy = false;
    }
  }

  private async duplicate() {
    if (!this.stored) {
      return;
    }
    try {
      // a draft is copied as it is on screen, not as it was last saved
      if (this.#dirty) {
        await this.persist({ updateUrl: false });
      }
      const where = await choose(
        'Die Kopie kann einen eigenen Vorgang bekommen oder zu diesem gehören.',
        {
          title: `${this.texts.noun} duplizieren`,
          okLabel: 'Neuer Vorgang',
          altLabel: 'In diesem Vorgang',
          cancelLabel: 'Abbrechen',
        }
      );
      if (where === null) {
        return;
      }
      const copy = await api.invoiceDuplicate(this.stored.id, where === 'alt');
      this.#dirty = false;
      this.announce({ id: copy.id });
      goto(this.route(this.kind, copy.id));
    } catch (error) {
      notifyError(error);
    }
  }

  /** A new offer in this Vorgang that refers to this created one: a Nachtrag, an alternative, a replacement */
  private async relate(relation: Relation) {
    if (!this.stored) {
      return;
    }
    try {
      const offer = await api.offerRelated(this.stored.id, relation);
      this.announce({ id: offer.id });
      goto(this.route('offer', offer.id));
    } catch (error) {
      notifyError(error);
    }
  }

  /** Why an offer cannot get a related one of a kind, nothing when it can (as `related_offer` checks) */
  private relationRefused(relation: Relation): string {
    const { answer, lapsed } = this.stored ?? {};
    switch (relation) {
      case 'supplement':
        return answer === 'declined' ? 'Ein abgelehntes Angebot bekommt keinen Nachtrag.' : '';
      case 'alternative':
        return answer === 'accepted' || lapsed
          ? 'Ein Angebot dieser Gruppe ist schon angenommen.'
          : '';
      case 'replacement':
        return answer === 'accepted' ? 'Ein angenommenes Angebot wird nicht ersetzt.' : '';
    }
  }

  /** A created offer: a Nachtrag, an alternative or a replacement of it, made in its Vorgang */
  private renderRelate() {
    const relations = Object.entries(RELATIONS) as [Relation, (typeof RELATIONS)[Relation]][];
    return html`
      <wa-dropdown
        placement="top-end"
        @wa-select="${(e: CustomEvent<{ item: HTMLElement & { value: string } }>) =>
          void this.relate(e.detail.item.value as Relation)}"
      >
        <wa-button slot="trigger" appearance="outlined" with-caret>Angebot dazu</wa-button>
        ${relations.map(([relation, { noun, hint }]) => {
          const refused = this.relationRefused(relation);
          return html`
            <wa-dropdown-item
              value="${relation}"
              ?disabled="${refused !== ''}"
              title="${refused || hint}"
            >
              ${noun}
            </wa-dropdown-item>
          `;
        })}
      </wa-dropdown>
    `;
  }

  /** Puts the invoice away; edits of a draft are kept as a draft, like when another invoice opens */
  private async close() {
    if (this.#dirty && !this.locked) {
      try {
        await this.persist({ updateUrl: false });
        this.announce();
      } catch (error) {
        // what was typed stays on screen
        notifyError(error);
        return;
      }
    }
    goto(this.route(this.kind, 'none'));
  }

  /** A new invoice that was never saved is dropped, what was typed asks first */
  private async cancelNew() {
    if (this.#dirty) {
      const sure = await ask(this.texts.cancelText, {
        title: this.texts.cancelTitle,
        danger: true,
        okLabel: 'Verwerfen',
        cancelLabel: 'Zurück',
      });
      if (!sure) {
        return;
      }
    }
    // nothing of it is kept: no draft is saved on the way out
    this.#dirty = false;
    goto(this.route(this.kind, 'none'));
  }

  /** An exported invoice cannot go, it leaves the list instead and stays for the books */
  private async archive() {
    if (!this.stored) {
      return;
    }
    try {
      const id = this.stored.id;
      await api.invoiceArchive(id);
      this.announce({ id, deleted: true });
      notify(this.texts.archived);
      goto(this.route(this.kind));
    } catch (error) {
      notifyError(error);
    }
  }

  private async restore() {
    if (!this.stored) {
      return;
    }
    try {
      this.stored = await api.invoiceUnarchive(this.stored.id);
      this.announce({ restored: true });
      notify(this.texts.restored);
    } catch (error) {
      notifyError(error);
    }
  }

  private async deleteDraft() {
    if (!this.stored) {
      return;
    }
    try {
      const id = this.stored.id;
      await api.invoiceDelete(id);
      this.#dirty = false;
      this.announce({ id, deleted: true });
      goto(this.route(this.kind));
    } catch (error) {
      notifyError(error);
    }
  }

  /** The draft becomes an invoice: the user confirms the number, then it is checked, numbered, locked and its files are kept */
  private async create() {
    if (!this.draft || this.locked || this.creating) {
      return;
    }
    this.createError = undefined;
    this.creating = true;
    try {
      const stored = await this.persist();
      this.announce();
      if (!stored) {
        throw new Error('Die Rechnung konnte nicht gespeichert werden.');
      }
      const number = await api.invoiceNextNumber(stored.id);
      const sure = await ask(this.texts.confirm(number), {
        title: this.texts.createTitle,
        okLabel: 'Erstellen',
        cancelLabel: 'Abbrechen',
      });
      if (!sure) {
        return;
      }
      const created = await api.invoiceCreate(stored.id);
      this.stored = created.invoice;
      this.draft = created.invoice.draft;
      this.#dirty = false;
      this.announce();
      notify(this.texts.created(created.invoice.number ?? ''));
      if (created.warnings.length > 0) {
        notify(
          `Hinweise: ${created.warnings.map(w => w.message.slice(0, 120)).join(' ')}`,
          'neutral'
        );
      }
      await this.runCheck();
      void this.#preview();
    } catch (error) {
      this.createError = isProduceError(error) ? error : errorText(error);
    } finally {
      this.creating = false;
    }
  }

  /** Hands the kept originals out: where to put them is asked, nothing is written when a picker is cancelled */
  private async output(kind: 'pdf' | 'xml') {
    const stored = this.stored;
    if (!stored || !this.locked || this.outputting) {
      return;
    }
    const number = stored.number ?? this.texts.noun;
    try {
      const target = await save({
        defaultPath: `${fileName(number)}.${kind}`,
        filters: [{ name: kind.toUpperCase(), extensions: [kind] }],
      });
      // cancelling the picker means "not now": nothing is written
      if (!target) {
        return;
      }
      this.outputting = true;
      this.stored = await api.invoiceOutput(
        stored.id,
        kind === 'pdf' ? target : null,
        kind === 'xml' ? target : null
      );
      notify(`Als ${kind.toUpperCase()} gespeichert.`);
    } catch (error) {
      notifyError(error);
    } finally {
      this.outputting = false;
    }
  }

  /** The draft as a PDF to show around before it is created: it says draft and has no number */
  private async saveDraftPdf() {
    const stored = this.stored;
    if (!stored || !this.draft || this.locked || this.outputting) {
      return;
    }
    const customer = this.draft.customer.name.trim();
    try {
      const target = await save({
        defaultPath: `${fileName(`${draftTitle(this.settings, this.kind)}${customer ? ` ${customer}` : ''}`)}.pdf`,
        filters: [{ name: 'PDF', extensions: ['pdf'] }],
      });
      if (!target) {
        return;
      }
      this.outputting = true;
      await api.invoiceDraftPdf(this.draft, stored.id, this.contextId as number, this.kind, target);
      notify('Der Entwurf ist als PDF gespeichert.');
      // a version went out: the history has it, the preview shows the next one
      this.revision++;
      void this.#preview();
    } catch (error) {
      notifyError(error);
    } finally {
      this.outputting = false;
    }
  }

  private addPosition(from: 'blank' | 'template' | 'offer') {
    if (from === 'offer') {
      this.picking = true;
    } else if (from === 'template') {
      void this.openTemplates();
    } else {
      this.edit(d =>
        d.positions.push({
          description: '',
          quantity: null,
          ...startUnit(this.units, this.settings?.unit),
          tax_rate: null,
          tax_label: null,
          price: '',
        })
      );
    }
  }

  /** Picked offer positions join the invoice; an untouched blank first position makes room */
  private addBilled(positions: Position[]) {
    this.edit(draft => {
      const first = draft.positions[0];
      const untouched =
        draft.positions.length === 1 && first?.description === '' && first.price === '';
      draft.positions = [...(untouched ? [] : draft.positions), ...positions];
    });
  }

  /** What an invoice position refers to, and whether it bills more than is open of it */
  private renderSource(position: Position) {
    const source = position.source;
    if (!source) {
      return nothing;
    }
    const billing = this.billings.find(b => b.offer_id === source.offer_id);
    const number = billing?.number ?? this.links.find(l => l.id === source.offer_id)?.number ?? '';
    const open = billing?.positions[source.index]?.open;
    const over = !this.locked && open !== undefined && quantityOf(position) > Number(open);
    return html`
      <wa-tag
        class="source"
        size="s"
        appearance="outlined"
        variant="${over ? 'warning' : 'neutral'}"
        title="${over ? `Mehr als offen: ${amount(open ?? '0')}` : 'Aus dem Angebot übernommen'}"
      >
        ${sourceName(number, source.index)}${over ? ` · offen ${amount(open ?? '0')}` : ''}
      </wa-tag>
    `;
  }

  private movePosition(from: number, to: number) {
    this.edit(draft => {
      const [moved] = draft.positions.splice(from, 1);
      if (moved) {
        draft.positions.splice(to, 0, moved);
      }
    });
  }

  private async openTemplates() {
    this.templateQuery = '';
    this.templates = [];
    try {
      this.templates = await api.templatesList(this.contextId as number);
    } catch (error) {
      notifyError(error);
    }
  }

  private addTemplate(template: PositionTemplate) {
    this.edit(draft => {
      const { description, quantity, unit, unit_label, tax_rate, tax_label, price } = template;
      const first = draft.positions[0];
      const untouched =
        draft.positions.length === 1 && first?.description === '' && first.price === '';
      const position = { description, quantity, unit, unit_label, tax_rate, tax_label, price };
      if (untouched) {
        draft.positions[0] = position;
      } else {
        draft.positions.push(position);
      }
    });
    this.templates = undefined;
  }

  private async saveAsTemplate(position: Position) {
    if (position.description.trim() === '') {
      notify('Eine gespeicherte Position braucht eine Beschreibung.', 'danger');
      return;
    }
    try {
      const before = await api.templatesList(this.contextId as number);
      const saved = await api.templateSave({ ...position, id: null, context_id: null });
      // the same one is not saved twice, the saved one comes back
      if (before.some(t => t.id === saved.id)) {
        notify(`„${position.description}“ gibt es schon unter Positionen.`, 'neutral');
      } else {
        notify(`„${position.description}“ ist jetzt unter Positionen gespeichert.`);
      }
    } catch (error) {
      notifyError(error);
    }
  }

  /** Why a created offer cannot be billed (any more), nothing when it can */
  private get cannotBill(): string {
    const { stored, billing } = this;
    if (stored?.superseded || stored?.lapsed) {
      return 'Ein ersetztes oder entfallenes Angebot wird nicht in Rechnung gestellt.';
    }
    return billing?.positions.every(p => Number(p.open) <= 0)
      ? 'Dieses Angebot ist vollständig in Rechnung gestellt.'
      : '';
  }

  /** Invoices can be made under the offer's context only once it has what they need */
  private get invoicesReady() {
    return contextById(this.contextId)?.missing.length === 0;
  }

  /** A created offer becomes an invoice draft billing what is open of it; asks first when it has invoices already */
  private async invoiceFromOffer() {
    const offer = this.stored;
    if (!offer || this.kind !== 'offer' || !this.locked || this.converting) {
      return;
    }
    this.converting = true;
    try {
      this.links = await api.documentLinks(offer.id);
      const invoices = this.links.filter(l => l.kind === 'invoice');
      if (invoices.length > 0) {
        const numbers = invoices.flatMap(l => (l.number === null ? [] : [l.number]));
        const drafts = invoices.length - numbers.length;
        const named = new Intl.ListFormat('de', { type: 'conjunction' }).format([
          ...numbers,
          ...(drafts === 0
            ? []
            : [drafts === 1 ? 'ein Rechnungsentwurf' : `${drafts} Rechnungsentwürfe`]),
        ]);
        const sure = await ask(
          `Zu diesem Angebot gibt es schon ${named}. Eine Rechnung über das noch Offene anlegen?`,
          { title: 'Rechnung erstellen', okLabel: 'Anlegen', cancelLabel: 'Abbrechen' }
        );
        if (!sure) {
          return;
        }
      }
      const invoice = await api.offerToInvoice(offer.id);
      this.announce();
      notify(`Rechnungsentwurf aus Angebot ${offer.number ?? ''} angelegt.`);
      goto(this.route('invoice', invoice.id));
    } catch (error) {
      notifyError(error);
    } finally {
      this.converting = false;
    }
  }

  /** Payments and answers are the changes a created document allows: the store has the new state */
  private storedChanged(event: CustomEvent<StoredInvoice>) {
    this.stored = event.detail;
    this.announce();
  }

  /** The letter texts take placeholders: their label gets the button that lists them */
  private textPlaceholders(label: string, locked: boolean) {
    return !locked
      ? html`
          <span slot="label" class="label-row">
            ${label}
            <erng-placeholders .groups="${LETTER_PLACEHOLDERS}"></erng-placeholders>
          </span>
        `
      : nothing;
  }

  private renderPayment(stored: StoredInvoice) {
    return html`
      <wa-card>
        <div slot="header">Zahlung</div>
        <erng-payments
          .invoice="${stored}"
          @invoice-updated="${this.storedChanged}"
        ></erng-payments>
      </wa-card>
    `;
  }

  private renderAnswer(stored: StoredInvoice) {
    return html`
      <wa-card>
        <div slot="header">Antwort</div>
        <erng-offer-answer
          .offer="${stored}"
          @invoice-updated="${this.storedChanged}"
        ></erng-offer-answer>
      </wa-card>
    `;
  }

  /** What an invoice says about the service and the file format; an offer has neither */
  private renderService(draft: InvoiceDraft, locked: boolean) {
    return html`
      <div class="grid">
        <wa-input
          type="date"
          label="${draft.delivery_end === null ? 'Leistungsdatum' : 'Leistungszeitraum vom'}"
          ?disabled="${locked}"
          required
          .value="${draft.delivery_date}"
          @input="${(e: Event) => this.edit(d => (d.delivery_date = text(e)))}"
        ></wa-input>
        ${
          draft.delivery_end === null
            ? nothing
            : html`
                <wa-input
                  type="date"
                  label="bis"
                  ?disabled="${locked}"
                  required
                  .value="${draft.delivery_end}"
                  @input="${(e: Event) => this.edit(d => (d.delivery_end = text(e)))}"
                ></wa-input>
              `
        }
      </div>
      <wa-switch
        ?disabled="${locked}"
        ?checked="${draft.delivery_end !== null}"
        @change="${(e: Event) =>
          this.edit(d => {
            // a new period starts and ends on the day of service, the user moves its end
            d.delivery_end = (e.target as WaSwitch).checked
              ? (d.delivery_end ?? d.delivery_date)
              : null;
          })}"
      >
        Zeitraum
      </wa-switch>
      <wa-switch
        ?disabled="${locked}"
        ?checked="${draft.profile === 'xrechnung'}"
        @change="${(e: Event) => this.edit(d => (d.profile = (e.target as WaSwitch).checked ? 'xrechnung' : 'en16931'))}"
      >
        XRechnung (für öffentliche Auftraggeber)
      </wa-switch>
      ${
        draft.profile === 'xrechnung'
          ? html`
              <wa-input
                label="Leitweg-ID / Käuferreferenz"
                ?disabled="${locked}"
                required
                .value="${draft.buyer_reference ?? ''}"
                @input="${(e: Event) => this.edit(d => (d.buyer_reference = nullable(text(e))))}"
              ></wa-input>
            `
          : nothing
      }
    `;
  }

  /** How many days an offer is valid from its date; the day itself is worked out, never typed */
  private renderValidity(draft: InvoiceDraft, locked: boolean) {
    const until = validUntil(draft.issue_date, draft.validity_days);
    return html`
      <wa-input
        type="number"
        min="0"
        label="Gültigkeit (Tage)"
        hint="${until ? `bis ${formatDate(until)}` : 'Leer lassen, um keine Gültigkeit festzulegen.'}"
        ?disabled="${locked}"
        .value="${draft.validity_days === null ? '' : String(draft.validity_days)}"
        @input="${(e: Event) => this.edit(d => (d.validity_days = parseDays(text(e))))}"
      ></wa-input>
    `;
  }

  /** The date of the document; the (hidden) day of service of an offer goes along */
  private editIssueDate(value: string) {
    this.edit(d => {
      d.issue_date = value;
      if (this.kind === 'offer' && value !== '') {
        d.delivery_date = value;
      }
    });
  }

  private renderPreviewSheet() {
    const problems = this.preview?.problems ?? [];
    const hint = this.previewError
      ? `Vorschau nicht verfügbar: ${this.previewError}`
      : problems.length > 0
        ? problems.map(p => p.message).join(', ')
        : 'Die Vorschau wird erzeugt…';
    return html`
      <erng-sheet slot="preview" .pages="${this.preview?.pages ?? []}" hint="${hint}"></erng-sheet>
    `;
  }

  /** Called with the positions grid whenever it is (re)rendered or removed */
  /**
   * The unit of a position, picked from the list of the context. One that is not in it (deleted,
   * renamed since, from an older invoice) still shows, and can be switched; a locked invoice just
   * shows what it was made with.
   */
  private unitSelect(position: Position, index: number, locked: boolean) {
    const current = unitOf(this.units, position);
    return html`
      <wa-select
        label="Einheit"
        size="s"
        ?disabled="${locked}"
        .value="${current ? String(current.id) : NOT_LISTED}"
        @change="${(e: Event) => {
          const id = String((e.target as WaSelect).value);
          const unit = this.units.find(u => String(u.id) === id);
          this.editPosition(index, pos => pick(pos, unit));
        }}"
      >
        ${this.units.map(u => html`<wa-option value="${String(u.id)}">${u.label}</wa-option>`)}
        ${
          current
            ? nothing
            : html`
                <wa-option value="${NOT_LISTED}">
                  ${unitLabel(position)}${locked ? '' : ' (nicht in der Liste)'}
                </wa-option>
              `
        }
      </wa-select>
    `;
  }

  /** The default rate of the document: the frozen settings when it is created, else the context's */
  private get defaultRate() {
    return this.draft?.frozen?.tax_rate ?? this.settings?.tax_rate ?? '';
  }

  /**
   * The rate of a position, picked from the list of the context; one that picks none shows the
   * default of the context. One that is not in the list (deleted, changed since, from an older
   * document) still shows, and can be switched; the closed select shows only the percent, the open
   * list the labels.
   */
  private rateSelect(position: Position, index: number, locked: boolean) {
    const { selected, missing } = rateChoice(this.rates, position, this.defaultRate);
    return html`
      <wa-select
        label="USt."
        size="s"
        class="rate"
        ?disabled="${locked}"
        .value="${selected ? String(selected.id) : NOT_LISTED}"
        @change="${(e: Event) => {
          const id = String((e.target as WaSelect).value);
          const rate = this.rates.find(r => String(r.id) === id);
          this.editPosition(index, pos => pickRate(pos, rate));
        }}"
      >
        ${this.rates.map(
          r => html`
            <wa-option value="${String(r.id)}" .label="${percent(r.rate)}">
              ${rateText(r)}
            </wa-option>
          `
        )}
        ${
          missing
            ? html`
                <wa-option value="${NOT_LISTED}" .label="${missing.percent}">
                  ${missing.text}${locked ? '' : ' (nicht in der Liste)'}
                </wa-option>
              `
            : nothing
        }
      </wa-select>
    `;
  }

  private renderPositions(locked: boolean) {
    const draft = this.draft as InvoiceDraft;
    // a context without VAT has no rates to pick, the position takes none
    const taxed = chargesVat(this.defaultRate);
    return html`
      <div class="${taxed ? 'positions taxed' : 'positions'}">
        <div class="position head">
          <span></span>
          <span>Beschreibung</span>
          <span>Menge</span>
          <span>Einheit</span>
          ${taxed ? html`<span>USt.</span>` : nothing}
          <span>Preis (netto)</span>
          <span></span>
        </div>
        ${draft.positions.map(
          (p, i) => html`
            <div class="position">
              ${this.reorder.handle(i, locked)}
              <wa-textarea
                label="Beschreibung"
                rows="3"
                resize="auto"
                size="s"
                ?disabled="${locked}"
                .value="${p.description}"
                @input="${(e: Event) => this.editPosition(i, pos => (pos.description = text(e)))}"
              >
                ${p.source ? html`<span slot="hint">${this.renderSource(p)}</span>` : nothing}
              </wa-textarea>
              <wa-input
                label="Menge"
                placeholder="1"
                inputmode="decimal"
                size="s"
                ?disabled="${locked}"
                .value="${p.quantity ?? ''}"
                @input="${(e: Event) => this.editPosition(i, pos => (pos.quantity = nullable(text(e))))}"
              ></wa-input>
              ${this.unitSelect(p, i, locked)} ${taxed ? this.rateSelect(p, i, locked) : nothing}
              <wa-input
                label="Preis (netto)"
                inputmode="decimal"
                placeholder="0,00"
                size="s"
                ?disabled="${locked}"
                .value="${p.price}"
                @input="${(e: Event) => this.editPosition(i, pos => (pos.price = text(e)))}"
              ></wa-input>
              <wa-button-group class="row-actions" label="Position">
                <wa-button
                  size="s"
                  appearance="outlined"
                  title="Position für später speichern"
                  aria-label="Position für später speichern"
                  @click="${() => void this.saveAsTemplate(p)}"
                >
                  <wa-icon name="save"></wa-icon>
                </wa-button>
                <wa-button
                  size="s"
                  appearance="outlined"
                  title="Position entfernen"
                  aria-label="Position entfernen"
                  ?disabled="${locked || draft.positions.length === 1}"
                  @click="${() => this.edit(d => d.positions.splice(i, 1))}"
                >
                  <wa-icon name="trash"></wa-icon>
                </wa-button>
              </wa-button-group>
            </div>
          `
        )}
      </div>
      ${
        locked
          ? nothing
          : html`
              <wa-button-group class="add-position" label="Neue Position hinzufügen">
                <wa-dropdown
                  placement="bottom-end"
                  size="s"
                  @wa-select="${(e: CustomEvent<{ item: HTMLElement & { value: string } }>) =>
                    this.addPosition(e.detail.item.value as 'blank' | 'template' | 'offer')}"
                >
                  <wa-button slot="trigger" size="s" appearance="outlined" with-caret>
                    Neue Position
                  </wa-button>
                  <wa-dropdown-item value="blank">leer hinzufügen</wa-dropdown-item>
                  <wa-dropdown-item value="template">aus Vorlage</wa-dropdown-item>
                  ${
                    this.billings.length > 0
                      ? html`<wa-dropdown-item value="offer">aus Angebot</wa-dropdown-item>`
                      : nothing
                  }
                </wa-dropdown>
              </wa-button-group>
            `
      }
    `;
  }

  private renderTemplates() {
    const query = this.templateQuery.trim().toLowerCase();
    const shown = (this.templates ?? []).filter(t => t.description.toLowerCase().includes(query));
    const empty =
      this.templates?.length === 0
        ? 'Noch keine gespeicherten Positionen. Unter „Positionen“ oder mit „Position für später speichern“ anlegen.'
        : 'Keine Treffer.';
    return html`
      <wa-dialog
        ?open="${this.templates !== undefined}"
        label="Gespeicherte Position einfügen"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && (this.templates = undefined)}"
      >
        <div class="stack">
          <erng-filter-bar
            .query="${this.templateQuery}"
            placeholder="Positionen durchsuchen"
            @query-change="${(e: CustomEvent<string>) => (this.templateQuery = e.detail)}"
          ></erng-filter-bar>
          ${
            shown.length === 0
              ? html`<wa-callout variant="neutral">${empty}</wa-callout>`
              : html`
                  <div class="templates">
                    ${shown.map(
                      t => html`
                        <button
                          class="template"
                          type="button"
                          @click="${() => this.addTemplate(t)}"
                        >
                          <strong>${t.description}</strong>
                          <span class="muted"
                            >${t.quantity ?? '1'}
                            ${unitLabel(t)}${t.price.trim() ? ` × ${t.price} €` : ''}</span
                          >
                        </button>
                      `
                    )}
                  </div>
                `
          }
        </div>
        <div slot="footer" class="dialog-footer">
          <wa-button appearance="outlined" @click="${() => (this.templates = undefined)}">
            Schließen
          </wa-button>
        </div>
      </wa-dialog>
    `;
  }

  /** The offers an invoice bills, the invoices billing an offer and what is billed of it: read from the positions */
  private renderLinks() {
    const other = KINDS[otherKind(this.kind)];
    return html`
      <wa-card id="links">
        <div slot="header">${this.links.length === 1 ? other.noun : other.list}</div>
        <div class="stack">
          ${
            // a created offer says what is billed of it below
            this.links.length === 0 && this.billing
              ? nothing
              : this.links.length === 0
                ? html`
                    <span class="muted">
                      ${
                        this.kind === 'offer'
                          ? 'Noch keine Rechnung dazu.'
                          : 'Bezieht sich auf kein Angebot. Positionen „aus Angebot“ übernehmen.'
                      }
                    </span>
                  `
                : html`
                    <erng-list>
                      ${this.links.map(
                        l => html`
                          <div class="link">
                            <erng-list-item @click="${() => goto(this.route(l.kind, l.id))}">
                              ${l.customer_name || '–'}
                              <span slot="sub">
                                ${l.number ?? html`<wa-tag variant="neutral" size="s">Entwurf</wa-tag>`}
                              </span>
                              <span slot="sub-end">${formatDate(l.issue_date)}</span>
                            </erng-list-item>
                          </div>
                        `
                      )}
                    </erng-list>
                  `
          }
          ${
            this.kind === 'invoice' && this.draft?.positions.some(p => p.source)
              ? html`
                  <wa-switch
                    ?checked="${this.draft.offer_reference}"
                    ?disabled="${this.locked}"
                    hint="Steht bei den Angaben zur Rechnung. Mehrere Angebote werden alle genannt."
                    @change="${(e: Event) =>
                      this.edit(d => (d.offer_reference = (e.target as WaSwitch).checked))}"
                  >
                    Angebotsnummer auf der Rechnung angeben
                  </wa-switch>
                `
              : nothing
          }
          ${this.billing ? html`<erng-billing .billing="${this.billing}"></erng-billing>` : nothing}
        </div>
      </wa-card>
    `;
  }

  private renderCheck() {
    const check = this.check;
    if (!check) {
      return nothing;
    }
    const fatal = check.findings.filter(f => f.severity === 'Fatal');
    const advice = check.findings.filter(f => f.severity !== 'Fatal');
    return html`
      ${
        check.problems.length > 0
          ? html`
              <wa-callout variant="warning">
                Noch unvollständig:
                <ul>
                  ${check.problems.map(p => html`<li>${p.message}</li>`)}
                </ul>
              </wa-callout>
            `
          : nothing
      }
      ${
        fatal.length > 0
          ? html`
              <wa-callout variant="danger">
                Regelverstöße, die den Export verhindern:
                <ul>
                  ${fatal.map(f => html`<li><strong>${f.rule}</strong> ${f.message}</li>`)}
                </ul>
              </wa-callout>
            `
          : nothing
      }
      ${
        advice.length > 0
          ? html`
              <wa-callout variant="neutral">
                Hinweise:
                <ul>
                  ${advice.map(f => html`<li><strong>${f.rule}</strong> ${f.message}</li>`)}
                </ul>
              </wa-callout>
            `
          : nothing
      }
    `;
  }

  private renderFailure(error: ProduceError | string) {
    if (typeof error === 'string') {
      return html`<wa-callout variant="danger">${error}</wa-callout>`;
    }
    if (error.kind === 'failed') {
      return html`<wa-callout variant="danger">${error.message}</wa-callout>`;
    }
    if (error.kind === 'input') {
      return html`
        <wa-callout variant="warning">
          Die Angaben sind unvollständig:
          <ul>
            ${error.problems.map(p => html`<li>${p.message}</li>`)}
          </ul>
        </wa-callout>
      `;
    }
    if (error.kind === 'not-valid') {
      return html`
        <wa-callout variant="danger">
          Die erzeugte Datei hat die Prüfung nicht bestanden, die Rechnung wurde nicht erstellt:
          <ul>
            ${error.issues.map(i => html`<li><strong>${i.code}</strong> ${i.message.slice(0, 300)}</li>`)}
          </ul>
        </wa-callout>
      `;
    }
    return html`
      <wa-callout variant="danger">
        Die Rechnung verstößt gegen Regeln, sie wurde nicht erstellt:
        <ul>
          ${error.findings.map(f => html`<li><strong>${f.rule}</strong> ${f.message}</li>`)}
        </ul>
      </wa-callout>
    `;
  }

  /** What the document is: a draft, or created (numbered, checked) and perhaps paid, an offer valid until a day */
  private renderStatus() {
    const stored = this.stored;
    if (!stored) {
      return html`<div class="status"><wa-tag appearance="outlined">Neu</wa-tag></div>`;
    }
    const vorgang = this.case
      ? html`<wa-tag appearance="outlined" title="Vorgang">${this.case.number}</wa-tag>`
      : nothing;
    // the offer it refers to, as the document names it
    const related = stored.relation
      ? html`
          <wa-tag appearance="outlined" title="${RELATIONS[stored.relation].hint}">
            ${RELATIONS[stored.relation].of(stored.related_number ?? '')}
          </wa-tag>
        `
      : nothing;
    if (stored.status === 'draft') {
      return html`<div class="status">${vorgang}${related}<wa-tag>Entwurf</wa-tag></div>`;
    }
    const until = validUntil(stored.draft.issue_date, stored.draft.validity_days);
    return html`
      <div class="status">
        ${vorgang} ${related}
        <wa-tag>${stored.number}</wa-tag>
        ${stored.superseded ? html`<wa-tag variant="neutral">Ersetzt</wa-tag>` : nothing}
        ${stored.lapsed ? html`<wa-tag variant="neutral">Entfallen</wa-tag>` : nothing}
        ${stored.archived_at ? html`<wa-tag variant="neutral">Archiviert</wa-tag>` : nothing}
        ${
          stored.paid_at
            ? html`<wa-tag variant="success">Bezahlt am ${formatDate(stored.paid_at)}</wa-tag>`
            : stored.payments.length > 0
              ? html`<wa-tag variant="warning">Teilweise bezahlt</wa-tag>`
              : nothing
        }
        ${
          stored.answer
            ? html`
                <wa-tag variant="${stored.answer === 'accepted' ? 'success' : 'danger'}">
                  ${stored.answer === 'accepted' ? 'Angenommen' : 'Abgelehnt'} am
                  ${formatDate(stored.answered_on ?? '')}
                </wa-tag>
              `
            : nothing
        }
        ${
          this.kind === 'offer' && until && !stored.superseded && !stored.lapsed
            ? html`
                <wa-tag variant="${until < today() ? 'warning' : 'neutral'}">
                  Gültig bis ${formatDate(until)}
                </wa-tag>
              `
            : nothing
        }
      </div>
    `;
  }

  override render() {
    const draft = this.draft;
    if (!draft) {
      return nothing;
    }
    const locked = this.locked;
    const stored = this.stored;
    const blocked =
      (this.check?.problems.length ?? 0) > 0 ||
      (this.check?.findings.some(f => f.severity === 'Fatal') ?? false);

    return html`
      <erng-editor position-key="editor:${this.kind}">
        <section id="form" slot="form">
          ${
            locked
              ? html`
                  <wa-callout variant="neutral">
                    Erstellt als <strong>${stored?.number}</strong> am
                    ${formatDate(stored?.exported_at ?? '', true)}.<br>
                    <em>${this.texts.locked}</em>
                  </wa-callout>
                `
              : nothing
          }
          ${
            locked && stored
              ? this.kind === 'invoice'
                ? this.renderPayment(stored)
                : this.renderAnswer(stored)
              : nothing
          }

          <wa-card>
            <div slot="header">${this.texts.noun}</div>
            <div class="stack">
              <div class="grid">
                <div class="customer">
                  <wa-select
                    label="Kunde"
                    placeholder="Kunde auswählen"
                    hint="${this.customerFixed && !locked ? `Der Kunde des Vorgangs ${this.case?.number ?? ''}` : ''}"
                    ?disabled="${locked || this.customerFixed}"
                    required
                    .value="${draft.customer.id == null ? '' : String(draft.customer.id)}"
                    @change="${(e: Event) => this.selectCustomer(String((e.target as WaSelect).value))}"
                  >
                    ${this.customers.map(c => html`<wa-option value="${String(c.id)}">${c.name}</wa-option>`)}
                  </wa-select>
                  ${locked || this.customerFixed ? nothing : html`<wa-button @click="${() => (this.newCustomer = blankCustomer())}" appearance="outlined">Neuer Kunde</wa-button>`}
                </div>
                <wa-select
                  label="Layout"
                  ?disabled="${locked}"
                  .value="${draft.layout ?? this.defaults?.layout ?? this.settings?.layout ?? 'standard'}"
                  @change="${(e: Event) => this.edit(d => (d.layout = String((e.target as WaSelect).value)))}"
                >
                  ${this.layouts
                    .filter(l => !l.archived || l.id === draft.layout)
                    .map(l => html`<wa-option value="${l.id}">${l.name}</wa-option>`)}
                </wa-select>
              </div>
              ${
                draft.customer.name
                  ? html`
                      <div class="muted">
                        ${draft.customer.name}, ${draft.customer.address.street},
                        ${draft.customer.address.post_code} ${draft.customer.address.city}
                      </div>
                    `
                  : nothing
              }
              <div class="grid">
                <wa-input
                  type="date"
                  label="${this.texts.dateLabel}"
                  ?disabled="${locked}"
                  required
                  .value="${draft.issue_date}"
                  @input="${(e: Event) => this.editIssueDate(text(e))}"
                ></wa-input>
                ${this.kind === 'offer' ? this.renderValidity(draft, locked) : nothing}
              </div>
              ${this.kind === 'invoice' ? this.renderService(draft, locked) : nothing}
            </div>
          </wa-card>

          <wa-card id="positions">
            <div slot="header">Positionen</div>
            <div class="stack">${this.renderPositions(locked)}</div>
          </wa-card>

          <wa-card>
            <div slot="header">Texte</div>
            <div class="stack">
              <wa-textarea
                label="Einleitung"
                rows="2"
                ?disabled="${locked}"
                .placeholder="${this.defaults?.intro ?? ''}"
                .value="${draft.intro ?? ''}"
                @input="${(e: Event) => this.edit(d => (d.intro = text(e)))}"
              >
                ${this.textPlaceholders('Einleitung', locked)}
              </wa-textarea>
              <wa-textarea
                label="Anmerkung"
                hint="Steht nach der Einleitung, vor den Positionen"
                rows="2"
                ?disabled="${locked}"
                .value="${draft.note ?? ''}"
                @input="${(e: Event) => this.edit(d => (d.note = nullable(text(e))))}"
              ></wa-textarea>
              <wa-textarea
                label="Schlusstext"
                rows="2"
                ?disabled="${locked}"
                .placeholder="${this.defaults?.closing ?? ''}"
                .value="${draft.closing ?? ''}"
                @input="${(e: Event) => this.edit(d => (d.closing = text(e)))}"
              >
                ${this.textPlaceholders('Schlusstext', locked)}
              </wa-textarea>
            </div>
          </wa-card>

          ${this.renderLinks()} ${this.createError ? this.renderFailure(this.createError) : nothing}
          ${this.renderCheck()}
          ${
            stored
              ? html`
                  <wa-card>
                    <div slot="header">Verlauf</div>
                    <erng-history
                      invoice-id="${stored.id}"
                      revision="${this.revision}"
                      .current="${stored.status === 'draft' ? this.draft : undefined}"
                    ></erng-history>
                  </wa-card>
                `
              : nothing
          }
        </section>

        ${this.renderPreviewSheet()}
      </erng-editor>

      <footer>
        <div class="start">
          ${
            stored
              ? html`<wa-button appearance="outlined" @click="${this.close}">Schließen</wa-button>`
              : html`
                  <wa-button appearance="outlined" @click="${this.cancelNew}">
                    Abbrechen
                  </wa-button>
                `
          }
        </div>
        ${this.renderStatus()}
        <nav>
          ${
            stored
              ? html`
                  <wa-button-group label="${this.texts.noun}">
                    <wa-button appearance="outlined" @click="${this.duplicate}">
                      Duplizieren
                    </wa-button>
                    ${locked && this.kind === 'offer' ? this.renderRelate() : nothing}
                    ${
                      locked
                        ? stored.archived_at
                          ? html`
                              <wa-button appearance="outlined" @click="${this.restore}">
                                Wiederherstellen
                              </wa-button>
                            `
                          : html`
                              <wa-button appearance="outlined" @click="${this.archive}">
                                Archivieren
                              </wa-button>
                            `
                        : html`
                            <wa-button
                              appearance="outlined"
                              variant="danger"
                              @click="${this.deleteDraft}"
                            >
                              Entwurf löschen
                            </wa-button>
                          `
                    }
                  </wa-button-group>
                `
              : nothing
          }
          ${
            locked && this.kind === 'offer'
              ? // an offer has a PDF only: no format to pick
                html`
                  <wa-button
                    appearance="outlined"
                    ?loading="${this.outputting}"
                    @click="${() => void this.output('pdf')}"
                  >
                    Exportieren als PDF
                  </wa-button>
                  <wa-button
                    variant="brand"
                    ?loading="${this.converting}"
                    ?disabled="${!this.invoicesReady || this.cannotBill !== ''}"
                    title="${
                      this.cannotBill ||
                      (this.invoicesReady
                        ? ''
                        : 'Für Rechnungen fehlen noch Angaben in den Einstellungen.')
                    }"
                    @click="${this.invoiceFromOffer}"
                  >
                    Rechnung erstellen
                  </wa-button>
                `
              : locked
                ? html`
                    <wa-button-group label="Exportieren">
                      <wa-button
                        variant="brand"
                        ?loading="${this.outputting}"
                        @click="${() => void this.output(this.outputKind)}"
                      >
                        Exportieren als
                      </wa-button>
                      <wa-dropdown
                        placement="top-end"
                        @wa-select="${(
                          e: CustomEvent<{ item: HTMLElement & { value: string } }>
                        ) => {
                          this.outputKind = e.detail.item.value === 'xml' ? 'xml' : 'pdf';
                        }}"
                      >
                        <wa-button
                          slot="trigger"
                          variant="brand"
                          with-caret
                          ?disabled="${this.outputting}"
                          aria-label="Format zum Exportieren"
                        >
                          ${this.outputKind.toUpperCase()}
                        </wa-button>
                        <wa-dropdown-item value="pdf">PDF</wa-dropdown-item>
                        <wa-dropdown-item value="xml">XML</wa-dropdown-item>
                      </wa-dropdown>
                    </wa-button-group>
                  `
                : html`
                    <wa-button-group label="Speichern">
                      ${
                        // a stored draft saves itself, what it can still be saved as is a PDF to show
                        stored
                          ? html`
                              <wa-button
                                ?loading="${this.outputting}"
                                ?disabled="${this.creating}"
                                @click="${this.saveDraftPdf}"
                                appearance="outlined"
                              >
                                Exportieren als PDF
                              </wa-button>
                            `
                          : html`
                              <wa-button
                                ?loading="${this.busy}"
                                ?disabled="${this.creating}"
                                @click="${this.saveDraft}"
                                appearance="outlined"
                              >
                                Entwurf speichern
                              </wa-button>
                            `
                      }
                      <wa-button
                        variant="brand"
                        ?loading="${this.creating}"
                        ?disabled="${blocked}"
                        @click="${this.create}"
                      >
                        ${this.texts.create}
                      </wa-button>
                    </wa-button-group>
                  `
          }
        </nav>
      </footer>

      ${this.renderTemplates()}
      <erng-offer-pick
        .billings="${this.billings}"
        .positions="${this.draft?.positions ?? []}"
        ?open="${this.picking}"
        @pick="${(e: CustomEvent<Position[]>) => this.addBilled(e.detail)}"
        @close="${() => (this.picking = false)}"
      ></erng-offer-pick>

      <wa-dialog
        ?open="${this.newCustomer !== undefined}"
        label="Neuer Kunde"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && (this.newCustomer = undefined)}"
      >
        ${
          this.newCustomer
            ? html`
                <erng-customer-form
                  .customer="${this.newCustomer}"
                  @change="${() => this.requestUpdate()}"
                ></erng-customer-form>
              `
            : nothing
        }
        <div slot="footer" class="dialog-footer">
          <wa-button appearance="outlined" @click="${() => (this.newCustomer = undefined)}">
            Abbrechen
          </wa-button>
          <wa-button variant="brand" @click="${this.saveNewCustomer}">Speichern</wa-button>
        </div>
      </wa-dialog>
    `;
  }
}

/** A number like `12/26` is a name, not a folder: what a file name cannot hold becomes a dash */
function fileName(name: string): string {
  return name.replace(/[/\\:*?"<>|]/g, '-');
}

function isProduceError(error: unknown): error is ProduceError {
  return typeof error === 'object' && error !== null && 'kind' in error;
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-invoice-edit': InvoiceEditView;
  }
}
