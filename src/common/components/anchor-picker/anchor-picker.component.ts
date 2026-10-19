import { html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import type { Anchor } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import styles from './anchor-picker.component.css';

/** The nine anchors in reading order, with how a person says them */
export const ANCHORS: [Anchor, string][] = [
  ['top-left', 'oben links'],
  ['top-center', 'oben Mitte'],
  ['top-right', 'oben rechts'],
  ['middle-left', 'Mitte links'],
  ['center', 'Mitte'],
  ['middle-right', 'Mitte rechts'],
  ['bottom-left', 'unten links'],
  ['bottom-center', 'unten Mitte'],
  ['bottom-right', 'unten rechts'],
];

/**
 * Picks one of the nine points of the page, as a three by three grid.
 *
 * @fires change - `detail` is the picked anchor
 */
@customElement('erng-anchor-picker')
export class AnchorPickerComponent extends ErngElement.withStyles(styles) {
  @property() value: Anchor = 'top-left';

  override connectedCallback() {
    super.connectedCallback();
    this.setAttribute('role', 'radiogroup');
  }

  private pick(anchor: Anchor) {
    this.value = anchor;
    this.dispatchEvent(new CustomEvent<Anchor>('change', { detail: anchor, bubbles: true }));
  }

  override render() {
    return html`<div class="grid">
      ${ANCHORS.map(
        ([anchor, label]) => html`
          <button
            type="button"
            role="radio"
            title="${label}"
            aria-label="${label}"
            aria-checked="${String(anchor === this.value)}"
            @click="${() => this.pick(anchor)}"
          ></button>
        `
      )}
    </div>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-anchor-picker': AnchorPickerComponent;
  }
}
