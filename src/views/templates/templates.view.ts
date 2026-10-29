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

import type { PositionTemplate, Unit } from '../../common/utils/api.utils.js';
import { api } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import { activeContext, contextsOf, label } from '../../common/utils/context.utils.js';
import { nullable, text } from '../../common/utils/form.utils.js';
import { notify, notifyError } from '../../common/utils/notify.utils.js';
import { firstUnit, pick, unitLabel, unitOf } from '../../common/utils/units.utils.js';
import styles from './templates.view.css';

const blank = (units: Unit[]): PositionTemplate => ({
  id: null,
  context_id: null,
  description: '',
  quantity: null,
  ...firstUnit(units),
  price: '',
});

/** The value of the unit select for a unit that is not in the list */
const NOT_LISTED = 'not-listed';

/** Saved positions the invoice form can start from, as cards like the customers */
@customElement('erng-templates')
export class TemplatesView extends ErngElement.withStyles(styles) {
  @state() private templates: PositionTemplate[] = [];
  /** The units of the active context, what the positions pick from */
  @state() private units: Unit[] = [];
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
        [this.templates, this.units] = await Promise.all([
          api.templatesList(context),
          api.unitsList(context),
        ]);
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
    // the price may stay open, it is filled in on the invoice
    if (template.description.trim() === '') {
      notify('Eine Beschreibung ist erforderlich.', 'danger');
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
    const unit = this.units.find(u => String(u.id) === this.unit);
    return this.templates.filter(
      t =>
        (this.unit === 'all' || (unit !== undefined && unitOf([unit], t) !== undefined)) &&
        [t.description, ...contextsOf(t.context_id).map(label)]
          .join(' ')
          .toLowerCase()
          .includes(query)
    );
  }

  /** The unit of the position, from the list of the context; one that is not in it still shows */
  private unitSelect(template: PositionTemplate) {
    const current = unitOf(this.units, template);
    return html`
      <wa-select
        label="Einheit"
        .value="${current ? String(current.id) : NOT_LISTED}"
        @change="${(e: Event) => {
          const id = String((e.target as WaSelect).value);
          const unit = this.units.find(u => String(u.id) === id);
          this.change(t => pick(t, unit));
        }}"
      >
        ${this.units.map(u => html`<wa-option value="${String(u.id)}">${u.label}</wa-option>`)}
        ${
          current
            ? nothing
            : html`
                <wa-option value="${NOT_LISTED}">
                  ${unitLabel(template)} (nicht in der Liste)
                </wa-option>
              `
        }
      </wa-select>
    `;
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
            size="s"
            aria-label="Einheit"
            .value="${this.unit}"
            @change="${(e: Event) => (this.unit = String((e.target as WaSelect).value))}"
          >
            <wa-option value="all">Alle Einheiten</wa-option>
            ${this.units.map(u => html`<wa-option value="${String(u.id)}">${u.label}</wa-option>`)}
          </wa-select>
          <wa-button
            slot="actions"
            size="s"
            variant="brand"
            @click="${() => this.edit(blank(this.units))}"
          >
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
                        ${t.quantity ?? '1'}
                        ${unitLabel(t)}${t.price.trim() ? ` × ${t.price} €` : ''}
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
                      ${this.unitSelect(editing)}
                      <wa-input
                        label="Preis (netto)"
                        inputmode="decimal"
                        placeholder="0,00"
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
                  ? html`
                      <wa-button
                        variant="danger"
                        appearance="outlined"
                        @click="${this.deleteTemplate}"
                      >
                        Löschen
                      </wa-button>
                    `
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
