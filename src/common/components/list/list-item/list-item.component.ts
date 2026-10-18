import { html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import { ErngElement } from '../../../utils/base.utils.js';
import styles from './list-item.component.css';

/**
 * One entry of a list: a name, an amount beside it and a second line, all optional but the name.
 * Click it as you would a button.
 *
 * @slot - the name
 * @slot end - beside the name, right aligned (an amount)
 * @slot sub - second line, quiet (a number, a place)
 * @slot sub-end - second line, right aligned (a date)
 * @csspart button - the clickable surface
 */
@customElement('erng-list-item')
export class ListItemComponent extends ErngElement.withStyles(styles) {
  /** The entry that is shown on the other side */
  @property({ type: Boolean, reflect: true }) selected = false;

  /** Kept for what was made with it, but no longer offered */
  @property({ type: Boolean, reflect: true }) archived = false;

  /** Tells assistive technology this is an entry of the list, the button inside does the rest */
  @property({ type: String, reflect: true })
  override readonly role = 'listitem';

  override render() {
    return html`
      <button part="button" type="button" aria-current="${this.selected ? 'true' : 'false'}">
        <span class="name"><slot></slot></span>
        <slot name="end"></slot>
        <slot name="sub"></slot>
        <slot name="sub-end"></slot>
      </button>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-list-item': ListItemComponent;
  }
}
