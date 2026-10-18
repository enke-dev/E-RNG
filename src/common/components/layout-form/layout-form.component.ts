import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';
import '@awesome.me/webawesome/dist/components/input/input.js';

import { open } from '@tauri-apps/plugin-dialog';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { CustomLayout, Insets } from '../../utils/api.utils.js';
import { api } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { text } from '../../utils/form.utils.js';
import { notifyError } from '../../utils/notify.utils.js';
import styles from './layout-form.component.css';

const EDGES: [keyof Insets, string][] = [
  ['top', 'Oben'],
  ['right', 'Rechts'],
  ['bottom', 'Unten'],
  ['left', 'Links'],
];

/** The fields of an own layout, edits the given object and fires `change` */
@customElement('erng-layout-form')
export class LayoutFormComponent extends ErngElement.withStyles(styles) {
  @property({ attribute: false })
  layout!: CustomLayout;

  @state() private busy = false;

  private changed() {
    this.requestUpdate();
    this.dispatchEvent(new CustomEvent('change', { bubbles: true, composed: true }));
  }

  /** Runs a pick-and-convert step: the files are read by the app, which can fail */
  private async importing(step: () => Promise<void>) {
    this.busy = true;
    try {
      await step();
      this.changed();
    } catch (error) {
      notifyError(error);
    } finally {
      this.busy = false;
    }
  }

  private chooseBackground() {
    return this.importing(async () => {
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'PDF', extensions: ['pdf'] }],
      });
      if (typeof picked === 'string') {
        this.layout.background = await api.layoutImportBackground(picked);
      }
    });
  }

  private chooseFonts() {
    return this.importing(async () => {
      const picked = await open({
        multiple: true,
        directory: false,
        filters: [{ name: 'Schrift', extensions: ['ttf', 'otf', 'ttc', 'otc'] }],
      });
      if (!Array.isArray(picked) || picked.length === 0) {
        return;
      }
      const fonts = await Promise.all(picked.map(path => api.layoutImportFont(path)));
      if (fonts.some(f => f.family !== fonts[0]?.family)) {
        throw new Error('Alle Dateien müssen zur selben Schriftfamilie gehören.');
      }
      this.layout.fonts = fonts;
    });
  }

  private edge(insets: Insets, key: keyof Insets, label: string) {
    return html`<wa-input
      label="${label}"
      type="number"
      min="0"
      step="1"
      .value="${String(insets[key])}"
      @input="${(e: Event) => {
        insets[key] = Math.max(0, Number(text(e)) || 0);
        this.changed();
      }}"
    >
      <span slot="end">mm</span>
    </wa-input>`;
  }

  override render() {
    const l = this.layout;
    const first = l.first_margins;
    const family = l.fonts[0]?.family;
    return html`
      <div class="stack">
        <wa-input
          label="Name"
          required
          .value="${l.name}"
          @input="${(e: Event) => {
            l.name = text(e);
            this.changed();
          }}"
        ></wa-input>

        <div class="stack">
          <strong>Briefpapier</strong>
          <div class="file">
            <wa-button
              appearance="outlined"
              ?disabled="${this.busy}"
              @click="${this.chooseBackground}"
            >
              PDF wählen
            </wa-button>
            ${
              l.background
                ? html`<span>${l.background.name}</span>
                    <wa-button
                      appearance="outlined"
                      @click="${() => {
                        l.background = null;
                        this.changed();
                      }}"
                      >Entfernen</wa-button
                    >`
                : nothing
            }
          </div>
          <span class="muted">
            ${
              l.background?.rest
                ? 'Seite 1 liegt hinter der ersten Seite der Rechnung, Seite 2 hinter allen weiteren.'
                : 'Die PDF-Seite liegt hinter allen Seiten der Rechnung. Mit einer zweiten PDF-Seite bekommen die Folgeseiten ein eigenes Briefpapier.'
            }
          </span>
        </div>

        <div class="stack">
          <strong>Rand für den Inhalt</strong>
          <div class="edges">${EDGES.map(([key, label]) => this.edge(l.margins, key, label))}</div>
          <wa-checkbox
            ?checked="${first !== null}"
            @change="${(e: Event) => {
              l.first_margins = (e.target as HTMLInputElement).checked ? { ...l.margins } : null;
              this.changed();
            }}"
          >
            Erste Seite weicht ab
          </wa-checkbox>
          ${
            first
              ? html`<div class="edges">
                  ${EDGES.map(([key, label]) => this.edge(first, key, label))}
                </div>`
              : nothing
          }
        </div>

        <div class="stack">
          <strong>Schrift</strong>
          <div class="file">
            <wa-button appearance="outlined" ?disabled="${this.busy}" @click="${this.chooseFonts}">
              Schriftdateien wählen
            </wa-button>
            ${
              family
                ? html`<span
                      >${family} (${l.fonts.length}
                      ${l.fonts.length === 1 ? 'Datei' : 'Dateien'})</span
                    >
                    <wa-button
                      appearance="outlined"
                      @click="${() => {
                        l.fonts = [];
                        this.changed();
                      }}"
                      >Entfernen</wa-button
                    >`
                : nothing
            }
          </div>
          <span class="muted">
            TTF oder OTF. Alle Schnitte einer Familie auf einmal wählen (Regular, Bold, …).
          </span>
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-layout-form': LayoutFormComponent;
  }
}
