import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';
import '../page/page.component.js';
import '../unit-code/unit-code.component.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import { ask } from '@tauri-apps/plugin-dialog';
import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { CatalogEntry, Unit, UnitUsage } from '../../utils/api.utils.js';
import { api } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { text } from '../../utils/form.utils.js';
import { notify, notifyError } from '../../utils/notify.utils.js';
import reorderStyles from '../../utils/reorder.utils.css';
import { Reorder } from '../../utils/reorder.utils.js';
import { catalogName, defaultUnitOf } from '../../utils/units.utils.js';
import styles from './unit-list.component.css';

/** The catalog is the same for every context and comes once, on first use */
let catalogLoad: Promise<CatalogEntry[]> | undefined;
const loadCatalog = () =>
  (catalogLoad ??= api.unitsCatalog().catch(error => {
    catalogLoad = undefined;
    throw error;
  }));

const isEmpty = (usage: UnitUsage) => usage.templates.length + usage.drafts.length === 0;

/**
 * The units of one context: what the positions of its invoices and offers pick from. Every change
 * is saved at once, in a dialog. A new label reaches drafts and saved positions, created invoices
 * keep the one they were made with. A unit that is still in use is deleted by naming the unit that
 * takes its place. The default unit, which new positions start with, is chosen at the top and saved
 * at once as well. Whenever something was saved, `units-change` tells the host, which holds the
 * context's settings: a renamed or deleted default unit changes them.
 */
@customElement('erng-unit-list')
export class UnitListComponent extends ErngElement.withStyles(reorderStyles, styles) {
  @property({ type: Number, attribute: 'context-id' })
  contextId: number | undefined;

  /** The label of the context's default unit as it is stored, none: the first unit */
  @property({ attribute: 'default-unit' })
  defaultUnit: string | null = null;

  private readonly reorder = new Reorder(this, {
    rows: '.unit',
    noun: 'Einheit',
    count: () => this.units.length,
    move: (from, to) => this.move(from, to),
  });

  @state() private units: Unit[] = [];
  /** How many drafts and saved positions use each unit, by unit id */
  @state() private counts: Record<string, number> = {};
  @state() private catalog: CatalogEntry[] = [];
  /** The names of the catalog by code, for the rows */
  private names = new Map<string, string>();
  /** The dialog to add or change a unit: what is typed, and where the unit is used */
  @state() private editing:
    { id: number | null; label: string; code: string; usage?: UnitUsage } | undefined;
  /** The dialog to delete a unit that is in use: where, and what takes its place */
  @state() private replacing: { unit: Unit; usage: UnitUsage; replacement: string } | undefined;

  override connectedCallback() {
    super.connectedCallback();
    // the rows do not wait for the 2000 names, they fill in when those arrive
    loadCatalog()
      .then(catalog => {
        this.names = new Map(catalog.map(e => [e.code, catalogName(e)]));
        this.catalog = catalog;
      })
      .catch(notifyError);
  }

  protected override willUpdate(changed: PropertyValues<this>) {
    if (changed.has('contextId')) {
      void this.load();
    }
  }

  private async load() {
    const id = this.contextId;
    if (id === undefined) {
      return;
    }
    try {
      const [units, counts] = await Promise.all([api.unitsList(id), api.unitsCounts(id)]);
      // the context changed while the answer was on its way
      if (id === this.contextId) {
        this.units = units;
        this.counts = counts;
      }
    } catch (error) {
      notifyError(error);
    }
  }

  /** The list changed: show it, and tell the host, whose copy of the settings may have followed */
  private async changed() {
    await this.load();
    this.dispatchEvent(new CustomEvent('units-change', { bubbles: true, composed: true }));
  }

  private async chooseDefault(id: string) {
    const contextId = this.contextId;
    const unit = this.units.find(u => String(u.id) === id);
    if (!unit || contextId === undefined) {
      return;
    }
    const before = this.defaultUnit;
    this.defaultUnit = unit.label;
    try {
      await api.unitDefault(contextId, unit.label);
      await this.changed();
    } catch (error) {
      this.defaultUnit = before;
      notifyError(error);
    }
  }

  private openAdd() {
    this.editing = { id: null, label: '', code: 'C62' };
  }

  private async openEdit(unit: Unit) {
    const id = unit.id as number;
    this.editing = { id, label: unit.label, code: unit.code };
    try {
      const usage = await api.unitUsage(id);
      if (this.editing?.id === id) {
        this.editing = { ...this.editing, usage };
      }
    } catch (error) {
      notifyError(error);
    }
  }

  private change(update: Partial<NonNullable<UnitListComponent['editing']>>) {
    if (this.editing) {
      this.editing = { ...this.editing, ...update };
    }
  }

  private async save() {
    const edit = this.editing;
    const contextId = this.contextId;
    if (!edit || contextId === undefined) {
      return;
    }
    try {
      await api.unitSave({
        id: edit.id,
        context_id: contextId,
        code: edit.code,
        label: edit.label,
      });
      this.editing = undefined;
      await this.changed();
      notify('Die Einheit ist gespeichert.');
    } catch (error) {
      notifyError(error);
    }
  }

  /** An unused unit goes after a plain question, one in use asks for the unit that replaces it */
  private async deleteOrReplace() {
    const edit = this.editing;
    const unit = this.units.find(u => u.id === edit?.id);
    if (!edit || !unit) {
      return;
    }
    try {
      const usage = edit.usage ?? (await api.unitUsage(unit.id as number));
      this.editing = undefined;
      if (!isEmpty(usage)) {
        this.replacing = { unit, usage, replacement: '' };
        return;
      }
      const sure = await ask(`„${unit.label}“ aus der Liste der Einheiten löschen?`, {
        title: 'Einheit löschen',
        okLabel: 'Löschen',
        cancelLabel: 'Abbrechen',
      });
      if (sure) {
        await this.deleteUnit(unit, null);
      }
    } catch (error) {
      notifyError(error);
    }
  }

  private async confirmReplace() {
    const choice = this.replacing;
    if (!choice || choice.replacement === '') {
      return;
    }
    try {
      await this.deleteUnit(choice.unit, Number(choice.replacement));
      this.replacing = undefined;
    } catch (error) {
      notifyError(error);
    }
  }

  private async deleteUnit(unit: Unit, replacement: number | null) {
    await api.unitDelete(unit.id as number, replacement);
    await this.changed();
    notify(`„${unit.label}“ ist gelöscht.`);
  }

  private async move(from: number, to: number) {
    const contextId = this.contextId;
    const before = this.units;
    const units = [...before];
    const [moved] = units.splice(from, 1);
    if (!moved || contextId === undefined) {
      return;
    }
    units.splice(to, 0, moved);
    this.units = units;
    try {
      await api.unitsReorder(
        contextId,
        units.map(u => u.id as number)
      );
    } catch (error) {
      this.units = before;
      notifyError(error);
    }
  }

  private renderRow(unit: Unit, index: number) {
    const name = this.names.get(unit.code);
    const uses = this.counts[String(unit.id)] ?? 0;
    const isDefault = defaultUnitOf(this.units, this.defaultUnit) === unit;
    return html`
      <div class="unit">
        ${this.reorder.handle(index)}
        <button class="main" type="button" @click="${() => void this.openEdit(unit)}">
          <strong>${unit.label}</strong>
          <span class="muted">${unit.code}${name ? ` – ${name}` : ''}</span>
          ${
            isDefault
              ? html`
                  <wa-tag
                    size="small"
                    variant="brand"
                    title="Die Einheit, mit der neue Positionen beginnen"
                  >
                    Standard
                  </wa-tag>
                `
              : nothing
          }
          ${
            uses > 0
              ? html`
                  <span class="muted uses" title="In Entwürfen und gespeicherten Positionen">
                    ${uses === 1 ? '1 Verwendung' : `${uses} Verwendungen`}
                  </span>
                `
              : nothing
          }
        </button>
      </div>
    `;
  }

  /** The unit new positions start with: one of the list, the first until another is chosen */
  private defaultSelect() {
    const chosen = defaultUnitOf(this.units, this.defaultUnit);
    return html`
      <wa-select
        class="default"
        label="Standard-Einheit"
        hint="Mit dieser Einheit beginnen neue Positionen."
        .value="${chosen ? String(chosen.id) : ''}"
        @change="${(e: Event) => void this.chooseDefault(String((e.target as WaSelect).value))}"
      >
        ${this.units.map(u => html`<wa-option value="${String(u.id)}">${u.label}</wa-option>`)}
      </wa-select>
    `;
  }

  private editDialog() {
    const edit = this.editing;
    const usage = edit?.usage;
    return html`
      <wa-dialog
        ?open="${edit !== undefined}"
        label="${edit?.id == null ? 'Einheit hinzufügen' : 'Einheit bearbeiten'}"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && (this.editing = undefined)}"
      >
        ${
          edit
            ? html`
                <div class="stack">
                  <wa-input
                    label="Bezeichnung"
                    hint="So steht die Einheit in der Position und auf dem Dokument."
                    required
                    ?autofocus="${edit.id == null}"
                    .value="${edit.label}"
                    @input="${(e: Event) => this.change({ label: text(e) })}"
                    @keydown="${(e: KeyboardEvent) => e.key === 'Enter' && void this.save()}"
                  ></wa-input>
                  <erng-unit-code
                    .catalog="${this.catalog}"
                    .value="${edit.code}"
                    @code-change="${(e: CustomEvent<string>) => this.change({ code: e.detail })}"
                  ></erng-unit-code>
                  ${
                    usage && !isEmpty(usage)
                      ? html`
                          <wa-callout variant="neutral">
                            Wird in
                            ${count(usage.templates.length, 'gespeicherten Position', 'gespeicherten Positionen')}
                            und ${count(usage.drafts.length, 'Entwurf', 'Entwürfen')} verwendet, sie
                            übernehmen die Änderung. Erstellte Dokumente behalten ihre Bezeichnung.
                          </wa-callout>
                        `
                      : nothing
                  }
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
              edit?.id != null
                ? html`
                    <wa-button
                      variant="danger"
                      appearance="outlined"
                      ?disabled="${this.units.length < 2}"
                      title="${this.units.length < 2 ? 'Die letzte Einheit bleibt.' : ''}"
                      @click="${this.deleteOrReplace}"
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
    `;
  }

  private replaceDialog() {
    const choice = this.replacing;
    const others = this.units.filter(u => u.id !== choice?.unit.id);
    return html`
      <wa-dialog
        ?open="${choice !== undefined}"
        label="Einheit ersetzen und löschen"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && (this.replacing = undefined)}"
      >
        ${
          choice
            ? html`
                <div class="stack">
                  <span>
                    „${choice.unit.label}“ wird noch verwendet. Die Positionen darunter bekommen die
                    Einheit, die du wählst. Erstellte Dokumente behalten ihre Bezeichnung.
                  </span>
                  <ul class="uses-list">
                    ${choice.usage.templates.map(
                      t => html`
                        <li>
                          Gespeicherte Position
                          „${t.description}“${t.context_id === null ? ' (für alle Kontexte)' : ''}
                        </li>
                      `
                    )}
                    ${choice.usage.drafts.map(
                      d => html`
                        <li>
                          ${d.kind === 'offer' ? 'Angebotsentwurf' : 'Rechnungsentwurf'}
                          ${d.customer.trim() ? `für ${d.customer}` : 'ohne Kunde'}
                        </li>
                      `
                    )}
                  </ul>
                  <wa-select
                    label="Ersetzen durch"
                    required
                    .value="${choice.replacement}"
                    @change="${(e: Event) =>
                      (this.replacing = {
                        ...choice,
                        replacement: String((e.target as WaSelect).value),
                      })}"
                  >
                    ${others.map(
                      u =>
                        html`<wa-option value="${String(u.id)}">${u.label} (${u.code})</wa-option>`
                    )}
                  </wa-select>
                </div>
              `
            : nothing
        }
        <div slot="footer" class="dialog-footer">
          <wa-button appearance="outlined" @click="${() => (this.replacing = undefined)}">
            Abbrechen
          </wa-button>
          <wa-button
            variant="danger"
            ?disabled="${choice?.replacement === ''}"
            @click="${this.confirmReplace}"
          >
            Ersetzen und löschen
          </wa-button>
        </div>
      </wa-dialog>
    `;
  }

  override render() {
    return html`
      <erng-page heading="Einheiten">
        <span class="muted">
          Die Einheiten, aus denen die Positionen dieses Kontexts wählen. Ändert sich eine
          Bezeichnung, übernehmen sie Entwürfe und gespeicherte Positionen; erstellte Rechnungen und
          Angebote behalten ihre.
        </span>
        ${this.defaultSelect()}
        <div class="units">${this.units.map((unit, index) => this.renderRow(unit, index))}</div>
        <wa-button slot="primary" variant="brand" @click="${this.openAdd}">
          Einheit hinzufügen
        </wa-button>
      </erng-page>
      ${this.editDialog()} ${this.replaceDialog()}
    `;
  }
}

/** `1 Entwurf`, `2 Entwürfen` */
function count(n: number, one: string, many: string) {
  return `${n} ${n === 1 ? one : many}`;
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-unit-list': UnitListComponent;
  }
}
