import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/icon/icon.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import { open } from '@tauri-apps/plugin-dialog';
import { html, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import type { CustomLayout } from '../../utils/api.utils.js';
import { api } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { notifyError } from '../../utils/notify.utils.js';
import styles from './font-list.component.css';

/**
 * The fonts of a layout as a list to add to and remove from, and which family the content is set
 * in. Edits `fonts` and `font` of the given layout.
 *
 * @fires change
 */
@customElement('erng-font-list')
export class FontListComponent extends ErngElement.withStyles(styles) {
  @property({ attribute: false })
  layout!: CustomLayout;

  private changed() {
    this.requestUpdate();
    this.dispatchEvent(new CustomEvent('change', { bubbles: true, composed: true }));
  }

  private async add() {
    try {
      const picked = await open({
        multiple: true,
        directory: false,
        filters: [{ name: 'Schrift', extensions: ['ttf', 'otf', 'ttc', 'otc'] }],
      });
      if (!Array.isArray(picked) || picked.length === 0) {
        return;
      }
      const fonts = await Promise.all(picked.map(path => api.layoutImportFont(path)));
      // the same file twice is one font
      const known = new Set(this.layout.fonts.map(f => f.path));
      this.layout.fonts.push(...fonts.filter(f => !known.has(f.path)));
      this.changed();
    } catch (error) {
      notifyError(error);
    }
  }

  private removeFont(index: number) {
    const [gone] = this.layout.fonts.splice(index, 1);
    // the family is chosen no more when its last file is gone
    if (gone && !this.layout.fonts.some(f => f.family === gone.family)) {
      if (this.layout.font === gone.family) {
        this.layout.font = null;
      }
    }
    this.changed();
  }

  override render() {
    const { fonts } = this.layout;
    const families = [...new Set(fonts.map(f => f.family))];
    return html`
      <div class="stack">
        ${
          fonts.length > 0
            ? html`<div class="fonts">
                ${fonts.map(
                  (font, index) => html`
                    <div class="font">
                      <span class="name">
                        <strong>${font.family}</strong>
                        ${font.style ? html`<span class="muted">${font.style}</span>` : nothing}
                        <span class="muted">${font.path.split('/').pop()}</span>
                      </span>
                      <wa-button
                        appearance="outlined"
                        variant="danger"
                        title="Schrift entfernen"
                        aria-label="Schrift entfernen"
                        @click="${() => this.removeFont(index)}"
                      >
                        <wa-icon name="trash"></wa-icon>
                      </wa-button>
                    </div>
                  `
                )}
              </div>`
            : nothing
        }
        <div>
          <wa-button appearance="outlined" @click="${this.add}">Schriftdatei hinzufügen</wa-button>
        </div>
        ${
          families.length > 1
            ? html`<wa-select
                label="Schrift der Inhalte"
                .value="${this.layout.font ?? families[0] ?? ''}"
                @change="${(e: Event) => {
                  this.layout.font = String((e.target as WaSelect).value);
                  this.changed();
                }}"
              >
                ${families.map(f => html`<wa-option value="${f}">${f}</wa-option>`)}
              </wa-select>`
            : nothing
        }
        <span class="muted">
          TTF oder OTF, alle Schnitte (Regular, Bold, …) einer Familie. Ohne eigene Schrift gilt die
          eingebaute.
        </span>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-font-list': FontListComponent;
  }
}
