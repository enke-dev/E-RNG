import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/switch/switch.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';
import '@awesome.me/webawesome/dist/components/tooltip/tooltip.js';
import '@awesome.me/webawesome/dist/components/textarea/textarea.js';
import '../../common/components/address-fields/address-fields.component.js';
import '../../common/components/layout-form/layout-form.component.js';
import '../../common/components/list/list/list.component.js';
import '../../common/components/list/list-group/list-group.component.js';
import '../../common/components/list/list-item/list-item.component.js';
import '../../common/components/page/page.component.js';
import '../../common/components/scope-select/scope-select.component.js';
import '../../common/components/rate-list/rate-list.component.js';
import '../../common/components/unit-list/unit-list.component.js';
import '../../common/layouts/editor/editor.layout.js';
import '../../common/layouts/split/split.layout.js';

import WaInput from '@awesome.me/webawesome/dist/components/input/input.js';
import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import type WaSwitch from '@awesome.me/webawesome/dist/components/switch/switch.js';
import { ask, open, save } from '@tauri-apps/plugin-dialog';
import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import { ifDefined } from 'lit/directives/if-defined.js';

import { BRANDS } from '../../common/components/brand-select/brand-select.component.js';
import { regionKey } from '../../common/components/sheet/sheet.component.js';
import type {
  Context,
  ContextUsage,
  CustomLayout,
  ImportChoice,
  ImportOutcome,
  Kind,
  LayoutInfo,
  LayoutUsage,
  Missing,
  NumberReset,
  Preview,
  Sequence,
  Settings,
} from '../../common/utils/api.utils.js';
import { api, errorText } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import {
  activate,
  activeContext,
  available,
  blockedAt,
  contextById,
  contexts,
  ensureContexts,
  label,
  loadContexts,
  switchContext,
} from '../../common/utils/context.utils.js';
import {
  inheritedDefaults,
  inheritedRates,
  inheritedUnits,
} from '../../common/utils/defaults.utils.js';
import { nullable, text, today } from '../../common/utils/form.utils.js';
import { parseDays } from '../../common/utils/kind.utils.js';
import { registerLeaveCheck } from '../../common/utils/leave.utils.js';
import { recall, remember } from '../../common/utils/memory.utils.js';
import { notify, notifyError } from '../../common/utils/notify.utils.js';
import { takeOpenedLayouts } from '../../common/utils/opened.utils.js';
import { indexAfterMove } from '../../common/utils/reorder.utils.js';
import { goto, redirect } from '../../common/utils/routing.utils.js';
import styles from './settings.view.css';

/** The pages of a context: all but the rates and units are saved together, those save themselves */
const PAGES = [
  ['company', 'Unternehmen'],
  ['bank', 'Bankverbindung'],
  ['rates', 'Steuersätze'],
  ['units', 'Einheiten'],
  ['offers', 'Angebote'],
  ['invoices', 'Rechnungen'],
] as const;

type Page = (typeof PAGES)[number][0];

/**
 * Common number patterns in different forms, shown with what they make today. `{P}` is the prefix
 * of the kind, the fiscal year one only shows when the fiscal year differs from the calendar year.
 */
const NUMBER_EXAMPLES: [pattern: string, reset: NumberReset, extra?: string, fiscal?: boolean][] = [
  ['[{P}-]YYYY-NNN', 'fiscal_year'],
  ['N/YY', 'fiscal_year', 'ohne führende Nullen'],
  ['YYYYMM-NNN', 'month'],
  ['YYYY-[Q]Q-NN', 'quarter'],
  ['GGGG/gg-NNN', 'fiscal_year', undefined, true],
  ['[{P}]NNNNN', 'never'],
];

/** The resets as the example notes name them */
const RESET_NAMES: Record<NumberReset, string> = {
  never: 'nie',
  fiscal_year: 'je Geschäftsjahr',
  quarter: 'je Quartal',
  month: 'je Monat',
};

/** `context-<id>-<page>`, or `context-new-<page>` for one that is not stored yet */
const CONTEXT_PAGE = /^context-(new|\d+)-(company|bank|rates|units|offers|invoices)$/;

/** What the form calls the fields that must be filled in before an invoice can be issued */
const FIELD_LABELS: Record<string, string> = {
  'seller.name': 'Name',
  'seller.address.street': 'Straße',
  'seller.address.post_code': 'PLZ',
  'seller.address.city': 'Ort',
  'seller.address.country': 'Land (zwei Buchstaben)',
  'seller.vat_id': 'USt-IdNr. oder Steuernummer',
  'seller.iban': 'IBAN',
  number_pattern: 'Muster der Rechnungsnummer',
  number_reset: 'Zähler periodisch zurücksetzen (Rechnungen)',
  'offer.number_pattern': 'Muster der Angebotsnummer',
  'offer.number_reset': 'Zähler periodisch zurücksetzen (Angebote)',
  tax_rate: 'Standard-Steuersatz (unter Steuersätze)',
  vat_note: 'Hinweis zur Steuerbefreiung',
  'seller.contact_name': 'Ansprechpartner',
  'seller.phone': 'Telefon',
  'seller.email': 'E-Mail',
};

const fieldNames = (fields: string[]) => fields.map(f => FIELD_LABELS[f] ?? f).join(', ');

const MONTHS = [
  'Januar',
  'Februar',
  'März',
  'April',
  'Mai',
  'Juni',
  'Juli',
  'August',
  'September',
  'Oktober',
  'November',
  'Dezember',
];

/** The ways the counter can start again, the business year is named by what the settings say */
const resetOptions = (fiscalYearStart: string | null): [NumberReset, string][] => {
  const [, month = 1, day = 1] = (fiscalYearStart ?? '').split('-').map(Number);
  const calendar = fiscalYearStart === null || (month === 1 && day === 1);
  return [
    ['never', 'Nie'],
    [
      'fiscal_year',
      calendar
        ? 'Je Geschäftsjahr (Kalenderjahr)'
        : `Je Geschäftsjahr (ab ${day}. ${MONTHS[month - 1]})`,
    ],
    ['quarter', 'Je Quartal'],
    ['month', 'Je Monat'],
  ];
};

const patternOf = (settings: Settings, kind: Kind) =>
  kind === 'offer' ? settings.offer.number_pattern : settings.number_pattern;

const resetOf = (settings: Settings, kind: Kind) =>
  kind === 'offer' ? settings.offer.number_reset : settings.number_reset;

/** What identifies a context's form values, brand and settings (the rest is computed) */
const snapshot = (context: Context | undefined) =>
  JSON.stringify(context && [context.brand, context.settings]);

const LAYOUT = 'layout-';

/** The lines of legal information the footer of the built-in layout has room for */
const LEGAL_LINES = 4;

/** The ending of a layout file */
const LAYOUT_EXTENSION = 'erng-layout';

/** The setup and the place of a part in it, from what `regionKey` made */
const parseKey = (key: string): { first: boolean; index: number } => {
  const [setup, index] = key.split(':');
  return { first: setup === 'first', index: Number(index) };
};

/**
 * Settings: on the left the contexts (the open one shows its pages: company, bank, offers, invoices) and
 * the layouts, the selected one on the right. The pages of a context share one Save button, a layout
 * is saved on its own.
 */
@customElement('erng-settings')
export class SettingsView extends ErngElement.withStyles(styles) {
  /** From the address: `context-<id>-<page>`, `layout-standard`, `layout-<id>`, `layout-new`; empty is the active context */
  @property() selected = '';

  /** A copy of the selected context, what its pages edit; one that is not stored yet has no id */
  @state() private context: Context | undefined;
  /** What the context in the form still lacks, asked of the backend while typing */
  @state() private missing: Missing = {
    missing: [],
    missing_for_xrechnung: [],
    missing_for_offers: [],
  };
  /** What belongs to the selected stored context, unknown while it is looked up */
  @state() private contextUsage: ContextUsage | undefined;
  /** The question of removing the selected context: what happens with what belongs to it */
  @state() private removing: { what: 'share' | 'move' | 'delete'; to: number | null } | undefined;

  /** What the next number would look like by the pattern in the form, or what is wrong with it */
  @state() private numberHints: Record<Kind, string> = { invoice: '', offer: '' };
  /** The common patterns of each kind with what they make today: pattern, number, note */
  @state() private numberExamples: Record<Kind, [string, string, string][]> = {
    invoice: [],
    offer: [],
  };
  /** Where the numbers of the selected stored context stand, by its saved settings */
  @state() private sequences: Partial<Record<Kind, Sequence>> = {};
  /** The dialog that continues the counter of a kind: what is typed and why it was refused */
  @state() private counterEdit: { kind: Kind; value: string; error: string } | undefined;
  @state() private layouts: LayoutInfo[] = [];
  @state() private customs: CustomLayout[] = [];
  /** A copy of the selected own layout, what its form edits */
  @state() private editing: CustomLayout | undefined;
  /** The invoices the selected own layout is used by, unknown while they are looked up */
  @state() private usage: LayoutUsage[] | undefined;
  /** The sample invoice in the layout of the form */
  @state() private sample: Preview | undefined;
  @state() private sampleError = '';
  /** Lines where the margins are, to see what a layout does */
  @state() private guides = recall('guides') === 1;
  /** What the user points at in the form: a page setup, and a part of it */
  @state() private pointed: { first: boolean; index: number | null } | undefined;
  /** The part under the pointer in the sample, or in the form */
  @state() private overSample: { first: boolean; index: number } | null = null;
  @state() private overForm: { first: boolean; index: number } | null = null;
  /** The part that is open in the form: the selected one */
  @state() private openPart: { first: boolean; index: number } | null = null;
  /** A part to bring into view in the sample, because it was opened in the form */
  @state() private showInSample: { key: string; token: number } | undefined;
  /** A part to open in the form, asked for by a click in the sample */
  @state() private reveal: { first: boolean; index: number; token: number } | undefined;

  /** The context as it is stored, what "unchanged" is compared against */
  #saved = '';
  #missingTimer: ReturnType<typeof setTimeout> | undefined;
  /** The tables of examples by prefix and fiscal year start, and the latest ask for a hint */
  #examples = new Map<string, Promise<[string, string, string][]>>();
  #exampleRequest = 0;
  #exampleTimer: ReturnType<typeof setTimeout> | undefined;
  /** The own layout as it is stored */
  #layoutSaved = '';

  /** What an import asks the user, and how the answer gets back to it */
  @state() private importQuestion:
    | {
        question: Exclude<ImportOutcome, { kind: 'done' }>;
        name: string;
        resolve: (answer: ImportChoice | undefined) => void;
      }
    | undefined;
  #sampleTimer: ReturnType<typeof setTimeout> | undefined;
  #sampleRequest = 0;

  private get changed() {
    return snapshot(this.context) !== this.#saved;
  }

  private get layoutChanged() {
    return this.editing !== undefined && JSON.stringify(this.editing) !== this.#layoutSaved;
  }

  private get section() {
    return this.selected;
  }

  /** The context and page on show, none on the pages of the layouts */
  private get contextRoute(): { id: number | 'new'; page: Page } | undefined {
    const found = CONTEXT_PAGE.exec(this.selected);
    if (!found) {
      return undefined;
    }
    const [, id = '', page = ''] = found;
    return { id: id === 'new' ? 'new' : Number(id), page: page as Page };
  }

  /** The context whose layouts the lists show: the one on show, else the active one */
  private get listContextId(): number | undefined {
    const shown = this.contextRoute?.id;
    return typeof shown === 'number' ? shown : (activeContext.get()?.id ?? undefined);
  }

  /** The id of the own layout on the page, `undefined` for other pages, `null` for a new one */
  private get layoutId(): number | null | undefined {
    const section = this.section;
    if (!section.startsWith(LAYOUT) || section === `${LAYOUT}standard`) {
      return undefined;
    }
    return section === `${LAYOUT}new` ? null : Number(section.slice(LAYOUT.length));
  }

  private get builtInName() {
    return this.layouts.find(l => !l.custom)?.name ?? '';
  }

  /** The layout new invoices of the active context start with */
  private get defaultLayout() {
    return activeContext.get()?.settings.layout ?? 'standard';
  }

  #stopOpened: (() => void) | undefined;
  #stopLeaveCheck: (() => void) | undefined;

  /** Whether going to `target` loses edits: the pages of a context share theirs, a layout has its own */
  readonly #losesEdits = (target: string): boolean => {
    const section = target.startsWith('/settings/') ? target.slice('/settings/'.length) : '';
    if (
      this.context &&
      this.changed &&
      !section.startsWith(`context-${this.context.id ?? 'new'}-`)
    ) {
      return true;
    }
    return this.layoutChanged && section !== this.selected;
  };

  override connectedCallback() {
    super.connectedCallback();
    this.#stopLeaveCheck = registerLeaveCheck(this.#losesEdits);
    // files the system opened with the app come here, to be imported like picked ones
    this.#stopOpened = takeOpenedLayouts(path =>
      this.layoutAction(async () => this.importFrom(path, {}))
    );
    ensureContexts().then(
      () => void this.enter(),
      (error: unknown) => notifyError(error)
    );
  }

  override disconnectedCallback() {
    super.disconnectedCallback();
    this.#stopOpened?.();
    this.#stopLeaveCheck?.();
  }

  protected override updated(changed: PropertyValues<this>) {
    if (changed.has('selected') && contexts.get()) {
      void this.enter();
    }
  }

  /** Shows what the address asks for; the bare address is the active context, or the form of the first one */
  private async enter() {
    if (this.selected === '') {
      const active = activeContext.get();
      redirect(active ? `/settings/context-${active.id}-company` : '/settings/context-new-company');
      return;
    }
    await this.openContext();
  }

  /** Takes the selected context into the form, the pages of one context share what is being edited */
  private async openContext() {
    const route = this.contextRoute;
    if (route && contexts.get()) {
      const keep = route.id === 'new' ? this.context?.id === null : this.context?.id === route.id;
      if (!keep) {
        await this.startEditing(route.id);
      }
    } else if (!route) {
      this.context = undefined;
    }
    await this.loadLayouts();
  }

  private async startEditing(id: number | 'new') {
    this.contextUsage = undefined;
    if (id === 'new') {
      const blank = await api.contextBlank();
      // the address moved on while the answer was on its way (a context was just saved): not for us
      if (this.contextRoute?.id !== 'new') {
        return;
      }
      const active = activeContext.get();
      if (active) {
        // the defaults of the one in use are a better start than the app's, the seller is its own
        blank.settings = { ...blank.settings, ...inheritedDefaults(active.settings) };
      }
      // the first context is blue, the next ones take a color no other context has
      const used = new Set((contexts.get() ?? []).map(c => c.brand));
      const free = BRANDS.map(([name]) => name).filter(name => !used.has(name));
      blank.brand =
        used.size === 0 ? 'blue' : (free[Math.floor(Math.random() * free.length)] ?? 'blue');
      this.context = blank;
    } else {
      const found = contextById(id);
      this.context = found ? structuredClone(found) : undefined;
      if (found) {
        void this.loadUsage(id);
      }
    }
    this.#saved = snapshot(this.context);
    this.missing = {
      missing: this.context?.missing ?? [],
      missing_for_xrechnung: this.context?.missing_for_xrechnung ?? [],
      missing_for_offers: this.context?.missing_for_offers ?? [],
    };
    void this.showNumberExample();
    void this.loadSequence();
  }

  /**
   * A list page saved something: the default of the context may have followed it (a changed or
   * replaced rate, a renamed or deleted unit, or the default chosen there). The settings in the
   * store are taken over here for those fields, other edits that are not saved yet stay.
   */
  private async listsChanged() {
    const id = this.context?.id;
    if (id == null) {
      return;
    }
    await loadContexts();
    const fresh = contextById(id);
    if (!fresh || this.context?.id !== id) {
      return;
    }
    const [, stored] = JSON.parse(this.#saved) as [string, Settings];
    const followed = (['tax_rate', 'unit'] as const).filter(
      field => fresh.settings[field] !== stored[field]
    );
    if (followed.length > 0) {
      const settings = { ...this.context.settings };
      followed.forEach(field => Object.assign(settings, { [field]: fresh.settings[field] }));
      this.context = { ...this.context, settings };
      this.#saved = snapshot(fresh);
    }
  }

  private async loadSequence() {
    const id = this.context?.id ?? null;
    this.sequences = {};
    if (id === null) {
      return;
    }
    try {
      const [invoice, offer] = await Promise.all([
        api.numberSequence(id, 'invoice'),
        api.numberSequence(id, 'offer'),
      ]);
      if (this.context?.id === id) {
        this.sequences = { invoice, offer };
      }
    } catch (error) {
      notifyError(error);
    }
  }

  private async loadUsage(id: number) {
    try {
      const usage = await api.contextUsage(id);
      if (this.context?.id === id) {
        this.contextUsage = usage;
      }
    } catch (error) {
      notifyError(error);
    }
  }

  private async loadLayouts() {
    const context = this.listContextId;
    if (context === undefined) {
      this.layouts = [];
      this.customs = [];
      return;
    }
    try {
      [this.layouts, this.customs] = await Promise.all([
        api.layoutsList(context),
        api.layoutsCustom(context),
      ]);
      await this.openLayout();
    } catch (error) {
      notifyError(error);
    }
  }

  /** Takes the selected own layout into the form */
  private async openLayout() {
    const id = this.layoutId;
    if (this.section === `${LAYOUT}standard`) {
      this.sample = undefined;
      this.schedulePreview(0);
    }
    // a new layout starts as the built-in one, parts and all
    const layout =
      id === null
        ? { ...(await api.layoutBuiltin()), name: '' }
        : this.customs.find(c => c.id === id);
    if (id === undefined || !layout) {
      this.editing = undefined;
      if (typeof id === 'number' && this.layouts.length > 0) {
        // the layout is not there for this context (it belongs to another one): the built-in one is
        redirect(`/settings/${LAYOUT}standard`);
      }
      return;
    }
    // what the user is typing stays when the list reloads
    if (this.editing?.id === layout.id && this.layoutChanged) {
      return;
    }
    this.editing = structuredClone(layout);
    this.#layoutSaved = JSON.stringify(this.editing);
    this.sample = undefined;
    this.pointed = undefined;
    this.overForm = null;
    this.openPart = null;
    this.schedulePreview(0);
    this.usage = undefined;
    if (id === null) {
      this.usage = [];
      return;
    }
    try {
      const usage = await api.layoutUsage(id);
      // the page may have moved on while the answer was on its way
      if (this.layoutId === id) {
        this.usage = usage;
      }
    } catch (error) {
      notifyError(error);
    }
  }

  /** The sample is rendered by the real template, a moment after the last edit */
  private schedulePreview(delay = 100) {
    clearTimeout(this.#sampleTimer);
    this.#sampleTimer = setTimeout(() => void this.renderSample(), delay);
  }

  private async renderSample() {
    const builtIn = this.section === `${LAYOUT}standard`;
    const layout = builtIn ? null : (this.editing ?? undefined);
    if (layout === undefined) {
      return;
    }
    const request = ++this.#sampleRequest;
    try {
      const sample = await api.layoutPreview(layout, this.guides, this.listContextId ?? null);
      if (request === this.#sampleRequest) {
        this.sample = sample;
        this.sampleError = '';
      }
    } catch (error) {
      if (request === this.#sampleRequest) {
        this.sampleError = errorText(error);
      }
    }
  }

  private edit(change: (settings: Settings) => void) {
    if (!this.context) {
      return;
    }
    change(this.context.settings);
    this.requestUpdate();
    this.askMissing();
  }

  /** What is still missing, asked of the backend shortly after the last keystroke */
  private askMissing() {
    clearTimeout(this.#missingTimer);
    this.#missingTimer = setTimeout(() => {
      const settings = this.context?.settings;
      if (settings) {
        api.settingsCheck(settings).then(
          missing => (this.missing = missing),
          () => undefined
        );
      }
    }, 150);
  }

  private seller(
    key: 'name' | 'contact_name' | 'phone' | 'email' | 'iban',
    caption: string,
    type = 'text',
    required = false
  ) {
    const settings = (this.context as Context).settings;
    return html`
      <wa-input
        label="${caption}"
        type="${type as WaInput['type']}"
        ?required="${required}"
        .value="${settings.seller[key]}"
        @input="${(e: Event) => this.edit(s => (s.seller[key] = text(e)))}"
      ></wa-input>
    `;
  }

  private optional(
    key:
      | 'vat_id'
      | 'tax_number'
      | 'bic'
      | 'account_holder'
      | 'bank_name'
      | 'mobile'
      | 'fax'
      | 'website',
    caption: string,
    type = 'text'
  ) {
    const settings = (this.context as Context).settings;
    return html`
      <wa-input
        label="${caption}"
        type="${type as WaInput['type']}"
        .value="${settings.seller[key] ?? ''}"
        @input="${(e: Event) => this.edit(s => (s.seller[key] = nullable(text(e))))}"
      ></wa-input>
    `;
  }

  /**
   * The common patterns of a kind with what they make today. They do not depend on what is typed,
   * only on the start of the fiscal year, so each table is asked for once.
   */
  private examplesOf(prefix: string, fiscal: string | null) {
    const key = `${prefix}|${fiscal ?? ''}`;
    const known = this.#examples.get(key);
    if (known) {
      return known;
    }
    // each example with the reset it fits, one that cannot work with the fiscal year is left out
    const table = Promise.all(
      NUMBER_EXAMPLES.filter(([, , , needsFiscal]) => !needsFiscal || fiscal !== null).map(
        async ([pattern, reset, extra]) => {
          const own = pattern.replace('{P}', prefix);
          const note = `passt zu: ${RESET_NAMES[reset]}${extra ? `, ${extra}` : ''}`;
          return [own, await api.numberExample(own, reset, fiscal).catch(() => ''), note] as [
            string,
            string,
            string,
          ];
        }
      )
    ).then(rows => rows.filter(([, number]) => number !== ''));
    this.#examples.set(key, table);
    return table;
  }

  /** The hint under the patterns shortly after the last keystroke, not with every one */
  private askNumberExample() {
    clearTimeout(this.#exampleTimer);
    this.#exampleTimer = setTimeout(() => void this.showNumberExample(), 150);
  }

  private async showNumberExample() {
    // a slower answer to an earlier call must not replace the one of a later call
    const request = ++this.#exampleRequest;
    const settings = this.context?.settings;
    const fiscal = settings?.fiscal_year_start ?? null;
    const hint = async (pattern: string, reset: NumberReset) => {
      try {
        return `Die nächste Nummer sähe so aus: ${await api.numberExample(pattern, reset, fiscal)}`;
      } catch (error) {
        return errorText(error);
      }
    };
    const [invoice, offer, invoiceExamples, offerExamples] = await Promise.all([
      hint(settings?.number_pattern ?? '', settings?.number_reset ?? 'fiscal_year'),
      hint(settings?.offer.number_pattern ?? '', settings?.offer.number_reset ?? 'fiscal_year'),
      this.examplesOf('RE', fiscal),
      this.examplesOf('AN', fiscal),
    ]);
    if (request !== this.#exampleRequest) {
      return;
    }
    this.numberHints = { invoice, offer };
    this.numberExamples = { invoice: invoiceExamples, offer: offerExamples };
  }

  private async save() {
    const context = this.context;
    if (!context) {
      return;
    }
    try {
      const wasNew = context.id === null;
      // a new context starts with a copy of the units and rates of the one in use
      const from = activeContext.get()?.id;
      const [units, rates] =
        wasNew && from != null
          ? await Promise.all([api.unitsList(from), api.ratesList(from)])
          : [undefined, undefined];
      const saved = await api.contextSave(
        context,
        units && inheritedUnits(units),
        rates && inheritedRates(rates)
      );
      await loadContexts();
      if (wasNew && saved.id !== null) {
        // the one just made is the one to work in
        activate(saved.id);
      }
      this.context = structuredClone(saved);
      this.#saved = snapshot(saved);
      this.missing = {
        missing: saved.missing,
        missing_for_xrechnung: saved.missing_for_xrechnung,
        missing_for_offers: saved.missing_for_offers,
      };
      void this.loadSequence();
      notify('Gespeichert. Bereits exportierte Rechnungen behalten ihre alten Angaben.');
      const back = blockedAt.get();
      // the way to the offers is open with less than the way to the invoices
      const open =
        (back?.startsWith('/offers') ? saved.missing_for_offers : saved.missing).length === 0;
      if (open && back !== undefined) {
        // the guard sent the user here, so they can go on where they wanted to
        blockedAt.set(undefined);
        goto(back);
      } else if (wasNew) {
        goto(`/settings/context-${saved.id}-company`);
      }
    } catch (error) {
      notifyError(error);
    }
  }

  /** Where to go when a context is gone: the next one in use, or the form for a new one */
  private afterContextGone() {
    const next = available.get()[0];
    goto(next ? `/settings/context-${next.id}-company` : '/settings/context-new-company');
  }

  /** An archived context leaves the switch, its invoices stay; there is no way back yet */
  private archiveContext() {
    const id = this.context?.id;
    if (id == null) {
      return;
    }
    return this.layoutAction(async () => {
      const sure = await ask(
        `„${label(this.context)}“ aus der Auswahl nehmen? Die Rechnungen bleiben erhalten. Wiederherstellen ist noch nicht möglich.`,
        { title: 'Kontext archivieren', okLabel: 'Archivieren', cancelLabel: 'Abbrechen' }
      );
      if (!sure) {
        return;
      }
      await api.contextArchive(id);
      await loadContexts();
      this.context = undefined;
      this.afterContextGone();
    });
  }

  /** What belongs to the context decides what the removal has to ask */
  private get scoped() {
    const usage = this.contextUsage;
    return usage ? usage.customers + usage.templates + usage.layouts : 0;
  }

  private openRemove() {
    this.removing = { what: 'share', to: null };
  }

  private confirmRemove() {
    const id = this.context?.id;
    const choice = this.removing;
    if (id == null || !choice) {
      return;
    }
    return this.layoutAction(async () => {
      const what =
        choice.what === 'move'
          ? { kind: 'move' as const, to: choice.to ?? 0 }
          : { kind: choice.what };
      await api.contextRemove(id, what);
      this.removing = undefined;
      await loadContexts();
      this.context = undefined;
      this.afterContextGone();
    });
  }

  private removeDialog() {
    const choice = this.removing;
    const others = available.get().filter(c => c.id !== this.context?.id);
    const usage = this.contextUsage;
    return html`
      <wa-dialog
        ?open="${choice !== undefined}"
        label="Kontext entfernen"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && (this.removing = undefined)}"
      >
        ${
          choice
            ? html`
                <div class="stack">
                  <span>
                    „${label(this.context)}“ wird entfernt. Es hat weder Rechnungen noch Angebote,
                    dieser Schritt lässt sich nicht rückgängig machen.
                  </span>
                  ${
                    usage && this.scoped > 0
                      ? html`
                          <span>
                            Zu diesem Kontext gehören ${usage.customers} Kunden, ${usage.templates}
                            gespeicherte Positionen und ${usage.layouts} Layouts. Was soll damit
                            geschehen?
                          </span>
                          <wa-select
                            label="Dazugehörige Einträge"
                            .value="${choice.what}"
                            @change="${(e: Event) => {
                              const what = String((e.target as WaSelect).value) as
                                'share' | 'move' | 'delete';
                              this.removing = {
                                what,
                                to: what === 'move' ? (others[0]?.id ?? null) : null,
                              };
                            }}"
                          >
                            <wa-option value="share">Für alle Kontexte freigeben</wa-option>
                            ${
                              others.length > 0
                                ? html`
                                    <wa-option value="move"
                                      >In einen anderen Kontext verschieben</wa-option
                                    >
                                  `
                                : nothing
                            }
                            <wa-option value="delete">Mit entfernen</wa-option>
                          </wa-select>
                          ${
                            choice.what === 'move'
                              ? html`
                                  <wa-select
                                    label="Verschieben nach"
                                    .value="${String(choice.to ?? '')}"
                                    @change="${(e: Event) => {
                                      this.removing = {
                                        what: 'move',
                                        to: Number((e.target as WaSelect).value),
                                      };
                                    }}"
                                  >
                                    ${others.map(
                                      c =>
                                        html`<wa-option value="${String(c.id)}"
                                          >${label(c)}</wa-option
                                        >`
                                    )}
                                  </wa-select>
                                `
                              : nothing
                          }
                        `
                      : nothing
                  }
                </div>
              `
            : nothing
        }
        <div slot="footer" class="dialog-footer">
          <wa-button appearance="outlined" @click="${() => (this.removing = undefined)}">
            Abbrechen
          </wa-button>
          <wa-button variant="danger" @click="${this.confirmRemove}">Entfernen</wa-button>
        </div>
      </wa-dialog>
    `;
  }

  /** The period of the counter depends on the saved reset and business year, the number on the pattern */
  private numberingEdited(kind: Kind) {
    const id = this.context?.id;
    const saved = id == null ? undefined : contextById(id)?.settings;
    const edited = this.context?.settings;
    return (
      !saved ||
      !edited ||
      patternOf(saved, kind) !== patternOf(edited, kind) ||
      resetOf(saved, kind) !== resetOf(edited, kind) ||
      saved.fiscal_year_start !== edited.fiscal_year_start
    );
  }

  private async changeCounter() {
    const id = this.context?.id;
    const dialog = this.counterEdit;
    if (id == null || !dialog) {
      return;
    }
    const next = Number(dialog.value);
    if (dialog.value.trim() === '' || !Number.isInteger(next) || next < 1) {
      this.counterEdit = { ...dialog, error: 'Bitte eine ganze Zahl ab 1 eingeben.' };
      return;
    }
    try {
      const sequence = await api.numberSequenceSet(id, dialog.kind, next);
      if (this.context?.id === id) {
        this.sequences = { ...this.sequences, [dialog.kind]: sequence };
      }
      this.counterEdit = undefined;
      notify(`Die nächste Nummer ist ${sequence.next_number}.`);
    } catch (error) {
      this.counterEdit = { ...dialog, error: errorText(error) };
    }
  }

  /** The next number and the way to continue the numbers of another program */
  private counterRow(kind: Kind) {
    const sequence = this.sequences[kind];
    if (!sequence) {
      return nothing;
    }
    const edited = this.numberingEdited(kind);
    return html`
      <div class="counter">
        <span>
          Nächste Nummer: <strong>${sequence.next_number}</strong>
          ${sequence.floor === null ? nothing : `(fortgesetzt ab ${sequence.floor})`}
        </span>
        <span id="change-counter">
          <wa-button
            appearance="outlined"
            ?disabled="${edited}"
            @click="${() => (this.counterEdit = { kind, value: String(sequence.counter), error: '' })}"
          >
            Zähler ändern…
          </wa-button>
        </span>
        ${
          edited
            ? html`
                <wa-tooltip for="change-counter">
                  Erst speichern: der Zähler gilt für die gespeicherten Einstellungen zu Muster,
                  Zurücksetzen und Geschäftsjahr.
                </wa-tooltip>
              `
            : nothing
        }
      </div>
    `;
  }

  private counterDialog() {
    const dialog = this.counterEdit;
    const sequence = dialog && this.sequences[dialog.kind];
    const highest = sequence?.highest;
    return html`
      <wa-dialog
        ?open="${dialog !== undefined}"
        label="Zähler ändern"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && (this.counterEdit = undefined)}"
      >
        ${
          dialog
            ? html`
                <div class="stack">
                  <wa-callout variant="warning">
                    Nur ändern, um die Nummern eines anderen Programms fortzusetzen. Eine Nummer
                    darf nie doppelt vergeben werden: Werte bis zur höchsten vergebenen
                    Nummer${highest ? html` (${highest})` : nothing} werden abgelehnt.
                  </wa-callout>
                  <wa-input
                    type="number"
                    min="1"
                    step="1"
                    label="Zähler der nächsten Nummer"
                    hint="${sequence ? `Jetzt: ${sequence.next_number}` : ''}"
                    .value="${dialog.value}"
                    @input="${(e: Event) => (this.counterEdit = { ...dialog, value: text(e), error: '' })}"
                    @keydown="${(e: KeyboardEvent) => e.key === 'Enter' && void this.changeCounter()}"
                  ></wa-input>
                  ${
                    dialog.error
                      ? html`<wa-callout variant="danger">${dialog.error}</wa-callout>`
                      : nothing
                  }
                </div>
              `
            : nothing
        }
        <div slot="footer" class="dialog-footer">
          <wa-button appearance="outlined" @click="${() => (this.counterEdit = undefined)}">
            Abbrechen
          </wa-button>
          <wa-button variant="brand" @click="${this.changeCounter}">Ändern</wa-button>
        </div>
      </wa-dialog>
    `;
  }

  // layouts

  /** Runs a layout action, shows what went wrong */
  private async layoutAction(action: () => Promise<void>) {
    try {
      await action();
    } catch (error) {
      notifyError(error);
    }
  }

  /** Reads a layout file: the layout is stored right away, removing it takes a delete */
  private importLayout() {
    return this.layoutAction(async () => {
      const path = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Layout', extensions: [LAYOUT_EXTENSION] }],
      });
      if (path) {
        await this.importFrom(path, {});
      }
    });
  }

  /** Asks to import; where the file needs an answer from the user it is asked, then tried again */
  private async importFrom(path: string, choice: ImportChoice): Promise<void> {
    const outcome = await api.layoutImport(path, choice);
    if (outcome.kind === 'done') {
      await this.loadLayouts();
      goto(`/settings/${LAYOUT}${outcome.layout.id}`);
      const missing = outcome.missing_fonts;
      notify(
        missing.length > 0
          ? `Layout importiert, es kam ohne Schrift: ${missing.join(', ')}. Bitte unter „Schrift“ ergänzen.`
          : 'Layout importiert.',
        missing.length > 0 ? 'warning' : 'success'
      );
      return;
    }
    const answer = await this.askAboutImport(outcome);
    if (answer) {
      await this.importFrom(path, { ...choice, ...answer });
    }
  }

  /** Shows the question of an import, resolves with the answer or nothing if it was dismissed */
  private askAboutImport(question: Exclude<ImportOutcome, { kind: 'done' }>) {
    return new Promise<ImportChoice | undefined>(resolve => {
      this.importQuestion = {
        question,
        name: question.kind === 'name-taken' ? question.suggested : '',
        resolve,
      };
    });
  }

  private answerImport(answer?: ImportChoice) {
    const pending = this.importQuestion;
    this.importQuestion = undefined;
    pending?.resolve(answer);
  }

  /** When the layout was saved, for a person */
  private when(iso: string) {
    return iso
      ? new Date(iso).toLocaleString('de-DE', { dateStyle: 'medium', timeStyle: 'short' })
      : 'unbekannt';
  }

  private importDialog() {
    const pending = this.importQuestion;
    const question = pending?.question;
    return html`
      <wa-dialog
        ?open="${pending !== undefined}"
        label="${question?.kind === 'replace' ? 'Layout ersetzen?' : 'Name vergeben'}"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && this.answerImport()}"
      >
        ${
          question?.kind === 'replace'
            ? html`
                <div class="stack">
                  <span>
                    „${question.existing.name}“ gibt es schon (gespeichert
                    ${this.when(question.existing.updated_at)}). Die Datei
                    ${
                      question.incoming_updated_at > question.existing.updated_at
                        ? 'ist neuer'
                        : question.incoming_updated_at < question.existing.updated_at
                          ? 'ist älter'
                          : 'hat denselben Stand'
                    }
                    (${this.when(question.incoming_updated_at)}).
                  </span>
                  ${
                    question.existing.uses > 0 || question.existing.is_default
                      ? html`
                          <span class="muted">
                            ${question.existing.is_default ? 'Es ist das Standard-Layout. ' : ''}
                            ${
                              question.existing.uses > 0
                                ? `${question.existing.uses} Rechnungen und Entwürfe verwenden es, bereits exportierte behalten ihr Aussehen.`
                                : ''
                            }
                          </span>
                        `
                      : nothing
                  }
                </div>
              `
            : question?.kind === 'name-taken'
              ? html`
                  <div class="stack">
                    <span>
                      Ein anderes Layout heißt schon „${question.name}“. Unter welchem Namen soll
                      das importierte gespeichert werden?
                    </span>
                    <wa-input
                      label="Name"
                      required
                      .value="${pending?.name ?? ''}"
                      @input="${(e: Event) => {
                        if (this.importQuestion) {
                          this.importQuestion = { ...this.importQuestion, name: text(e) };
                        }
                      }}"
                    ></wa-input>
                  </div>
                `
              : nothing
        }
        <div slot="footer" class="dialog-footer">
          <wa-button appearance="outlined" @click="${() => this.answerImport()}"
            >Abbrechen</wa-button
          >
          ${
            question?.kind === 'replace'
              ? html`
                  <wa-button
                    variant="${question.incoming_updated_at < question.existing.updated_at ? 'danger' : 'brand'}"
                    @click="${() => this.answerImport({ replace: true })}"
                  >
                    Ersetzen
                  </wa-button>
                `
              : html`
                  <wa-button
                    variant="brand"
                    ?disabled="${(pending?.name ?? '').trim() === ''}"
                    @click="${() => this.answerImport({ name: pending?.name })}"
                  >
                    Importieren
                  </wa-button>
                `
          }
        </div>
      </wa-dialog>
    `;
  }

  /** Writes a layout, as it is in the form, to a file; fonts only if the user wants to hand them out */
  private exportLayout(layout: CustomLayout) {
    return this.layoutAction(async () => {
      // closing the question counts as "no": fonts have licences, they leave only on purpose
      const withFonts =
        layout.fonts.length > 0 &&
        (await ask(
          'Sollen die Schriften mit exportiert werden? Prüfe, ob ihre Lizenz das erlaubt.',
          {
            title: 'Layout exportieren',
            okLabel: 'Mit Schriften',
            cancelLabel: 'Ohne Schriften',
          }
        ));
      const path = await save({
        defaultPath: `${layout.name.trim() || 'Layout'}.${LAYOUT_EXTENSION}`,
        filters: [{ name: 'Layout', extensions: [LAYOUT_EXTENSION] }],
      });
      if (!path) {
        return;
      }
      await api.layoutExport(layout, withFonts, path);
      notify('Layout exportiert.');
    });
  }

  private saveLayout() {
    return this.layoutAction(async () => {
      const layout = this.editing;
      if (!layout) {
        return;
      }
      if (layout.name.trim() === '') {
        throw new Error('Bitte dem Layout einen Namen geben.');
      }
      const saved = await api.layoutSave(layout);
      this.keep(saved);
      await this.loadLayouts();
      goto(`/settings/${LAYOUT}${saved.id}`);
    });
  }

  /** What was stored is what the form shows now, so there is nothing left to save */
  private keep(saved: CustomLayout) {
    this.editing = structuredClone(saved);
    this.#layoutSaved = JSON.stringify(this.editing);
  }

  private archiveLayout(archived: boolean) {
    return this.layoutAction(async () => {
      const layout = this.editing;
      if (!layout) {
        return;
      }
      this.keep(await api.layoutSave({ ...layout, archived }));
      await this.loadLayouts();
    });
  }

  private deleteLayout() {
    return this.layoutAction(async () => {
      const id = this.editing?.id;
      if (id == null) {
        return;
      }
      await api.layoutDelete(id);
      this.#layoutSaved = '';
      await this.loadLayouts();
      goto(`/settings/${LAYOUT}standard`);
    });
  }

  /** The default is stored at once, without what else is edited on the other pages */
  private makeDefault(key: string) {
    return this.layoutAction(async () => {
      const active = activeContext.get();
      if (!active) {
        return;
      }
      await api.contextSave({ ...active, settings: { ...active.settings, layout: key } });
      await loadContexts();
    });
  }

  // render

  private item(key: string, label: string, sub: string, tag: unknown = nothing, archived = false) {
    const kind = html`<wa-tag variant="neutral" size="s">${sub}</wa-tag>`;
    return html`
      <erng-list-item
        ?selected="${this.section === key}"
        ?archived="${archived}"
        @click="${() => goto(`/settings/${key}`)}"
      >
        ${label}
        <span slot="end">${tag}</span>
        <span slot="sub">${kind}</span>
      </erng-list-item>
    `;
  }

  /**
   * The contexts, each a group with the seller name as heading. Only the one on show has its pages
   * (the active one while a layout is on show), the others are empty until the user switches to them.
   */
  private renderContexts() {
    const active = activeContext.get()?.id;
    // on the pages of the layouts the active context stays open, its pages remain reachable
    const open = this.contextRoute?.id ?? active;
    const status = (c: Context | undefined) => {
      if (c === undefined) {
        return nothing;
      }
      if (c.id === null) {
        return html`<wa-tag size="s" appearance="outlined">Neu</wa-tag>`;
      }
      if (c.missing.length > 0) {
        return html`<wa-tag size="s" variant="warning">unvollständig</wa-tag>`;
      }
      return html`
        ${
          // with one context there is nothing to tell apart or to switch to
          available.get().length < 2
            ? nothing
            : c.id === active
              ? html`<wa-tag size="s" variant="brand">Aktiv</wa-tag>`
              : html`
                  <wa-button
                    size="xs"
                    appearance="outlined"
                    variant="neutral"
                    class="tag"
                    @click="${() => void switchContext(c.id as number)}"
                  >
                    Wechseln
                  </wa-button>
                `
        }
      `;
    };
    const group = (id: number | 'new', heading: string, extra: unknown = nothing) => html`
      <erng-list-group heading="${heading}" ?placeholder="${id === 'new'}">
        <span slot="action" class="row">${extra}</span>
        ${
          open === id
            ? PAGES.filter(([page]) => id !== 'new' || (page !== 'units' && page !== 'rates')).map(
                ([page, caption]) => html`
                  <erng-list-item
                    ?selected="${this.contextRoute?.page === page}"
                    @click="${() => goto(`/settings/context-${id}-${page}`)}"
                  >
                    ${caption}
                  </erng-list-item>
                `
              )
            : nothing
        }
      </erng-list-group>
    `;
    return html`
      ${available.get().map(c => group(c.id as number, label(c), status(c)))}
      ${open === 'new' ? group('new', label(this.context), status(this.context)) : nothing}
      ${
        open === 'new'
          ? nothing
          : html`
              <erng-list-item class="add" @click="${() => goto('/settings/context-new-company')}">
                + Kontext hinzufügen
              </erng-list-item>
            `
      }
    `;
  }

  private renderList() {
    const isDefault = (key: string) =>
      key === this.defaultLayout
        ? html`<wa-tag size="s" variant="brand">Standard</wa-tag>`
        : nothing;
    return html`
      <erng-list slot="list">
        ${this.renderContexts()}
        <erng-list-group heading="Layouts">
          <span slot="action" class="row">
            <wa-button
              size="xs"
              appearance="outlined"
              ?disabled="${this.section === `${LAYOUT}new`}"
              @click="${this.importLayout}"
            >
              Import
            </wa-button>
            <wa-button
              size="xs"
              appearance="outlined"
              ?disabled="${this.section === `${LAYOUT}new`}"
              @click="${() => goto(`/settings/${LAYOUT}new`)}"
            >
              Neu
            </wa-button>
          </span>
          ${
            this.section === `${LAYOUT}new`
              ? html`
                  <erng-list-item placeholder selected>
                    ${this.editing?.name.trim() || 'Unbenannt'}
                    <span slot="end"><wa-tag size="s" appearance="outlined">Neu</wa-tag></span>
                  </erng-list-item>
                `
              : nothing
          }
          ${this.item(`${LAYOUT}standard`, this.builtInName, 'integriert', isDefault('standard'))}
          ${this.customs.map(c =>
            this.item(
              `${LAYOUT}${c.id}`,
              c.name,
              c.archived ? 'Archiviert' : 'individuell',
              isDefault(`custom-${c.id}`),
              c.archived
            )
          )}
        </erng-list-group>
      </erng-list>
    `;
  }

  private companyPage(settings: Settings) {
    const context = this.context as Context;
    return html`
      <div class="name-row">
        ${this.seller('name', 'Name des Unternehmens', 'text', true)}
        <erng-brand-select
          .value="${context.brand}"
          @brand-change="${(e: CustomEvent<string>) => {
            context.brand = e.detail;
            this.requestUpdate();
          }}"
        ></erng-brand-select>
      </div>
      <erng-address-fields
        required
        .address="${settings.seller.address}"
        @change="${() => this.edit(() => undefined)}"
      ></erng-address-fields>
      <section>
        <div class="grid">
          ${this.optional('vat_id', 'USt-IdNr.')} ${this.optional('tax_number', 'Steuernummer')}
        </div>
        <span class="hint">
          Eine von beiden ist Pflicht: ohne sie ist eine Rechnung nicht gültig.
        </span>
      </section>
      <section>
        <div class="grid">
          ${this.seller('contact_name', 'Ansprechpartner')}
          ${this.seller('phone', 'Telefon', 'tel')} ${this.optional('mobile', 'Mobil', 'tel')}
          ${this.optional('fax', 'Fax', 'tel')} ${this.seller('email', 'E-Mail', 'email')}
          ${this.optional('website', 'Website', 'url')}
        </div>
        <span class="hint">
          Name, Telefon und E-Mail sind für XRechnung Pflicht, bei ZUGFeRD freiwillig. Mobil, Fax
          und Website stehen nicht in der XML-Datei, sie lassen sich nur über Platzhalter in eigenen
          Layouts zeigen.
        </span>
      </section>
      <wa-textarea
        label="Rechtliche Angaben"
        hint="Für Geschäftsbriefe, auch Rechnungen, je nach Rechtsform: z. B. bei einer GmbH Registergericht, HRB-Nummer und alle Geschäftsführer, bei einem e. K. Registergericht und HRA-Nummer. Leer lassen, wenn nichts eingetragen ist. Steht im Fuß der Rechnung und in der XML-Datei; im eingebauten Layout ist Platz für ${LEGAL_LINES} Zeilen."
        rows="${LEGAL_LINES}"
        .value="${settings.seller.legal_info ?? ''}"
        @input="${(e: Event) => {
          // more lines would run off the page in the built-in layout
          const field = e.target as HTMLTextAreaElement;
          const lines = field.value.split('\n');
          if (lines.length > LEGAL_LINES) {
            field.value = lines.slice(0, LEGAL_LINES).join('\n');
          }
          this.edit(s => (s.seller.legal_info = nullable(field.value)));
        }}"
      ></wa-textarea>
      <wa-switch
        ?checked="${settings.fiscal_year_start === null}"
        hint="Nimmt Einfluss auf die fortlaufende Nummerierung."
        @change="${(e: Event) => {
          const calendar = (e.target as WaSwitch).checked;
          this.edit(s => (s.fiscal_year_start = calendar ? null : `${today().slice(0, 4)}-01-01`));
          void this.showNumberExample();
        }}"
      >
        Geschäftsjahr entspricht Kalenderjahr
      </wa-switch>
      ${
        settings.fiscal_year_start === null
          ? nothing
          : html`
              <wa-input
                type="date"
                label="Beginn des ersten Geschäftsjahres"
                hint="Tag und Monat wiederholen sich jedes Jahr. Davor gilt das Kalenderjahr, das erste Geschäftsjahr kann also kürzer sein."
                .value="${settings.fiscal_year_start}"
                @input="${(e: Event) => {
                  // a half typed date is empty: keep the last good one
                  const date = text(e);
                  if (date !== '') {
                    this.edit(s => (s.fiscal_year_start = date));
                    void this.showNumberExample();
                  }
                }}"
              ></wa-input>
            `
      }
    `;
  }

  private bankPage() {
    return html`
      <section>
        <div class="grid">
          ${this.seller('iban', 'IBAN', 'text', true)} ${this.optional('bic', 'BIC')}
          ${this.optional('account_holder', 'Kontoinhaber')}
          ${this.optional('bank_name', 'Name der Bank')}
        </div>
        <span class="hint">
          Der Name der Bank steht nicht in der XML-Datei, er lässt sich nur über den Platzhalter
          {bank_name} in eigenen Layouts zeigen.
        </span>
      </section>
    `;
  }

  /** The pattern of a kind's numbers, how it is written, and where its counter stands */
  private numberFields(kind: Kind, settings: Settings) {
    const offer = kind === 'offer';
    return html`
      <wa-input
        label="${offer ? 'Muster der Angebotsnummer' : 'Muster der Rechnungsnummer'}"
        hint="${this.numberHints[kind]}"
        .value="${patternOf(settings, kind)}"
        @input="${(e: Event) => {
          this.edit(s => {
            if (offer) {
              s.offer.number_pattern = text(e);
            } else {
              s.number_pattern = text(e);
            }
          });
          this.askNumberExample();
        }}"
      ></wa-input>
      <wa-select
        label="Zähler periodisch zurücksetzen"
        hint="Das Muster formatiert nur. Es muss den Zeitraum enthalten, sonst wiederholen sich Nummern."
        .value="${resetOf(settings, kind)}"
        @change="${(e: Event) => {
          const reset = (e.target as WaSelect).value as NumberReset;
          this.edit(s => {
            if (offer) {
              s.offer.number_reset = reset;
            } else {
              s.number_reset = reset;
            }
          });
          void this.showNumberExample();
        }}"
      >
        ${resetOptions(settings.fiscal_year_start).map(
          ([value, label]) => html`<wa-option value="${value}">${label}</wa-option>`
        )}
      </wa-select>
      <wa-callout variant="brand" size="small" class="numbering">
        <wa-icon slot="icon" name="info"></wa-icon>
        Jahr YYYY (2026) oder YY, Monat MM (03) oder M, Tag DD (07) oder D, Zähler N: NNN füllt mit
        Nullen auf drei Stellen auf (007), N gar nicht.
        ${
          // the fiscal year marks only matter when it differs from the calendar year
          settings.fiscal_year_start === null
            ? 'Quartal Q.'
            : html`
                Geschäftsjahr: GGGG oder GG für das Jahr, in dem es beginnt, gggg oder gg für das
                Jahr, in dem es endet (<code>GGGG/gg</code> gibt 2026/27), Q für sein Quartal.
              `
        }
        Buchstaben als Text in [eckige Klammern].
        <table class="examples">
          <thead>
            <tr>
              <th>Muster</th>
              <th>Ergibt</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            ${this.numberExamples[kind].map(
              ([pattern, number, note]) => html`
                <tr>
                  <td><code>${pattern}</code></td>
                  <td>${number}</td>
                  <td>${note}</td>
                </tr>
              `
            )}
          </tbody>
        </table>
      </wa-callout>
      ${this.counterRow(kind)}
    `;
  }

  /** What a kind of document calls itself: headline, headline of a draft, label of the number */
  private wordingFields(kind: Kind, settings: Settings) {
    const offer = kind === 'offer';
    const wording = offer ? settings.offer.wording : settings.invoice_wording;
    const [title, draft, number] = offer
      ? ['Angebot', 'Angebotsentwurf', 'Angebotsnummer']
      : ['Rechnung', 'Rechnungsentwurf', 'Rechnungsnummer'];
    const field = (
      key: 'title' | 'draft_title' | 'number_label',
      caption: string,
      standard: string,
      hint = ''
    ) => html`
      <wa-input
        label="${caption}"
        placeholder="${standard}"
        hint="${hint}"
        .value="${wording[key]}"
        @input="${(e: Event) =>
          this.edit(s => ((offer ? s.offer.wording : s.invoice_wording)[key] = text(e)))}"
      ></wa-input>
    `;
    return html`
      <div class="grid">
        ${field(
          'title',
          'Überschrift',
          title,
          offer ? 'Leer: die Vorgabe, z. B. „Offerte“ schreiben.' : 'Leer: die Vorgabe.'
        )}
        ${field('draft_title', 'Überschrift im Entwurf', draft)}
        ${field('number_label', 'Bezeichnung der Nummer', number)}
      </div>
    `;
  }

  private offersPage(settings: Settings) {
    const offer = settings.offer;
    // none: the layout of the invoices, which then shows selected
    const offerLayout = offer.layout ?? settings.layout;
    return html`
      ${this.numberFields('offer', settings)}
      <wa-input
        label="Gültigkeit (Tage)"
        type="number"
        min="0"
        hint="Leer lassen, um keine Gültigkeit festzulegen."
        .value="${offer.validity_days === null ? '' : String(offer.validity_days)}"
        @input="${(e: Event) => this.edit(s => (s.offer.validity_days = parseDays(text(e))))}"
      ></wa-input>
      ${this.wordingFields('offer', settings)}
      <wa-textarea
        label="Einleitung"
        hint="Vor der Positionstabelle, z. B. Anrede und Anlass. Jedes Angebot kann einen eigenen Text haben."
        rows="3"
        .value="${offer.intro ?? ''}"
        @input="${(e: Event) => this.edit(s => (s.offer.intro = nullable(text(e))))}"
      ></wa-textarea>
      <wa-textarea
        label="Schlusstext"
        hint="Nach den Summen. {valid_until} schreibt das Datum, bis zu dem das Angebot gilt, eine Zeile mit dem Platzhalter entfällt ohne Gültigkeit."
        rows="3"
        .value="${offer.closing ?? ''}"
        @input="${(e: Event) => this.edit(s => (s.offer.closing = nullable(text(e))))}"
      ></wa-textarea>
      <wa-select
        label="Standard-Layout"
        hint="Neue Angebote dieses Kontexts beginnen damit, jedes kann ein anderes wählen."
        .value="${offerLayout}"
        @change="${(e: Event) =>
          this.edit(s => (s.offer.layout = String((e.target as WaSelect).value)))}"
      >
        ${this.layouts
          .filter(l => !l.archived || l.id === offerLayout)
          .map(l => html`<wa-option value="${l.id}">${l.name}</wa-option>`)}
      </wa-select>
    `;
  }

  private invoicesPage(settings: Settings) {
    return html`
      <wa-input
        label="Zahlungsziel (Tage)"
        type="number"
        min="0"
        hint="Leer: kein Fälligkeitsdatum, kein Zahlungshinweis"
        .value="${settings.payment_days === null ? '' : String(settings.payment_days)}"
        @input="${(e: Event) =>
          this.edit(s => {
            const days = text(e).trim();
            s.payment_days = days === '' ? null : Math.max(0, Math.trunc(Number(days)) || 0);
          })}"
      ></wa-input>
      ${
        settings.tax_rate.trim() === ''
          ? html`
              <wa-input
                label="Hinweis zur Steuerbefreiung"
                hint="Steht auf der Rechnung und in der XML-Datei, solange keine Umsatzsteuer berechnet wird."
                .value="${settings.vat_note}"
                @input="${(e: Event) => this.edit(s => (s.vat_note = text(e)))}"
              ></wa-input>
            `
          : nothing
      }
      ${this.numberFields('invoice', settings)} ${this.wordingFields('invoice', settings)}
      <wa-textarea
        label="Einleitung"
        hint="Vor der Positionstabelle, z. B. Anrede und Anlass. Jede Rechnung kann einen eigenen Text haben."
        rows="3"
        .value="${settings.intro ?? ''}"
        @input="${(e: Event) => this.edit(s => (s.intro = nullable(text(e))))}"
      ></wa-textarea>
      <wa-textarea
        label="Schlusstext"
        hint="Nach den Summen, z. B. Dank und Grußformel mit Namen."
        rows="3"
        .value="${settings.closing ?? ''}"
        @input="${(e: Event) => this.edit(s => (s.closing = nullable(text(e))))}"
      ></wa-textarea>
      <wa-select
        label="Standard-Layout"
        hint="Neue Rechnungen dieses Kontexts beginnen damit, jede kann ein anderes wählen. Individuelle Layouts legst du unter Layouts an."
        .value="${settings.layout}"
        @change="${(e: Event) => this.edit(s => (s.layout = String((e.target as WaSelect).value)))}"
      >
        ${this.layouts
          .filter(l => !l.archived || l.id === settings.layout)
          .map(l => html`<wa-option value="${l.id}">${l.name}</wa-option>`)}
      </wa-select>
      <wa-switch
        ?checked="${settings.offer_reference}"
        hint="Neue Rechnungen nennen die Nummer des Angebots, mit dem sie verknüpft sind. Jede Rechnung kann das ändern."
        @change="${(e: Event) => this.edit(s => (s.offer_reference = (e.target as WaSwitch).checked))}"
      >
        Angebotsnummer auf der Rechnung angeben
      </wa-switch>
    `;
  }

  /** What stops invoices from being issued under the context, and what only XRechnung needs */
  private missingNotes() {
    const {
      missing,
      missing_for_xrechnung: forXRechnung,
      missing_for_offers: forOffers,
    } = this.missing;
    return html`
      ${
        missing.length > 0
          ? html`
              <wa-callout variant="warning">
                Für Rechnungen fehlt noch: ${fieldNames(missing)}. Bis dahin sind Rechnungen,
                ${forOffers.length > 0 ? 'Angebote, ' : ''}Kunden und Positionen gesperrt.
              </wa-callout>
            `
          : nothing
      }
      ${
        forXRechnung.length > 0
          ? html`
              <wa-callout variant="neutral">
                Für XRechnung fehlt noch: ${fieldNames(forXRechnung)}.
              </wa-callout>
            `
          : nothing
      }
    `;
  }

  /** What can be done with a stored context besides editing it */
  private contextActions(context: Context) {
    const usage = this.contextUsage;
    const hasInvoices = usage === undefined || usage.drafts + usage.exported + usage.offers > 0;
    return html`
      <span slot="actions" id="remove-context">
        <wa-button
          variant="danger"
          appearance="outlined"
          ?disabled="${hasInvoices}"
          @click="${this.openRemove}"
        >
          Entfernen
        </wa-button>
      </span>
      ${
        hasInvoices
          ? html`
              <wa-tooltip slot="actions" for="remove-context">
                ${usage === undefined ? 'Wird geprüft…' : 'Mit Rechnungen oder Angeboten kann der Kontext nur archiviert werden.'}
              </wa-tooltip>
            `
          : nothing
      }
      <wa-button
        slot="actions"
        appearance="outlined"
        ?disabled="${context.archived_at !== null}"
        @click="${this.archiveContext}"
      >
        Archivieren
      </wa-button>
    `;
  }

  /** A new context can be dropped as long as there is another one to go back to */
  private cancelNewContext() {
    const back = activeContext.get()?.id;
    return back == null
      ? nothing
      : html`
          <wa-button
            slot="actions"
            appearance="outlined"
            @click="${() => goto(`/settings/context-${back}-company`)}"
          >
            Abbrechen
          </wa-button>
        `;
  }

  private contextPage(context: Context, page: Page) {
    if (page === 'rates') {
      // the rates are saved by their own dialogs, there is no form to save here
      return context.id === null
        ? html`
            <wa-callout slot="detail" variant="neutral">
              Ein neuer Kontext übernimmt die Steuersätze des aktiven. Nach dem Speichern lassen sie
              sich hier ändern.
            </wa-callout>
          `
        : html`
            <erng-rate-list
              slot="detail"
              context-id="${context.id}"
              default-rate="${contextById(context.id)?.settings.tax_rate ?? ''}"
              @rates-change="${() => void this.listsChanged()}"
            ></erng-rate-list>
          `;
    }
    if (page === 'units') {
      // the units are saved by their own dialogs, there is no form to save here
      return context.id === null
        ? html`
            <wa-callout slot="detail" variant="neutral">
              Ein neuer Kontext übernimmt die Einheiten des aktiven. Nach dem Speichern lassen sie
              sich hier ändern.
            </wa-callout>
          `
        : html`
            <erng-unit-list
              slot="detail"
              context-id="${context.id}"
              default-unit="${contextById(context.id)?.settings.unit ?? ''}"
              @units-change="${() => void this.listsChanged()}"
            ></erng-unit-list>
          `;
    }
    const settings = context.settings;
    const content = {
      company: () => this.companyPage(settings),
      bank: () => this.bankPage(),
      offers: () => this.offersPage(settings),
      invoices: () => this.invoicesPage(settings),
    }[page];
    const heading = PAGES.find(([key]) => key === page)?.[1];
    return html`
      <erng-page slot="detail" heading="${ifDefined(heading)}">
        ${this.missingNotes()} ${content()}
        ${context.id === null ? this.cancelNewContext() : this.contextActions(context)}
        <wa-button
          slot="primary"
          variant="brand"
          ?disabled="${!this.changed}"
          @click="${this.save}"
        >
          Speichern
        </wa-button>
      </erng-page>
    `;
  }

  /** The boxes of the sample follow a moved part, until the next rendering brings the real ones */
  private partMoved({ first, from, to }: { first: boolean; from: number; to: number }) {
    const sample = this.sample;
    if (!sample) {
      return;
    }
    this.sample = {
      ...sample,
      parts: sample.parts.map(box =>
        box.first === first ? { ...box, index: indexAfterMove(box.index, from, to) } : box
      ),
    };
  }

  /** The part the pointer is over, in the sample or in the form; the selected one needs no hover */
  private get hovered(): { first: boolean; index: number } | null {
    const over = this.overSample ?? this.overForm;
    const same = over && this.openPart?.first === over.first && this.openPart.index === over.index;
    return same ? null : over;
  }

  /** The sample invoice, rendered by the real template; its parts are for pointing at when there is a form for them */
  private sampleSheet(interactive = true) {
    // the first page shows what is set up for it, the second what the others look like
    const page = this.pointed && !this.pointed.first && this.editing?.first ? 1 : 0;
    const { openPart, hovered } = this;
    return html`
      <erng-sheet
        slot="preview"
        .page="${page}"
        .pages="${this.sample?.pages ?? []}"
        .regions="${interactive ? (this.sample?.parts ?? []) : []}"
        .reveal="${this.showInSample}"
        .active="${openPart ? regionKey(openPart) : null}"
        .hover="${hovered ? regionKey(hovered) : null}"
        @page-change="${(e: CustomEvent<number>) => {
          // the first page is the first setup, the others belong to the other: the tab follows
          this.pointed = { first: e.detail === 0, index: null };
        }}"
        @region-hover="${(e: CustomEvent<string | null>) => {
          this.overSample = e.detail ? parseKey(e.detail) : null;
        }}"
        @region-click="${(e: CustomEvent<string>) => {
          const part = parseKey(e.detail);
          this.pointed = { ...part };
          this.openPart = { ...part };
          this.reveal = { ...part, token: (this.reveal?.token ?? 0) + 1 };
        }}"
        hint="${this.sampleError ? `Vorschau nicht verfügbar: ${this.sampleError}` : (this.sample?.problems.map(p => p.message).join(', ') ?? 'Die Vorschau wird erzeugt…')}"
      ></erng-sheet>
    `;
  }

  /** The guides are for looking at layouts: they stay as they were left, for the next start too */
  private setGuides(on: boolean) {
    this.guides = on;
    remember('guides', on ? 1 : 0);
  }

  private guidesToggle() {
    return html`
      <wa-checkbox
        slot="form"
        ?checked="${this.guides}"
        hint="Zeichnet Linien an die Ränder des Inhalts und den druckbaren Bereich"
        @change="${(e: Event) => {
          this.setGuides((e.target as HTMLInputElement).checked);
          this.schedulePreview(0);
        }}"
      >
        Hilfslinien in der Vorschau
      </wa-checkbox>
    `;
  }

  private standardPage() {
    return html`
      <erng-page frameless slot="detail">
        <erng-editor position-key="editor:layout">
          <h2 slot="form">Standard</h2>
          <div slot="form" class="stack">
            <span>${this.layouts.find(l => l.id === 'standard')?.description}</span>
            <span class="hint">
              Das integrierte Layout lässt sich nicht ändern. Individuelle Layouts mit Rand,
              Elementen (Texte, Bilder, Briefkopf als Bild) und Schrift legst du mit „Neu“ an. Die
              Vorschau zeigt dieselben Beispieldaten wie bei jedem Layout.
            </span>
          </div>
          ${this.guidesToggle()} ${this.sampleSheet(false)}
        </erng-editor>
        <wa-button slot="actions" appearance="outlined" @click="${this.exportBuiltIn}">
          Exportieren
        </wa-button>
        ${
          this.defaultLayout === 'standard'
            ? html`<span slot="actions" class="muted">Dieses Layout ist das Standard-Layout.</span>`
            : html`
                <wa-button
                  slot="actions"
                  variant="brand"
                  @click="${() => this.makeDefault('standard')}"
                >
                  Als Standard-Layout festlegen
                </wa-button>
              `
        }
      </erng-page>
    `;
  }

  /** The built-in layout as a file, a starting point to change elsewhere */
  private async exportBuiltIn() {
    await this.exportLayout({ ...(await api.layoutBuiltin()), name: this.builtInName });
  }

  /** What keeps a layout from being deleted, one line each; none means it can go */
  private holdOn(layout: CustomLayout): string[] {
    if (this.usage === undefined) {
      return ['Die Verwendung wird geprüft…'];
    }
    return [
      ...(this.defaultLayout === `custom-${layout.id}` ? ['Standard-Layout'] : []),
      ...this.usage.map(
        u =>
          `${u.kind === 'offer' ? 'Angebot ' : ''}${u.number ?? 'Entwurf'} · ${u.customer || '–'}`
      ),
    ];
  }

  /** What a stored own layout can be done with, besides editing it */
  private layoutActions(layout: CustomLayout) {
    const key = `custom-${layout.id}`;
    const isDefault = this.defaultLayout === key;
    const reasons = this.holdOn(layout);
    return html`
      <span slot="actions" id="delete-layout">
        <wa-button
          variant="danger"
          appearance="outlined"
          ?disabled="${reasons.length > 0}"
          @click="${this.deleteLayout}"
        >
          Löschen
        </wa-button>
      </span>
      ${
        reasons.length > 0
          ? html`
              <wa-tooltip slot="actions" for="delete-layout">
                <strong>Kann nur archiviert werden, in Verwendung:</strong>
                ${reasons.map(reason => html`<div>${reason}</div>`)}
              </wa-tooltip>
            `
          : nothing
      }
      <wa-button slot="actions" appearance="outlined" @click="${() => this.exportLayout(layout)}">
        Exportieren
      </wa-button>
      <wa-button
        slot="actions"
        appearance="outlined"
        ?disabled="${isDefault && !layout.archived}"
        @click="${() => this.archiveLayout(!layout.archived)}"
      >
        ${layout.archived ? 'Wiederherstellen' : 'Archivieren'}
      </wa-button>
      ${
        isDefault || layout.archived
          ? nothing
          : html`
              <wa-button
                slot="actions"
                appearance="outlined"
                ?disabled="${this.layoutChanged}"
                @click="${() => this.makeDefault(key)}"
                >Als Standard-Layout festlegen</wa-button
              >
            `
      }
    `;
  }

  private layoutPage(layout: CustomLayout) {
    const isNew = layout.id === null;
    return html`
      <erng-page frameless slot="detail">
        <erng-editor position-key="editor:layout">
          ${
            layout.archived
              ? html`
                  <wa-callout slot="form" variant="neutral">
                    Archiviert: Das Layout steht bei Rechnungen nicht mehr zur Auswahl, bestehende
                    Rechnungen behalten es.
                  </wa-callout>
                `
              : nothing
          }
          <erng-scope-select
            slot="form"
            .value="${layout.context_id}"
            @scope-change="${(e: CustomEvent<number | null>) => {
              layout.context_id = e.detail;
              this.requestUpdate();
            }}"
          ></erng-scope-select>
          <erng-layout-form
            slot="form"
            .layout="${layout}"
            @change="${() => {
              this.requestUpdate();
              this.schedulePreview();
            }}"
            .guides="${this.guides}"
            .hover="${this.overSample}"
            .reveal="${this.reveal}"
            .tab="${this.pointed && !this.pointed.first ? 'page' : 'first'}"
            @guides-change="${(e: CustomEvent<boolean>) => {
              this.setGuides(e.detail);
              this.schedulePreview(0);
            }}"
            @part-focus="${(
              e: CustomEvent<{ first: boolean; hovered: number | null; opened: number | null }>
            ) => {
              const { first, hovered, opened } = e.detail;
              this.pointed = { first, index: hovered ?? opened };
              this.overForm = hovered === null ? null : { first, index: hovered };
              this.openPart = opened === null ? null : { first, index: opened };
            }}"
            @part-move="${(e: CustomEvent<{ first: boolean; from: number; to: number }>) =>
              this.partMoved(e.detail)}"
            @part-open="${(e: CustomEvent<{ first: boolean; index: number }>) => {
              this.showInSample = {
                key: regionKey(e.detail),
                token: (this.showInSample?.token ?? 0) + 1,
              };
            }}"
          ></erng-layout-form>
          ${this.sampleSheet()}
        </erng-editor>
        ${
          isNew
            ? html`
                <wa-button
                  slot="actions"
                  appearance="outlined"
                  @click="${() => goto(`/settings/${LAYOUT}standard`)}"
                >
                  Abbrechen
                </wa-button>
              `
            : this.layoutActions(layout)
        }
        <wa-button
          slot="primary"
          variant="brand"
          ?disabled="${isNew ? layout.name.trim() === '' : !this.layoutChanged}"
          @click="${this.saveLayout}"
        >
          Speichern
        </wa-button>
      </erng-page>
    `;
  }

  private renderDetail() {
    const route = this.contextRoute;
    if (route) {
      return this.context
        ? this.contextPage(this.context, route.page)
        : html`<wa-callout slot="detail" variant="neutral">Kontext nicht gefunden.</wa-callout>`;
    }
    if (this.section === `${LAYOUT}standard`) {
      return this.standardPage();
    }
    if (this.layoutId !== undefined) {
      return this.editing
        ? this.layoutPage(this.editing)
        : html`<wa-callout slot="detail" variant="neutral">Layout nicht gefunden.</wa-callout>`;
    }
    return html`<wa-callout slot="detail" variant="neutral">Seite nicht gefunden.</wa-callout>`;
  }

  override render() {
    if (!contexts.get()) {
      return nothing;
    }
    return html`
      <erng-split position-key="split:settings"
        >${this.renderList()} ${this.renderDetail()}</erng-split
      >
      ${this.importDialog()} ${this.removeDialog()} ${this.counterDialog()}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-settings': SettingsView;
  }
}
