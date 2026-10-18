import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';
import '@awesome.me/webawesome/dist/components/tooltip/tooltip.js';
import '@awesome.me/webawesome/dist/components/textarea/textarea.js';
import '../../common/components/address-fields/address-fields.component.js';
import '../../common/components/layout-form/layout-form.component.js';
import '../../common/components/list/list/list.component.js';
import '../../common/components/list/list-group/list-group.component.js';
import '../../common/components/list/list-item/list-item.component.js';
import '../../common/components/page/page.component.js';
import '../../common/components/sheet/sheet.component.js';
import '../../common/layouts/editor/editor.layout.js';
import '../../common/layouts/split/split.layout.js';

import WaInput from '@awesome.me/webawesome/dist/components/input/input.js';
import { open } from '@tauri-apps/plugin-dialog';
import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type {
  CustomLayout,
  LayoutInfo,
  LayoutUsage,
  Preview,
  Settings,
} from '../../common/utils/api.utils.js';
import { api, errorText } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import { nullable, text } from '../../common/utils/form.utils.js';
import { notify, notifyError } from '../../common/utils/notify.utils.js';
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

const blankLayout = (): CustomLayout => ({
  id: null,
  name: '',
  background: null,
  margins: { top: 20, right: 20, bottom: 25, left: 25 },
  first_margins: null,
  fonts: [],
  archived: false,
});

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

  /** The settings as they are stored, what "unchanged" is compared against */
  #saved = '';
  /** The own layout as it is stored */
  #layoutSaved = '';
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

  override connectedCallback() {
    super.connectedCallback();
    void this.loadLayouts();
    api.settingsGet().then(
      settings => {
        this.#saved = JSON.stringify(settings);
        this.settings = settings;
      },
      (error: unknown) => notifyError(error)
    );
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
    const layout = id === null ? blankLayout() : this.customs.find(c => c.id === id);
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
  private schedulePreview(delay = 450) {
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

  private async chooseLogo() {
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Bild', extensions: ['png', 'jpg', 'jpeg', 'svg'] }],
    });
    if (typeof picked === 'string') {
      this.edit(s => (s.logo = picked));
    }
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
      this.#layoutSaved = '';
      await this.loadLayouts();
      goto(`/settings/${LAYOUT}${saved.id}`);
    });
  }

  private archiveLayout(archived: boolean) {
    return this.layoutAction(async () => {
      const layout = this.editing;
      if (!layout) {
        return;
      }
      await api.layoutSave({ ...layout, archived });
      this.#layoutSaved = '';
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
          <wa-button
            slot="action"
            size="s"
            appearance="outlined"
            ?disabled="${this.section === `${LAYOUT}new`}"
            @click="${() => goto(`/settings/${LAYOUT}new`)}"
          >
            Neu
          </wa-button>
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
      <div class="file">
        <wa-button @click="${this.chooseLogo}" appearance="outlined">Logo wählen</wa-button>
        ${
          settings.logo
            ? html`<span class="muted">${settings.logo.split('/').pop()}</span>
                <wa-button appearance="outlined" @click="${() => this.edit(s => (s.logo = null))}"
                  >Entfernen</wa-button
                >`
            : nothing
        }
      </div>
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

  /** The sample invoice, rendered by the real template */
  private sampleSheet() {
    return html`<erng-sheet
      slot="preview"
      .pages="${this.sample?.pages ?? []}"
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
              Das eingebaute Layout lässt sich nicht ändern. Eigene Layouts mit Briefpapier, Rand
              und Schrift legst du mit „Neu“ an. Die Vorschau zeigt dieselben Beispieldaten wie bei
              jedem Layout.
            </span>
          </div>
          ${this.guidesToggle()} ${this.sampleSheet()}
        </erng-editor>
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
          <h2 slot="form">${isNew ? 'Neues Layout' : layout.name || 'Layout'}</h2>
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
          ></erng-layout-form>
          ${this.guidesToggle()} ${this.sampleSheet()}
        </erng-editor>
        ${isNew ? nothing : this.layoutActions(layout)}
        <wa-button
          slot="primary"
          variant="brand"
          ?disabled="${!isNew && !this.layoutChanged}"
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
    return html`<erng-split>${this.renderList()} ${this.renderDetail(settings)}</erng-split>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-settings': SettingsView;
  }
}
