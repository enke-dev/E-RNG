import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/dropdown/dropdown.js';
import '@awesome.me/webawesome/dist/components/dropdown-item/dropdown-item.js';

import { html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import styles from './brand-select.component.css';

/** The palettes of the UI library a context can be tinted with, see `BRANDS` in `context.rs` */
export const BRANDS = [
  ['blue', 'Blau'],
  ['red', 'Rot'],
  ['orange', 'Orange'],
  ['yellow', 'Gelb'],
  ['green', 'Grün'],
  ['cyan', 'Türkis'],
  ['indigo', 'Indigo'],
  ['purple', 'Violett'],
  ['pink', 'Pink'],
] as const;

const swatch = (name: string) =>
  html`<span class="swatch" style="--swatch: var(--wa-color-${name}-50)"></span>`;

/**
 * Picks the brand color of a context: a small dropdown that offers the palettes as squares. The
 * names are only there for assistive technology and as tooltips.
 *
 * @fires brand-change - the palette name
 */
@customElement('erng-brand-select')
export class BrandSelectComponent extends ErngElement.withStyles(styles) {
  @property()
  value = '';

  override render() {
    const name = BRANDS.find(([key]) => key === this.value)?.[1] ?? '';
    return html`
      <wa-dropdown
        placement="bottom-end"
        @wa-select="${(e: CustomEvent<{ item: HTMLElement & { value: string } }>) =>
          this.dispatchEvent(
            new CustomEvent<string>('brand-change', { detail: e.detail.item.value })
          )}"
      >
        <wa-button
          slot="trigger"
          appearance="outlined"
          with-caret
          aria-label="Farbe: ${name}"
          title="Farbe der Oberfläche in diesem Kontext"
        >
          ${swatch(this.value)}
        </wa-button>
        ${BRANDS.map(
          ([key, caption]) => html`
            <wa-dropdown-item value="${key}" aria-label="${caption}" title="${caption}">
              ${swatch(key)}
            </wa-dropdown-item>
          `
        )}
      </wa-dropdown>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-brand-select': BrandSelectComponent;
  }
}
