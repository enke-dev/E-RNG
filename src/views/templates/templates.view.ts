import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/button-group/button-group.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/card/card.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';
import '../../common/components/filter-bar/filter-bar.component.js';
import '../../common/components/scope-select/scope-select.component.js';
import '../../common/layouts/cards/cards.layout.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import { html, nothing } from 'lit';
import { customElement, state } from 'lit/decorators.js';

import type { PositionTemplate } from '../../common/utils/api.utils.js';
import { api } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import { activeContext, contextsOf, label } from '../../common/utils/context.utils.js';
import { nullable, text } from '../../common/utils/form.utils.js';
import { UNITS } from '../../common/utils/format.utils.js';
import { notify, notifyError } from '../../common/utils/notify.utils.js';
import styles from './templates.view.css';

const blank = (): PositionTemplate => ({
  id: null,
  context_id: null,
  description: '',
  quantity: null,
  unit: null,
  price: '',
});

const unitLabel = (code: string | null) => UNITS.find(u => u.code === (code ?? 'C62'))?.label ?? '';

/** Saved positions the invoice form can start from, as cards like the customers */
@customElement('erng-templates')
export class TemplatesView extends ErngElement.withStyles(styles) {
  @state() private templates: PositionTemplate[] = [];
  @state() private editing: PositionTemplate | undefined;
  @state() private query = '';
  @state() private unit = 'all';

  override connectedCallback() {
    super.connectedCallback();
    void this.load();
  }

  private async load() {
    try {
      const context = activeContext.get()?.id;
      if (context != null) {
        this.templates = await api.templatesList(context);
      }
    } catch (error) {
      notifyError(error);
    }
  }

  private edit(template: PositionTemplate) {
    this.editing = structuredClone(template);
  }

  private async save() {
    const template = this.editing;
    if (!template) {
      return;
    }
    if (template.description.trim() === '' || template.price.trim() === '') {
      notify('Beschreibung und Preis sind erforderlich.', 'danger');
      return;
    }
    try {
      await api.templateSave(template);
      this.editing = undefined;
      await this.load();
    } catch (error) {
      notifyError(error);
    }
  }

  private async deleteTemplate() {
    const id = this.editing?.id;
    if (id == null) {
      return;
    }
    try {
      await api.templateDelete(id);
      this.editing = undefined;
      await this.load();
    } catch (error) {
      notifyError(error);
    }
  }

  private change(update: (template: PositionTemplate) => void) {
    if (this.editing) {
      update(this.editing);
      this.requestUpdate();
    }
  }

  private get shown(): PositionTemplate[] {
    const query = this.query.trim().toLowerCase();
    return this.templates.filter(
      t =>
        (this.unit === 'all' || (t.unit ?? 'C62') === this.unit) &&
        t.description.toLowerCase().includes(query)
    );
  }

  override render() {
    const editing = this.editing;
    const shown = this.shown;
    return html`
      <erng-cards>
        <erng-filter-bar
          slot="header"
          .query="${this.query}"
          placeholder="Positionen suchen"
          @query-change="${(e: CustomEvent<string>) => (this.query = e.detail)}"
        >
          <wa-select
            slot="filters"
            aria-label="Einheit"
            .value="${this.unit}"
            @change="${(e: Event) => (this.unit = String((e.target as WaSelect).value))}"
          >
            <wa-option value="all">Alle Einheiten</wa-option>
            ${UNITS.map(u => html`<wa-option value="${u.code}">${u.label}</wa-option>`)}
          </wa-select>
          <wa-button slot="actions" variant="brand" @click="${() => this.edit(blank())}">
            Neue Position
          </wa-button>
        </erng-filter-bar>
        ${
          shown.length === 0
            ? html`
                <wa-callout slot="notice" variant="neutral">
                  ${
                    this.templates.length === 0
                      ? 'Noch keine gespeicherten Positionen. Hier oder im Rechnungsformular mit „Position für später speichern“ anlegen.'
                      : 'Keine Treffer.'
                  }
                </wa-callout>
              `
            : html`
                ${shown.map(
                  t => html`
                    <wa-card slot="cards" @click="${() => this.edit(t)}">
                      <strong>${t.description}</strong>
                      <div class="muted">
                        ${t.quantity ?? '1'} ${unitLabel(t.unit)} × ${t.price} €
                      </div>
                      ${
                        contextsOf(t.context_id).length > 0
                          ? html`
                              <div class="contexts">
                                ${contextsOf(t.context_id).map(
                                  context => html`
                                    <wa-tag size="s" variant="neutral">${label(context)}</wa-tag>
                                  `
                                )}
                              </div>
                            `
                          : nothing
                      }
                    </wa-card>
                  `
                )}
              `
        }

        <wa-dialog
          slot="dialog"
          ?open="${editing !== undefined}"
          label="${editing?.id == null ? 'Neue Position' : 'Position bearbeiten'}"
          @wa-after-hide="${(e: Event) => e.target === e.currentTarget && (this.editing = undefined)}"
        >
          ${
            editing
              ? html`
                  <div class="stack">
                    <wa-input
                      label="Beschreibung"
                      required
                      .value="${editing.description}"
                      @input="${(e: Event) => this.change(t => (t.description = text(e)))}"
                    ></wa-input>
                    <div class="grid">
                      <wa-input
                        label="Menge"
                        placeholder="1"
                        inputmode="decimal"
                        .value="${editing.quantity ?? ''}"
                        @input="${(e: Event) => this.change(t => (t.quantity = nullable(text(e))))}"
                      ></wa-input>
                      <wa-select
                        label="Einheit"
                        .value="${editing.unit ?? 'C62'}"
                        @change="${(e: Event) =>
                          this.change(t => {
                            const unit = String((e.target as WaSelect).value);
                            t.unit = unit === 'C62' ? null : unit;
                          })}"
                      >
                        ${UNITS.map(u => html`<wa-option value="${u.code}">${u.label}</wa-option>`)}
                      </wa-select>
                      <wa-input
                        label="Preis (netto)"
                        inputmode="decimal"
                        placeholder="0,00"
                        required
                        .value="${editing.price}"
                        @input="${(e: Event) => this.change(t => (t.price = text(e)))}"
                      ></wa-input>
                    </div>
                    <erng-scope-select
                      .value="${editing.context_id}"
                      @scope-change="${(e: CustomEvent<number | null>) =>
                        this.change(t => (t.context_id = e.detail))}"
                    ></erng-scope-select>
                  </div>
                `
              : nothing
          }
          <div slot="footer" class="dialog-footer">
            <wa-button appearance="outlined" @click="${() => (this.editing = undefined)}">
              Abbrechen
            </wa-button>
            <div class="end">
              ${
                editing?.id != null
                  ? html`<wa-button
                      variant="danger"
                      appearance="outlined"
                      @click="${this.deleteTemplate}"
                    >
                      Löschen
                    </wa-button>`
                  : nothing
              }
              <wa-button variant="brand" @click="${this.save}">Speichern</wa-button>
            </div>
          </div>
        </wa-dialog>
      </erng-cards>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-templates': TemplatesView;
  }
}
