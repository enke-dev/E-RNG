import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/card/card.js';
import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/spinner/spinner.js';
import '@awesome.me/webawesome/dist/components/switch/switch.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';
import '@awesome.me/webawesome/dist/components/textarea/textarea.js';
import '../../common/components/customer-form/customer-form.component.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import type WaSwitch from '@awesome.me/webawesome/dist/components/switch/switch.js';
import { save } from '@tauri-apps/plugin-dialog';
import type { CSSResultGroup, PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type {
  Check,
  Customer,
  InvoiceDraft,
  Position,
  ProduceError,
  ProduceResult,
  Settings,
  StoredInvoice,
} from '../../common/utils/api.utils.js';
import { api, errorText } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import { nullable, text } from '../../common/utils/form.utils.js';
import { date, money, UNITS } from '../../common/utils/format.utils.js';
import { goto } from '../../common/utils/routing.utils.js';
import styles from './invoice-edit.view.css';

/** What the export dialog is doing */
type Export =
  | { phase: 'busy' }
  | { phase: 'failed'; error: ProduceError | string }
  | {
      phase: 'ready';
      result: ProduceResult;
      pdf: boolean;
      xml: boolean;
      saving: boolean;
      error?: string;
    };

const blankCustomer = (): Customer => ({
  id: null,
  name: '',
  address: { street: '', post_code: '', city: '', country: 'DE' },
  vat_id: null,
  email: null,
  buyer_reference: null,
});

@customElement('erng-invoice-edit')
export class InvoiceEditView extends ErngElement {
  static override styles: CSSResultGroup = [ErngElement.styles, styles];

  /** `new` or the id of a stored invoice */
  @property({ attribute: 'invoice-id' }) invoiceId = 'new';

  @state() private stored: StoredInvoice | undefined;
  @state() private draft: InvoiceDraft | undefined;
  @state() private settings: Settings | undefined;
  @state() private customers: Customer[] = [];
  @state() private check: Check | undefined;
  @state() private error = '';
  @state() private notice = '';
  @state() private busy = false;
  @state() private newCustomer: Customer | undefined;
  @state() private exporting: Export | undefined;

  #timer: ReturnType<typeof setTimeout> | undefined;

  protected override willUpdate(changed: PropertyValues<this>) {
    if (changed.has('invoiceId')) {
      void this.load();
    }
  }

  override disconnectedCallback() {
    super.disconnectedCallback();
    clearTimeout(this.#timer);
  }

  private get locked() {
    return this.stored?.status === 'exported';
  }

  private async load() {
    this.error = '';
    try {
      const [settings, customers] = await Promise.all([api.settingsGet(), api.customersList()]);
      this.settings = settings;
      this.customers = customers;
      if (this.invoiceId === 'new') {
        this.stored = undefined;
        this.draft = await api.invoiceBlank();
      } else {
        this.stored = await api.invoiceGet(Number(this.invoiceId));
        this.draft = this.stored.draft;
      }
      await this.runCheck();
    } catch (error) {
      this.error = errorText(error);
    }
  }

  private async runCheck() {
    if (this.draft) {
      this.check = await api.invoiceCheck(this.draft);
    }
  }

  /** Call after every edit: re-renders now, asks the backend for totals and findings shortly after */
  private changed() {
    this.requestUpdate();
    this.notice = '';
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => void this.runCheck().catch(() => undefined), 300);
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
      this.error = 'Bitte einen Namen für den Kunden eingeben.';
      return;
    }
    try {
      const saved = await api.customerSave(customer);
      this.customers = await api.customersList();
      this.newCustomer = undefined;
      this.error = '';
      this.selectCustomer(String(saved.id));
    } catch (error) {
      this.error = errorText(error);
    }
  }

  private async persist(): Promise<StoredInvoice | undefined> {
    if (!this.draft || this.locked) {
      return this.stored;
    }
    this.stored = await api.invoiceSave(this.stored?.id ?? null, this.draft);
    // the first save gives the new invoice its address, without reloading the form
    if (this.invoiceId === 'new') {
      history.replaceState({}, '', `/invoices/${this.stored.id}`);
    }
    return this.stored;
  }

  private async saveDraft() {
    this.busy = true;
    try {
      await this.persist();
      this.notice = 'Entwurf gespeichert.';
      this.error = '';
    } catch (error) {
      this.error = errorText(error);
    } finally {
      this.busy = false;
    }
  }

  private async duplicate() {
    if (!this.stored) {
      return;
    }
    try {
      goto(`/invoices/${(await api.invoiceDuplicate(this.stored.id)).id}`);
    } catch (error) {
      this.error = errorText(error);
    }
  }

  private async deleteDraft() {
    if (!this.stored) {
      return;
    }
    try {
      await api.invoiceDelete(this.stored.id);
      goto('/');
    } catch (error) {
      this.error = errorText(error);
    }
  }

  private async startExport() {
    this.exporting = { phase: 'busy' };
    try {
      const stored = await this.persist();
      if (!stored) {
        throw new Error('Die Rechnung konnte nicht gespeichert werden.');
      }
      const result = await api.produce(stored.id);
      this.exporting = {
        phase: 'ready',
        result,
        pdf: true,
        xml: this.draft?.profile === 'xrechnung',
        saving: false,
      };
    } catch (error) {
      this.exporting = { phase: 'failed', error: isProduceError(error) ? error : errorText(error) };
    }
  }

  private async closeExport() {
    const exporting = this.exporting;
    this.exporting = undefined;
    if (exporting?.phase === 'ready') {
      await api.discardWorkDir(exporting.result.work_dir).catch(() => undefined);
    }
  }

  private async saveFiles() {
    const exporting = this.exporting;
    const stored = this.stored;
    if (exporting?.phase !== 'ready' || !stored) {
      return;
    }
    const { result } = exporting;
    if (!exporting.pdf && !exporting.xml) {
      exporting.error = 'Bitte mindestens eine Datei wählen.';
      this.requestUpdate();
      return;
    }

    exporting.error = undefined;
    exporting.saving = true;
    this.requestUpdate();
    try {
      const pdf = exporting.pdf
        ? await save({
            defaultPath: `${result.number}.pdf`,
            filters: [{ name: 'PDF', extensions: ['pdf'] }],
          })
        : null;
      const xml = exporting.xml
        ? await save({
            defaultPath: `${result.number}.xml`,
            filters: [{ name: 'XML', extensions: ['xml'] }],
          })
        : null;
      // cancelling a dialog means "not now", nothing is written and the invoice stays a draft
      if ((exporting.pdf && !pdf) || (exporting.xml && !xml)) {
        exporting.saving = false;
        this.requestUpdate();
        return;
      }
      this.stored = await api.exportInvoice(stored.id, result.work_dir, pdf, xml);
      this.draft = this.stored.draft;
      this.exporting = undefined;
      this.notice = `Gespeichert als ${this.stored.number}. Die Rechnung ist jetzt gesperrt.`;
      await this.runCheck();
    } catch (error) {
      exporting.saving = false;
      exporting.error = errorText(error);
      this.requestUpdate();
    }
  }

  // rendering

  private renderPositions(locked: boolean) {
    const draft = this.draft as InvoiceDraft;
    return html`
      <div class="positions">
        <span class="head">Beschreibung</span><span class="head">Menge</span
        ><span class="head">Einheit</span> <span class="head">Preis (netto)</span><span></span>
        ${draft.positions.map(
          (p, i) => html`
            <wa-input
              aria-label="Beschreibung"
              ?disabled="${locked}"
              .value="${p.description}"
              @input="${(e: Event) => this.editPosition(i, pos => (pos.description = text(e)))}"
            ></wa-input>
            <wa-input
              aria-label="Menge"
              placeholder="1"
              inputmode="decimal"
              ?disabled="${locked}"
              .value="${p.quantity ?? ''}"
              @input="${(e: Event) => this.editPosition(i, pos => (pos.quantity = nullable(text(e))))}"
            ></wa-input>
            <wa-select
              aria-label="Einheit"
              ?disabled="${locked}"
              .value="${p.unit ?? 'C62'}"
              @change="${(e: Event) =>
                this.editPosition(
                  i,
                  pos =>
                    (pos.unit =
                      (e.target as WaSelect).value === 'C62'
                        ? null
                        : String((e.target as WaSelect).value))
                )}"
            >
              ${UNITS.map(u => html`<wa-option value="${u.code}">${u.label}</wa-option>`)}
            </wa-select>
            <wa-input
              aria-label="Preis"
              inputmode="decimal"
              placeholder="0,00"
              ?disabled="${locked}"
              .value="${p.price}"
              @input="${(e: Event) => this.editPosition(i, pos => (pos.price = text(e)))}"
            ></wa-input>
            <wa-button
              appearance="plain"
              variant="danger"
              aria-label="Position entfernen"
              ?disabled="${locked || draft.positions.length === 1}"
              @click="${() => this.edit(d => d.positions.splice(i, 1))}"
              >Entfernen</wa-button
            >
          `
        )}
      </div>
      ${
        locked
          ? nothing
          : html`<div>
              <wa-button
                @click="${() => this.edit(d => d.positions.push({ description: '', quantity: null, unit: null, price: '' }))}"
                >Position hinzufügen</wa-button
              >
            </div>`
      }
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
          ? html`<wa-callout variant="warning">
              Noch unvollständig:
              <ul>
                ${check.problems.map(p => html`<li>${p.message}</li>`)}
              </ul>
            </wa-callout>`
          : nothing
      }
      ${
        fatal.length > 0
          ? html`<wa-callout variant="danger">
              Regelverstöße, die den Export verhindern:
              <ul>
                ${fatal.map(f => html`<li><strong>${f.rule}</strong> ${f.message}</li>`)}
              </ul>
            </wa-callout>`
          : nothing
      }
      ${
        advice.length > 0
          ? html`<wa-callout variant="neutral">
              Hinweise:
              <ul>
                ${advice.map(f => html`<li><strong>${f.rule}</strong> ${f.message}</li>`)}
              </ul>
            </wa-callout>`
          : nothing
      }
    `;
  }

  private renderSummary() {
    const totals = this.check?.totals;
    const rate = this.draft?.frozen?.tax_rate ?? this.settings?.tax_rate ?? '';
    return html`
      <div class="summary">
        <span>Netto</span><span class="value">${money(totals?.net)}</span>
        <span>Umsatzsteuer ${rate} %</span><span class="value">${money(totals?.vat)}</span>
        <span class="total">Gesamtbetrag</span
        ><span class="value total">${money(totals?.gross)}</span>
      </div>
    `;
  }

  private renderExport() {
    const exporting = this.exporting;
    return html`
      <wa-dialog
        ?open="${exporting !== undefined}"
        label="Rechnung erstellen und prüfen"
        light-dismiss
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && void this.closeExport()}"
      >
        ${
          exporting?.phase === 'busy'
            ? html`<div class="row"><wa-spinner></wa-spinner> Wird erstellt und geprüft …</div>`
            : nothing
        }
        ${exporting?.phase === 'failed' ? this.renderFailure(exporting.error) : nothing}
        ${exporting?.phase === 'ready' ? this.renderReady(exporting) : nothing}
        <wa-button slot="footer" appearance="plain" @click="${() => void this.closeExport()}">
          ${exporting?.phase === 'ready' && exporting.result.report.valid ? 'Abbrechen' : 'Schließen'}
        </wa-button>
        ${
          exporting?.phase === 'ready' && exporting.result.report.valid
            ? html`<wa-button
                slot="footer"
                variant="brand"
                ?loading="${exporting.saving}"
                @click="${() => void this.saveFiles()}"
                >Speichern unter …</wa-button
              >`
            : nothing
        }
      </wa-dialog>
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
      return html`<wa-callout variant="warning"
        >Die Angaben sind unvollständig:
        <ul>
          ${error.problems.map(p => html`<li>${p.message}</li>`)}
        </ul></wa-callout
      >`;
    }
    return html`<wa-callout variant="danger"
      >Die Rechnung verstößt gegen Regeln:
      <ul>
        ${error.findings.map(f => html`<li><strong>${f.rule}</strong> ${f.message}</li>`)}
      </ul></wa-callout
    >`;
  }

  private renderReady(exporting: Extract<Export, { phase: 'ready' }>) {
    const { result } = exporting;
    if (!result.report.valid) {
      const errors = result.report.issues.filter(i => i.level === 'error');
      return html`<wa-callout variant="danger">
        Die erzeugte Datei hat die Prüfung nicht bestanden und kann nicht gespeichert werden:
        <ul>
          ${errors.map(i => html`<li class="issue"><strong>${i.code}</strong> ${i.message.slice(0, 300)}</li>`)}
        </ul>
      </wa-callout>`;
    }
    const warnings = result.report.issues.filter(i => i.level === 'warning');
    return html`
      <div class="stack">
        <wa-callout variant="success">
          Die Prüfung (PDF/A-3, ZUGFeRD und
          EN&nbsp;16931${this.draft?.profile === 'xrechnung' ? ', XRechnung' : ''}) wurde bestanden.
          ${
            this.stored?.status === 'draft'
              ? html`Die Rechnung erhält beim Speichern die Nummer
                  <strong>${result.number}</strong> und wird gesperrt.`
              : nothing
          }
        </wa-callout>
        ${
          warnings.length > 0
            ? html`<wa-callout variant="neutral"
                >Hinweise:
                <ul>
                  ${warnings.map(i => html`<li class="issue">${i.message.slice(0, 200)}</li>`)}
                </ul></wa-callout
              >`
            : nothing
        }
        <div class="stack">
          <wa-checkbox
            ?checked="${exporting.pdf}"
            @change="${(e: Event) => {
              exporting.pdf = (e.target as HTMLInputElement).checked;
              this.requestUpdate();
            }}"
            >PDF (ZUGFeRD / Factur-X)</wa-checkbox
          >
          <wa-checkbox
            ?checked="${exporting.xml}"
            @change="${(e: Event) => {
              exporting.xml = (e.target as HTMLInputElement).checked;
              this.requestUpdate();
            }}"
            >${this.draft?.profile === 'xrechnung' ? 'XRechnung (XML)' : 'XML (Factur-X)'}</wa-checkbox
          >
        </div>
        ${exporting.error ? html`<wa-callout variant="danger">${exporting.error}</wa-callout>` : nothing}
      </div>
    `;
  }

  override render() {
    const draft = this.draft;
    if (!draft) {
      return this.error ? html`<wa-callout variant="danger">${this.error}</wa-callout>` : nothing;
    }
    const locked = this.locked;
    const stored = this.stored;
    const blocked =
      (this.check?.problems.length ?? 0) > 0 ||
      (this.check?.findings.some(f => f.severity === 'Fatal') ?? false);

    return html`
      <div class="stack">
        <div class="row between">
          <h2>${stored ? (stored.number ?? 'Entwurf') : 'Neue Rechnung'}</h2>
          <div class="row">
            ${stored ? html`<wa-button appearance="outlined" @click="${this.duplicate}">Duplizieren</wa-button>` : nothing}
            ${
              stored && !locked
                ? html`<wa-button
                    appearance="outlined"
                    variant="danger"
                    @click="${this.deleteDraft}"
                    >Entwurf löschen</wa-button
                  >`
                : nothing
            }
          </div>
        </div>

        ${
          locked
            ? html`<wa-callout variant="neutral">
                Exportiert als <strong>${stored?.number}</strong> am
                ${date((stored?.exported_at ?? '').slice(0, 10))}. Exportierte Rechnungen sind
                gesperrt: für Änderungen duplizieren.
              </wa-callout>`
            : nothing
        }
        ${this.error ? html`<wa-callout variant="danger">${this.error}</wa-callout>` : nothing}
        ${this.notice ? html`<wa-callout variant="success">${this.notice}</wa-callout>` : nothing}

        <wa-card>
          <div slot="header">Kunde</div>
          <div class="stack">
            <div class="row">
              <wa-select
                label="Kunde"
                placeholder="Kunde wählen …"
                ?disabled="${locked}"
                .value="${draft.customer.id == null ? '' : String(draft.customer.id)}"
                @change="${(e: Event) => this.selectCustomer(String((e.target as WaSelect).value))}"
              >
                ${this.customers.map(c => html`<wa-option value="${String(c.id)}">${c.name}</wa-option>`)}
              </wa-select>
              ${locked ? nothing : html`<wa-button @click="${() => (this.newCustomer = blankCustomer())}">Neuer Kunde</wa-button>`}
            </div>
            ${
              draft.customer.name
                ? html`<div class="muted">
                    ${draft.customer.name}, ${draft.customer.address.street},
                    ${draft.customer.address.post_code} ${draft.customer.address.city}
                  </div>`
                : nothing
            }
          </div>
        </wa-card>

        <wa-card>
          <div slot="header">Rechnung</div>
          <div class="stack">
            <div class="grid">
              <wa-input
                type="date"
                label="Rechnungsdatum"
                ?disabled="${locked}"
                .value="${draft.issue_date}"
                @input="${(e: Event) => this.edit(d => (d.issue_date = text(e)))}"
              ></wa-input>
              <wa-input
                type="date"
                label="Leistungsdatum"
                ?disabled="${locked}"
                .value="${draft.delivery_date}"
                @input="${(e: Event) => this.edit(d => (d.delivery_date = text(e)))}"
              ></wa-input>
            </div>
            <wa-switch
              ?disabled="${locked}"
              ?checked="${draft.profile === 'xrechnung'}"
              @change="${(e: Event) => this.edit(d => (d.profile = (e.target as WaSwitch).checked ? 'xrechnung' : 'en16931'))}"
              >XRechnung (für öffentliche Auftraggeber)</wa-switch
            >
            ${
              draft.profile === 'xrechnung'
                ? html`<wa-input
                    label="Leitweg-ID / Käuferreferenz"
                    ?disabled="${locked}"
                    .value="${draft.buyer_reference ?? ''}"
                    @input="${(e: Event) => this.edit(d => (d.buyer_reference = nullable(text(e))))}"
                  ></wa-input>`
                : nothing
            }
          </div>
        </wa-card>

        <wa-card>
          <div slot="header">Positionen</div>
          <div class="stack">${this.renderPositions(locked)} ${this.renderSummary()}</div>
        </wa-card>

        <wa-card>
          <div slot="header">Anmerkung</div>
          <wa-textarea
            aria-label="Anmerkung"
            rows="2"
            ?disabled="${locked}"
            .value="${draft.note ?? ''}"
            @input="${(e: Event) => this.edit(d => (d.note = nullable(text(e))))}"
          ></wa-textarea>
        </wa-card>

        ${this.renderCheck()}

        <div class="row end">
          ${
            locked
              ? nothing
              : html`<wa-button ?loading="${this.busy}" @click="${this.saveDraft}"
                  >Entwurf speichern</wa-button
                >`
          }
          <wa-button variant="brand" ?disabled="${blocked && !locked}" @click="${this.startExport}">
            ${locked ? 'Erneut speichern …' : 'Erstellen und prüfen …'}
          </wa-button>
        </div>
      </div>

      ${this.renderExport()}

      <wa-dialog
        ?open="${this.newCustomer !== undefined}"
        label="Neuer Kunde"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && (this.newCustomer = undefined)}"
      >
        ${
          this.newCustomer
            ? html`<erng-customer-form
                .customer="${this.newCustomer}"
                @change="${() => this.requestUpdate()}"
              ></erng-customer-form>`
            : nothing
        }
        <wa-button slot="footer" appearance="plain" @click="${() => (this.newCustomer = undefined)}"
          >Abbrechen</wa-button
        >
        <wa-button slot="footer" variant="brand" @click="${this.saveNewCustomer}"
          >Speichern</wa-button
        >
      </wa-dialog>
    `;
  }
}

function isProduceError(error: unknown): error is ProduceError {
  return typeof error === 'object' && error !== null && 'kind' in error;
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-invoice-edit': InvoiceEditView;
  }
}
