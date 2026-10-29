import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';
import '../page/page.component.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { Rate, RateUsage } from '../../utils/api.utils.js';
import { api } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { text } from '../../utils/form.utils.js';
import { percent } from '../../utils/format.utils.js';
import { notify, notifyError } from '../../utils/notify.utils.js';
import { chargesVat, defaultRateOf, rateText, sameRate } from '../../utils/rates.utils.js';
import reorderStyles from '../../utils/reorder.utils.css';
import { Reorder } from '../../utils/reorder.utils.js';
import { ask } from '../ask/ask.component.js';
import styles from './rate-list.component.css';

/** The value of the default's select for no VAT */
const NO_VAT = 'none';

/** The value of the default's select for a rate the list lacks */
const NOT_LISTED = 'not-listed';

const isEmpty = (usage: RateUsage) =>
  usage.templates.length + usage.drafts.length === 0 && !usage.default;

/**
 * The VAT rates of one context: what the positions of its invoices and offers pick from, and the
 * default rate of the context is one of, chosen at the top (or no VAT at all). Every change is
 * saved at once, the list's in a dialog. A new label or
 * value reaches drafts and saved positions (and the default), created invoices keep the one they
 * were made with. A rate that is still in use is deleted by naming the rate that takes its place.
 * Whenever something was saved, `rates-change` tells the host, which holds the context's settings.
 */
@customElement('erng-rate-list')
export class RateListComponent extends ErngElement.withStyles(reorderStyles, styles) {
  @property({ type: Number, attribute: 'context-id' })
  contextId: number | undefined;

  /** The default rate of the context as it is stored, blank: the context charges no VAT */
  @property({ attribute: 'default-rate' })
  defaultRate = '';

  private readonly reorder = new Reorder(this, {
    rows: '.rate',
    noun: 'Steuersatz',
    count: () => this.rates.length,
    move: (from, to) => this.move(from, to),
  });

  @state() private rates: Rate[] = [];
  /** How many drafts and saved positions use each rate, by rate id */
  @state() private counts: Record<string, number> = {};
  /** The dialog to add or change a rate: what is typed, and where the rate is used */
  @state() private editing:
    { id: number | null; label: string; rate: string; usage?: RateUsage } | undefined;
  /** The dialog to delete a rate that is in use: where, and what takes its place */
  @state() private replacing: { rate: Rate; usage: RateUsage; replacement: string } | undefined;

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
      const [rates, counts] = await Promise.all([api.ratesList(id), api.ratesCounts(id)]);
      // the context changed while the answer was on its way
      if (id === this.contextId) {
        this.rates = rates;
        this.counts = counts;
      }
    } catch (error) {
      notifyError(error);
    }
  }

  /** The list changed: show it, and tell the host, whose copy of the settings may have followed */
  private async changed() {
    await this.load();
    this.dispatchEvent(new CustomEvent('rates-change', { bubbles: true, composed: true }));
  }

  /** Makes a rate of the list the default of the context, or no VAT */
  private async chooseDefault(choice: string) {
    const contextId = this.contextId;
    const rate = choice === NO_VAT ? '' : this.rates.find(r => String(r.id) === choice)?.rate;
    if (rate === undefined || contextId === undefined) {
      return;
    }
    const before = this.defaultRate;
    this.defaultRate = rate;
    try {
      await api.rateDefault(contextId, rate);
      await this.changed();
    } catch (error) {
      this.defaultRate = before;
      notifyError(error);
    }
  }

  private openAdd() {
    this.editing = { id: null, label: '', rate: '' };
  }

  private async openEdit(rate: Rate) {
    const id = rate.id as number;
    this.editing = { id, label: rate.label, rate: rate.rate.replace('.', ',') };
    try {
      const usage = await api.rateUsage(id);
      if (this.editing?.id === id) {
        this.editing = { ...this.editing, usage };
      }
    } catch (error) {
      notifyError(error);
    }
  }

  private change(update: Partial<NonNullable<RateListComponent['editing']>>) {
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
      await api.rateSave({
        id: edit.id,
        context_id: contextId,
        label: edit.label,
        rate: edit.rate,
      });
      this.editing = undefined;
      await this.changed();
      notify('Der Steuersatz ist gespeichert.');
    } catch (error) {
      notifyError(error);
    }
  }

  /** An unused rate goes after a plain question, one in use asks for the rate that replaces it */
  private async deleteOrReplace() {
    const edit = this.editing;
    const rate = this.rates.find(r => r.id === edit?.id);
    if (!edit || !rate) {
      return;
    }
    try {
      const usage = edit.usage ?? (await api.rateUsage(rate.id as number));
      this.editing = undefined;
      if (!isEmpty(usage)) {
        this.replacing = { rate, usage, replacement: '' };
        return;
      }
      const sure = await ask(`„${rate.label}“ aus der Liste der Steuersätze löschen?`, {
        title: 'Steuersatz löschen',
        danger: true,
        okLabel: 'Löschen',
        cancelLabel: 'Abbrechen',
      });
      if (sure) {
        await this.deleteRate(rate, null);
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
      await this.deleteRate(choice.rate, Number(choice.replacement));
      this.replacing = undefined;
    } catch (error) {
      notifyError(error);
    }
  }

  private async deleteRate(rate: Rate, replacement: number | null) {
    await api.rateDelete(rate.id as number, replacement);
    await this.changed();
    notify(`„${rate.label}“ ist gelöscht.`);
  }

  private async move(from: number, to: number) {
    const contextId = this.contextId;
    const before = this.rates;
    const rates = [...before];
    const [moved] = rates.splice(from, 1);
    if (!moved || contextId === undefined) {
      return;
    }
    rates.splice(to, 0, moved);
    this.rates = rates;
    try {
      await api.ratesReorder(
        contextId,
        rates.map(r => r.id as number)
      );
    } catch (error) {
      this.rates = before;
      notifyError(error);
    }
  }

  private renderRow(rate: Rate, index: number) {
    const uses = this.counts[String(rate.id)] ?? 0;
    return html`
      <div class="rate">
        ${this.reorder.handle(index)}
        <button class="main" type="button" @click="${() => void this.openEdit(rate)}">
          <strong>${rate.label}</strong>
          <span class="muted">${percent(rate.rate)}</span>
          ${
            sameRate(rate.rate, this.defaultRate)
              ? html`
                  <wa-tag
                    size="small"
                    variant="brand"
                    title="Der Satz, den Positionen ohne eigene Wahl nehmen"
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

  /** The default rate: one of the list, or no VAT; a rate the list lacks still shows */
  private defaultSelect() {
    const current = this.defaultRate.trim();
    const listed = defaultRateOf(this.rates, current);
    const value = current === '' ? NO_VAT : listed ? String(listed.id) : NOT_LISTED;
    return html`
      <wa-select
        class="default"
        label="Standard-Steuersatz"
        hint="Den nehmen Positionen, die keinen eigenen Satz wählen, in Rechnungen wie in Angeboten. Keine Umsatzsteuer z. B. als Kleinunternehmer."
        .value="${value}"
        @change="${(e: Event) => void this.chooseDefault(String((e.target as WaSelect).value))}"
      >
        ${this.rates.map(r => html`<wa-option value="${String(r.id)}">${rateText(r)}</wa-option>`)}
        ${
          value === NOT_LISTED
            ? html`
                <wa-option value="${NOT_LISTED}" disabled>
                  ${percent(current)} (nicht in der Liste)
                </wa-option>
              `
            : nothing
        }
        <wa-option value="${NO_VAT}">Keine Umsatzsteuer</wa-option>
      </wa-select>
    `;
  }

  private editDialog() {
    const edit = this.editing;
    const usage = edit?.usage;
    return html`
      <wa-dialog
        ?open="${edit !== undefined}"
        label="${edit?.id == null ? 'Steuersatz hinzufügen' : 'Steuersatz bearbeiten'}"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && (this.editing = undefined)}"
      >
        ${
          edit
            ? html`
                <div class="stack">
                  <wa-input
                    label="Bezeichnung"
                    hint="So steht der Satz in der Auswahl der Position, z. B. Regelsatz."
                    required
                    ?autofocus="${edit.id == null}"
                    .value="${edit.label}"
                    @input="${(e: Event) => this.change({ label: text(e) })}"
                    @keydown="${(e: KeyboardEvent) => e.key === 'Enter' && void this.save()}"
                  ></wa-input>
                  <wa-input
                    label="Steuersatz (%)"
                    hint="Mehr als 0 und höchstens 100, Komma oder Punkt."
                    required
                    inputmode="decimal"
                    .value="${edit.rate}"
                    @input="${(e: Event) => this.change({ rate: text(e) })}"
                    @keydown="${(e: KeyboardEvent) => e.key === 'Enter' && void this.save()}"
                  ></wa-input>
                  ${
                    usage && !isEmpty(usage)
                      ? html`
                          <wa-callout variant="neutral">
                            ${usageText(usage)} Erstellte Dokumente behalten ihren Satz.
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
    const others = this.rates.filter(r => r.id !== choice?.rate.id);
    return html`
      <wa-dialog
        ?open="${choice !== undefined}"
        label="Steuersatz ersetzen und löschen"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && (this.replacing = undefined)}"
      >
        ${
          choice
            ? html`
                <div class="stack">
                  <span>
                    „${choice.rate.label}“ wird noch verwendet. Wo, übernimmt den Satz, den du
                    wählst. Erstellte Dokumente behalten ihren Satz.
                  </span>
                  <ul class="uses-list">
                    ${choice.usage.default ? html`<li>Standardsatz dieses Kontexts</li>` : nothing}
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
                  ${
                    others.length === 0
                      ? html`
                          <wa-callout variant="neutral">
                            Es gibt keinen anderen Steuersatz. Lege einen an, oder stelle den
                            Standardsatz unter „Rechnungen“ auf „Keine Umsatzsteuer“.
                          </wa-callout>
                        `
                      : html`
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
                              r =>
                                html`<wa-option value="${String(r.id)}">${rateText(r)}</wa-option>`
                            )}
                          </wa-select>
                        `
                  }
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
            ?disabled="${choice?.replacement === '' || others.length === 0}"
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
      <erng-page heading="Steuersätze">
        <span class="muted">
          Die Umsatzsteuersätze, aus denen die Positionen dieses Kontexts wählen. Ändert sich ein
          Satz oder seine Bezeichnung, übernehmen ihn Entwürfe und gespeicherte Positionen;
          erstellte Rechnungen und Angebote behalten ihren.
        </span>
        ${this.defaultSelect()}
        ${
          chargesVat(this.defaultRate)
            ? nothing
            : html`
                <wa-callout class="hint" variant="neutral">
                  Dieser Kontext berechnet keine Umsatzsteuer, deshalb gibt es in den Positionen
                  keine Auswahl. Die Liste lässt sich trotzdem pflegen. Den Hinweis zur
                  Steuerbefreiung findest du unter „Rechnungen“.
                </wa-callout>
              `
        }
        <div class="rates">${this.rates.map((rate, index) => this.renderRow(rate, index))}</div>
        <wa-button slot="primary" variant="brand" @click="${this.openAdd}">
          Steuersatz hinzufügen
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

/** Where a rate is used, what a change of it reaches */
function usageText(usage: RateUsage) {
  const parts = [
    ...(usage.default ? ['als Standardsatz dieses Kontexts'] : []),
    ...(usage.templates.length > 0
      ? [
          `in ${count(usage.templates.length, 'gespeicherten Position', 'gespeicherten Positionen')}`,
        ]
      : []),
    ...(usage.drafts.length > 0
      ? [`in ${count(usage.drafts.length, 'Entwurf', 'Entwürfen')}`]
      : []),
  ];
  return `Wird ${parts.join(' und ')} verwendet. Ein neuer Satz gilt dort auch.`;
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-rate-list': RateListComponent;
  }
}
