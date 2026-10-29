import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';
import '@awesome.me/webawesome/dist/components/color-picker/color-picker.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/textarea/textarea.js';
import '../anchor-picker/anchor-picker.component.js';

import type WaColorPicker from '@awesome.me/webawesome/dist/components/color-picker/color-picker.js';
import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import { open } from '@tauri-apps/plugin-dialog';
import { html, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import type { Align, InfoField, Part } from '../../utils/api.utils.js';
import { api } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { text } from '../../utils/form.utils.js';
import { notifyError } from '../../utils/notify.utils.js';
import styles from './part-form.component.css';

/** The facts an information block can show, in the order it shows them */
const FIELDS: [InfoField, string][] = [
  ['number', 'Rechnungsnummer'],
  ['date', 'Datum'],
  ['valid-until', 'Gültig bis (Angebote)'],
  ['service-date', 'Leistungsdatum'],
  ['due-date', 'Fällig am'],
  ['buyer-reference', 'Käuferreferenz'],
  ['preceding-invoice', 'Bezugsrechnung'],
  ['offer-reference', 'Angebot (Rechnungen)'],
  ['tax-number', 'Steuernummer'],
  ['vat-id', 'USt-IdNr.'],
];

const ALIGNS: [Align, string][] = [
  ['left', 'Links'],
  ['center', 'Zentriert'],
  ['right', 'Rechts'],
];

/** What Typst brings along, besides the fonts of the layout */
export const BUILTIN_FAMILIES = ['Libertinus Serif', 'New Computer Modern', 'DejaVu Sans Mono'];

/** The placeholders a text can use, for the hint under it */
const PLACEHOLDERS =
  '{seller_name} {seller_address} {seller_contact} {seller_registration} {legal} {bank}';

/** Presets of the color picker: black, the letterhead's black and the greys the parts have by default */
const SWATCHES = [
  { color: '#000000', label: 'Schwarz' },
  { color: '#231f20', label: 'Briefkopf-Schwarz' },
  { color: '#6e6e6e', label: 'Grau (Seitenzahl)' },
  { color: '#bebebe', label: 'Hellgrau (Linie)' },
];

const MIN_DPI = 150;

/**
 * The fields of one part: where it is attached, how wide it is and what it shows. Edits the given
 * object.
 *
 * @fires change
 */
@customElement('erng-part-form')
export class PartFormComponent extends ErngElement.withStyles(styles) {
  @property({ attribute: false })
  item!: Part;

  /** The font families a text can be set in besides the one of the layout */
  @property({ attribute: false })
  families: string[] = [];

  private changed() {
    this.requestUpdate();
    this.dispatchEvent(new CustomEvent('change', { bubbles: true, composed: true }));
  }

  /** A number of millimetres or points, empty is none (when allowed) */
  private number(
    label: string,
    value: number | null,
    apply: (value: number | null) => void,
    unit = 'mm',
    optional = false
  ) {
    return html`
      <wa-input
        label="${label}"
        type="number"
        step="0.5"
        .value="${value === null ? '' : String(value)}"
        @input="${(e: Event) => {
          const raw = text(e).trim();
          const parsed = Number(raw);
          apply(raw === '' ? (optional ? null : 0) : Number.isFinite(parsed) ? parsed : 0);
          this.changed();
        }}"
      >
        <span slot="end">${unit}</span>
      </wa-input>
    `;
  }

  private align(value: Align, apply: (value: Align) => void) {
    return html`
      <wa-select
        label="Ausrichtung"
        .value="${value}"
        @change="${(e: Event) => {
          apply(String((e.target as WaSelect).value) as Align);
          this.changed();
        }}"
      >
        ${ALIGNS.map(([id, label]) => html`<wa-option value="${id}">${label}</wa-option>`)}
      </wa-select>
    `;
  }

  /** The color of a part, empty keeps the look of its kind */
  private color(part: { color?: string | null }) {
    return html`
      <div class="color">
        <wa-color-picker
          label="Farbe"
          format="hex"
          .swatches="${SWATCHES}"
          .value="${part.color ?? ''}"
          @change="${(e: Event) => {
            part.color = (e.target as WaColorPicker).value || null;
            this.changed();
          }}"
        ></wa-color-picker>
        <wa-button
          appearance="outlined"
          ?disabled="${!part.color}"
          @click="${() => {
            part.color = null;
            this.changed();
          }}"
          >Standard</wa-button
        >
      </div>
    `;
  }

  private async chooseImage() {
    if (this.item.kind !== 'image') {
      return;
    }
    try {
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Bild', extensions: ['png', 'jpg', 'jpeg', 'svg'] }],
      });
      if (typeof picked === 'string') {
        this.item.image = await api.layoutImportImage(picked);
        this.item.width ??= 40;
        this.changed();
      }
    } catch (error) {
      notifyError(error);
    }
  }

  private kind() {
    const part = this.item;
    switch (part.kind) {
      case 'infos': {
        const selected = new Set(part.fields);
        return html`
          <div class="checks">
            ${FIELDS.map(
              ([field, label]) => html`
                <wa-checkbox
                  ?checked="${selected.has(field)}"
                  @change="${(e: Event) => {
                    const on = (e.target as HTMLInputElement).checked;
                    part.fields = FIELDS.map(([f]) => f).filter(f =>
                      f === field ? on : selected.has(f)
                    );
                    this.changed();
                  }}"
                  >${label}</wa-checkbox
                >
              `
            )}
          </div>
          <div class="fields styled">
            ${this.align(part.align, align => (part.align = align))}
            ${this.number('Schriftgröße', part.size, size => (part.size = size ?? 9), 'pt')}
            ${this.color(part)}
          </div>
          <wa-checkbox
            ?checked="${part.labels}"
            @change="${(e: Event) => {
              part.labels = (e.target as HTMLInputElement).checked;
              this.changed();
            }}"
            >Bezeichnungen zeigen</wa-checkbox
          >
          <wa-checkbox
            ?checked="${part.long_dates}"
            @change="${(e: Event) => {
              part.long_dates = (e.target as HTMLInputElement).checked;
              this.changed();
            }}"
            >Datum ausgeschrieben (18. Oktober 2026)</wa-checkbox
          >
        `;
      }
      case 'address':
        return html`
          <div class="fields styled">
            ${this.number('Schriftgröße', part.size, size => (part.size = size ?? 10), 'pt')}
            ${this.color(part)}
          </div>
          <wa-checkbox
            ?checked="${part.sender_line}"
            @change="${(e: Event) => {
              part.sender_line = (e.target as HTMLInputElement).checked;
              this.changed();
            }}"
            >Absenderzeile über der Anschrift</wa-checkbox
          >
        `;
      case 'image': {
        const image = part.image;
        const width = part.width ?? 40;
        const dpi = image && !image.vector ? image.width_px / (width / 25.4) : Infinity;
        return html`
          <div class="file">
            <wa-button appearance="outlined" @click="${this.chooseImage}">Bild wählen</wa-button>
            ${image ? html`<span>${image.name}</span>` : nothing}
          </div>
          ${
            dpi < MIN_DPI
              ? html`
                  <span class="warning">
                    Nur ${Math.round(dpi)} dpi bei dieser Breite, im Druck wird das Bild unscharf.
                    Ein kleineres Maß oder ein größeres Bild (oder eine SVG) hilft.
                  </span>
                `
              : nothing
          }
          <span class="muted">
            PNG, JPG oder SVG bis 5 MB. SVG bleibt in jeder Größe scharf. Ein Bild über die ganze
            Seite (Breite 210 mm, oben links bei 0 und 0) ist ein Briefpapier, Bilder liegen immer
            hinter den anderen Elementen.
          </span>
        `;
      }
      case 'text':
        return html`
          <wa-textarea
            label="Text"
            rows="3"
            hint="**fett**, *kursiv*. Platzhalter: ${PLACEHOLDERS}"
            .value="${part.text}"
            @input="${(e: Event) => {
              part.text = text(e);
              this.changed();
            }}"
          ></wa-textarea>
          <div class="fields styled">
            <wa-select
              class="family"
              label="Schrift"
              .value="${part.family ?? ''}"
              @change="${(e: Event) => {
                part.family = String((e.target as WaSelect).value) || null;
                this.changed();
              }}"
            >
              <wa-option value="">Schrift des Layouts</wa-option>
              ${[...new Set([...this.families, ...BUILTIN_FAMILIES])].map(
                family => html`<wa-option value="${family}">${family}</wa-option>`
              )}
            </wa-select>
            ${this.number('Schriftgröße', part.size, size => (part.size = size ?? 9), 'pt')}
            ${this.align(part.align, align => (part.align = align))} ${this.color(part)}
          </div>
        `;
      case 'page-number':
        return html`
          <wa-input
            label="Text"
            hint="{page} ist die Seite, {pages} die Zahl der Seiten"
            .value="${part.text}"
            @input="${(e: Event) => {
              part.text = text(e);
              this.changed();
            }}"
          ></wa-input>
          <div class="fields styled">
            ${this.number('Schriftgröße', part.size, size => (part.size = size ?? 7.5), 'pt')}
            ${this.align(part.align, align => (part.align = align))} ${this.color(part)}
          </div>
        `;
      case 'line':
        return html`
          <div class="fields styled">
            ${this.number('Stärke', part.thickness, t => (part.thickness = t ?? 0.5), 'pt')}
            ${this.color(part)}
          </div>
        `;
    }
  }

  override render() {
    const part = this.item;
    return html`
      <div class="stack">
        <div class="place">
          <erng-anchor-picker
            .value="${part.anchor}"
            @change="${(e: CustomEvent<Part['anchor']>) => {
              part.anchor = e.detail;
              this.changed();
            }}"
          ></erng-anchor-picker>
          <div class="stack">
            <div class="fields">
              ${this.number('Abstand horizontal', part.x, x => (part.x = x ?? 0))}
              ${this.number('Abstand vertikal', part.y, y => (part.y = y ?? 0))}
              ${this.number('Breite', part.width, w => (part.width = w), 'mm', part.kind !== 'image' && part.kind !== 'line')}
            </div>
          </div>
        </div>
        ${this.kind()}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-part-form': PartFormComponent;
  }
}
