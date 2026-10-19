import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';
import '@awesome.me/webawesome/dist/components/tooltip/tooltip.js';
import '@awesome.me/webawesome/dist/components/textarea/textarea.js';
import '../../common/components/address-fields/address-fields.component.js';
import '../../common/components/layout-form/layout-form.component.js';
import '../../common/components/list/list/list.component.js';
import '../../common/components/list/list-group/list-group.component.js';
import '../../common/components/list/list-item/list-item.component.js';
import '../../common/components/page/page.component.js';
import '../../common/layouts/editor/editor.layout.js';
import '../../common/layouts/split/split.layout.js';

import WaInput from '@awesome.me/webawesome/dist/components/input/input.js';
import { ask, open, save } from '@tauri-apps/plugin-dialog';
import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import { regionKey } from '../../common/components/sheet/sheet.component.js';
import type {
  CustomLayout,
  ImportChoice,
  ImportOutcome,
  LayoutInfo,
  LayoutUsage,
  Preview,
  Settings,
} from '../../common/utils/api.utils.js';
import { api, errorText } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import { nullable, text } from '../../common/utils/form.utils.js';
import { notify, notifyError } from '../../common/utils/notify.utils.js';
import { takeOpenedLayouts } from '../../common/utils/opened.utils.js';
import { goto } from '../../common/utils/routing.utils.js';
import styles from './settings.view.css';

/** The pages that edit the settings, saved together */
const SECTIONS = [
  ['company', 'Unternehmen'],
  ['bank', 'Bankverbindung'],
  ['invoices', 'Rechnungen'],
  ['texts', 'Texte der Rechnung'],
] as const;

const LAYOUT = 'layout-';

/** The ending of a layout file */
const LAYOUT_EXTENSION = 'erng-layout';

/** The setup and the place of a part in it, from what `regionKey` made */
const parseKey = (key: string): { first: boolean; index: number } => {
  const [setup, index] = key.split(':');
  return { first: setup === 'first', index: Number(index) };
};

/**
 * Settings: the pages on the left (company, bank, invoices, texts, then the layouts), the selected
 * one on the right. The company pages share one Save button, a layout is saved on its own.
 */
@customElement('erng-settings')
export class SettingsView extends ErngElement.withStyles(styles) {
  /** From the address: a section, `layout-standard`, `layout-<id>`, `layout-new`; empty is the first */
  @property() selected = '';

  @state() private settings: Settings | undefined;
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
  @state() private guides = false;
  /** What the user points at in the form: a page setup, and a part of it */
  @state() private pointed: { first: boolean; index: number | null } | undefined;
  /** The part under the pointer in the sample */
  @state() private overSample: { first: boolean; index: number } | null = null;
  /** A part to bring into view in the sample, because it was opened in the form */
  @state() private showInSample: { key: string; token: number } | undefined;
  /** A part to open in the form, asked for by a click in the sample */
  @state() private reveal: { first: boolean; index: number; token: number } | undefined;

  /** The settings as they are stored, what "unchanged" is compared against */
  #saved = '';
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
    return JSON.stringify(this.settings) !== this.#saved;
  }

  private get layoutChanged() {
    return this.editing !== undefined && JSON.stringify(this.editing) !== this.#layoutSaved;
  }

  private get section() {
    return this.selected === '' ? 'company' : this.selected;
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

  private get defaultLayout() {
    return this.settings?.layout ?? 'standard';
  }

  #stopOpened: (() => void) | undefined;

  override connectedCallback() {
    super.connectedCallback();
    // files the system opened with the app come here, to be imported like picked ones
    this.#stopOpened = takeOpenedLayouts(path =>
      this.layoutAction(async () => this.importFrom(path, {}))
    );
    void this.loadLayouts();
    api.settingsGet().then(
      settings => {
        this.#saved = JSON.stringify(settings);
        this.settings = settings;
      },
      (error: unknown) => notifyError(error)
    );
  }

  override disconnectedCallback() {
    super.disconnectedCallback();
    this.#stopOpened?.();
  }

  protected override updated(changed: PropertyValues<this>) {
    if (changed.has('selected')) {
      void this.openLayout();
    }
  }

  private async loadLayouts() {
    try {
      [this.layouts, this.customs] = await Promise.all([api.layoutsList(), api.layoutsCustom()]);
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
      const sample = await api.layoutPreview(layout, this.guides);
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
    if (!this.settings) {
      return;
    }
    change(this.settings);
    this.requestUpdate();
  }

  private seller(
    key: 'name' | 'contact_name' | 'phone' | 'email' | 'iban',
    label: string,
    type = 'text'
  ) {
    const settings = this.settings as Settings;
    return html`<wa-input
      label="${label}"
      type="${type as WaInput['type']}"
      .value="${settings.seller[key]}"
      @input="${(e: Event) => this.edit(s => (s.seller[key] = text(e)))}"
    ></wa-input>`;
  }

  private optional(key: 'vat_id' | 'tax_number' | 'bic' | 'account_holder', label: string) {
    const settings = this.settings as Settings;
    return html`<wa-input
      label="${label}"
      .value="${settings.seller[key] ?? ''}"
      @input="${(e: Event) => this.edit(s => (s.seller[key] = nullable(text(e))))}"
    ></wa-input>`;
  }

  private async save() {
    const settings = this.settings;
    if (!settings) {
      return;
    }
    const rate = Number(settings.tax_rate.replace(',', '.'));
    if (!Number.isFinite(rate) || rate <= 0 || rate > 100) {
      notify('Der Steuersatz muss zwischen 0 und 100 liegen (in Prozent).', 'danger');
      return;
    }
    try {
      await api.settingsSave(settings);
      this.#saved = JSON.stringify(settings);
      notify('Gespeichert. Bereits exportierte Rechnungen behalten ihre alten Angaben.');
    } catch (error) {
      notifyError(error);
    }
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
    return html`<wa-dialog
      ?open="${pending !== undefined}"
      label="${question?.kind === 'replace' ? 'Layout ersetzen?' : 'Name vergeben'}"
      @wa-after-hide="${(e: Event) => e.target === e.currentTarget && this.answerImport()}"
    >
      ${
        question?.kind === 'replace'
          ? html`<div class="stack">
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
                  ? html`<span class="muted">
                      ${question.existing.is_default ? 'Es ist das Standard-Layout. ' : ''}
                      ${
                        question.existing.uses > 0
                          ? `${question.existing.uses} Rechnungen und Entwürfe verwenden es, bereits exportierte behalten ihr Aussehen.`
                          : ''
                      }
                    </span>`
                  : nothing
              }
            </div>`
          : question?.kind === 'name-taken'
            ? html`<div class="stack">
                <span>
                  Ein anderes Layout heißt schon „${question.name}“. Unter welchem Namen soll das
                  importierte gespeichert werden?
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
              </div>`
            : nothing
      }
      <wa-button slot="footer" appearance="outlined" @click="${() => this.answerImport()}">
        Abbrechen
      </wa-button>
      ${
        question?.kind === 'replace'
          ? html`<wa-button
              slot="footer"
              variant="${question.incoming_updated_at < question.existing.updated_at ? 'danger' : 'brand'}"
              @click="${() => this.answerImport({ replace: true })}"
            >
              Ersetzen
            </wa-button>`
          : html`<wa-button
              slot="footer"
              variant="brand"
              ?disabled="${(pending?.name ?? '').trim() === ''}"
              @click="${() => this.answerImport({ name: pending?.name })}"
            >
              Importieren
            </wa-button>`
      }
    </wa-dialog>`;
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
      if (!this.settings) {
        return;
      }
      const stored = JSON.parse(this.#saved) as Settings;
      stored.layout = key;
      await api.settingsSave(stored);
      this.#saved = JSON.stringify(stored);
      this.edit(s => (s.layout = key));
    });
  }

  // render

  private item(key: string, label: string, sub: string, tag: unknown = nothing, archived = false) {
    return html`
      <erng-list-item
        ?selected="${this.section === key}"
        ?archived="${archived}"
        @click="${() => goto(`/settings/${key}`)}"
      >
        ${label}
        <span slot="end">${tag}</span>
        ${sub ? html`<span slot="sub">${sub}</span>` : nothing}
      </erng-list-item>
    `;
  }

  private renderList() {
    const isDefault = (key: string) =>
      key === this.defaultLayout
        ? html`<wa-tag size="s" variant="brand">Standard</wa-tag>`
        : nothing;
    return html`
      <erng-list slot="list">
        ${SECTIONS.map(([key, label]) => this.item(key, label, ''))}
        <erng-list-group heading="Layouts">
          <span slot="action" class="row">
            <wa-button
              size="s"
              appearance="outlined"
              ?disabled="${this.section === `${LAYOUT}new`}"
              @click="${this.importLayout}"
            >
              Import
            </wa-button>
            <wa-button
              size="s"
              appearance="outlined"
              ?disabled="${this.section === `${LAYOUT}new`}"
              @click="${() => goto(`/settings/${LAYOUT}new`)}"
            >
              Neu
            </wa-button>
          </span>
          ${this.item(`${LAYOUT}standard`, this.builtInName, 'Eingebaut', isDefault('standard'))}
          ${this.customs.map(c =>
            this.item(
              `${LAYOUT}${c.id}`,
              c.name,
              c.archived ? 'Archiviert' : 'Eigenes Layout',
              isDefault(`custom-${c.id}`),
              c.archived
            )
          )}
        </erng-list-group>
      </erng-list>
    `;
  }

  private companyPage(settings: Settings) {
    return html`
      ${this.seller('name', 'Name des Unternehmens')}
      <erng-address-fields
        .address="${settings.seller.address}"
        @change="${() => this.edit(() => undefined)}"
      ></erng-address-fields>
      <div class="grid">
        ${this.optional('vat_id', 'USt-IdNr.')} ${this.optional('tax_number', 'Steuernummer')}
      </div>
      <div class="grid">
        ${this.seller('contact_name', 'Ansprechpartner')} ${this.seller('phone', 'Telefon', 'tel')}
        ${this.seller('email', 'E-Mail', 'email')}
      </div>
      <wa-textarea
        label="Zusatzangaben im Fuß (Handelsregister, Geschäftsführer, …)"
        rows="2"
        .value="${settings.footer ?? ''}"
        @input="${(e: Event) => this.edit(s => (s.footer = nullable(text(e))))}"
      ></wa-textarea>
    `;
  }

  private bankPage() {
    return html`
      <div class="grid">
        ${this.seller('iban', 'IBAN')} ${this.optional('bic', 'BIC')}
        ${this.optional('account_holder', 'Kontoinhaber')}
      </div>
    `;
  }

  private invoicesPage(settings: Settings) {
    return html`
      <div class="grid">
        <wa-input
          label="Umsatzsteuersatz (%)"
          inputmode="decimal"
          .value="${settings.tax_rate}"
          @input="${(e: Event) => this.edit(s => (s.tax_rate = text(e)))}"
        ></wa-input>
        <wa-input
          label="Zahlungsziel (Tage)"
          type="number"
          min="0"
          .value="${String(settings.payment_days)}"
          @input="${(e: Event) => this.edit(s => (s.payment_days = Math.max(0, Number(text(e)) || 0)))}"
        ></wa-input>
        <wa-input
          label="Präfix der Rechnungsnummer"
          hint="z. B. RE- ergibt RE-2026-001"
          .value="${settings.number_prefix}"
          @input="${(e: Event) => this.edit(s => (s.number_prefix = text(e)))}"
        ></wa-input>
      </div>
      <span class="muted">
        Das Layout der Rechnungen wählst du unter Layouts, dort legst du auch das Standard-Layout
        fest.
      </span>
    `;
  }

  private textsPage(settings: Settings) {
    return html`
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
    `;
  }

  private settingsPage(settings: Settings) {
    const pages = {
      company: ['Unternehmen', () => this.companyPage(settings)],
      bank: ['Bankverbindung', () => this.bankPage()],
      invoices: ['Rechnungen', () => this.invoicesPage(settings)],
      texts: ['Texte der Rechnung', () => this.textsPage(settings)],
    } satisfies Record<string, [string, () => unknown]>;
    const key = (this.section in pages ? this.section : 'company') as keyof typeof pages;
    const [heading, content] = pages[key];
    return html`
      <erng-page slot="detail" heading="${heading}">
        ${content()}
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

  /** The part marked in the form and in the sample: under the pointer there, or pointed at here */
  private get active(): { first: boolean; index: number } | null {
    const here = this.pointed;
    return (
      this.overSample ?? (here?.index == null ? null : { first: here.first, index: here.index })
    );
  }

  /** The sample invoice, rendered by the real template */
  private sampleSheet() {
    // the first page shows what is set up for it, the second what the others look like
    const page = this.pointed && !this.pointed.first && this.editing?.first ? 1 : 0;
    const active = this.active;
    return html`<erng-sheet
      slot="preview"
      .page="${page}"
      .pages="${this.sample?.pages ?? []}"
      .regions="${this.sample?.parts ?? []}"
      .reveal="${this.showInSample}"
      .active="${active ? regionKey({ first: active.first, index: active.index }) : null}"
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
        this.reveal = { ...part, token: (this.reveal?.token ?? 0) + 1 };
      }}"
      hint="${this.sampleError ? `Vorschau nicht verfügbar: ${this.sampleError}` : (this.sample?.problems.map(p => p.message).join(', ') ?? 'Die Vorschau wird erzeugt…')}"
    ></erng-sheet>`;
  }

  private guidesToggle() {
    return html`<wa-checkbox
      slot="form"
      ?checked="${this.guides}"
      hint="Zeichnet Linien an die Ränder des Inhalts"
      @change="${(e: Event) => {
        this.guides = (e.target as HTMLInputElement).checked;
        this.schedulePreview(0);
      }}"
    >
      Hilfslinien in der Vorschau
    </wa-checkbox>`;
  }

  private standardPage() {
    return html`
      <erng-page frameless slot="detail">
        <erng-editor>
          <h2 slot="form">Standard</h2>
          <div slot="form" class="stack">
            <span>${this.layouts.find(l => l.id === 'standard')?.description}</span>
            <span class="muted">
              Das eingebaute Layout lässt sich nicht ändern. Eigene Layouts mit Rand, Teilen (Texte,
              Bilder, Briefkopf als Bild) und Schrift legst du mit „Neu“ an. Die Vorschau zeigt
              dieselben Beispieldaten wie bei jedem Layout.
            </span>
          </div>
          ${this.guidesToggle()} ${this.sampleSheet()}
        </erng-editor>
        <wa-button slot="actions" appearance="outlined" @click="${this.exportBuiltIn}">
          Exportieren
        </wa-button>
        ${
          this.defaultLayout === 'standard'
            ? html`<span slot="actions" class="muted">Dieses Layout ist das Standard-Layout.</span>`
            : html`<wa-button
                slot="actions"
                variant="brand"
                @click="${() => this.makeDefault('standard')}"
              >
                Als Standard-Layout festlegen
              </wa-button>`
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
      ...this.usage.map(u => `${u.number ?? 'Entwurf'} · ${u.customer || '–'}`),
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
          ? html`<wa-tooltip slot="actions" for="delete-layout">
              <strong>Kann nur archiviert werden, in Verwendung:</strong>
              ${reasons.map(reason => html`<div>${reason}</div>`)}
            </wa-tooltip>`
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
          : html`<wa-button
              slot="actions"
              appearance="outlined"
              ?disabled="${this.layoutChanged}"
              @click="${() => this.makeDefault(key)}"
              >Als Standard-Layout festlegen</wa-button
            >`
      }
    `;
  }

  private layoutPage(layout: CustomLayout) {
    const isNew = layout.id === null;
    return html`
      <erng-page frameless slot="detail">
        <erng-editor>
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
          <erng-layout-form
            slot="form"
            .layout="${layout}"
            @change="${() => {
              this.requestUpdate();
              this.schedulePreview();
            }}"
            .guides="${this.guides}"
            .active="${this.active}"
            .reveal="${this.reveal}"
            .tab="${this.pointed && !this.pointed.first ? 'page' : 'first'}"
            @guides-change="${(e: CustomEvent<boolean>) => {
              this.guides = e.detail;
              this.schedulePreview(0);
            }}"
            @part-focus="${(e: CustomEvent<{ first: boolean; index: number | null }>) => {
              this.pointed = e.detail;
            }}"
            @part-open="${(e: CustomEvent<{ first: boolean; index: number }>) => {
              this.showInSample = {
                key: regionKey(e.detail),
                token: (this.showInSample?.token ?? 0) + 1,
              };
            }}"
          ></erng-layout-form>
          ${this.sampleSheet()}
        </erng-editor>
        ${isNew ? nothing : this.layoutActions(layout)}
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

  private renderDetail(settings: Settings) {
    if (this.section === `${LAYOUT}standard`) {
      return this.standardPage();
    }
    if (this.layoutId !== undefined) {
      return this.editing
        ? this.layoutPage(this.editing)
        : html`<wa-callout slot="detail" variant="neutral">Layout nicht gefunden.</wa-callout>`;
    }
    return this.settingsPage(settings);
  }

  override render() {
    const settings = this.settings;
    if (!settings) {
      return nothing;
    }
    return html`<erng-split>${this.renderList()} ${this.renderDetail(settings)}</erng-split>
      ${this.importDialog()}`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-settings': SettingsView;
  }
}
