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

  /** Stands for something that is being made and is not stored yet: set in italics, not clickable */
  @property({ type: Boolean, reflect: true }) placeholder = false;

  /** Kept for what was made with it, but no longer offered */
  @property({ type: Boolean, reflect: true }) archived = false;

  /** Tells assistive technology this is an entry of the list, the button inside does the rest */
  @property({ type: String, reflect: true })
  override readonly role = 'listitem';

  /** The grid places the slots by what is next to them: the host says which ones are filled */
  private noted(attribute: string) {
    return (e: Event) =>
      this.toggleAttribute(attribute, (e.target as HTMLSlotElement).assignedElements().length > 0);
  }

  override render() {
    return html`
      <button
        part="button"
        type="button"
        aria-current="${this.selected ? 'true' : 'false'}"
        ?disabled="${this.placeholder}"
      >
        <span class="name"><slot></slot></span>
        <slot name="end" @slotchange="${this.noted('has-end')}"></slot>
        <slot name="sub" @slotchange="${this.noted('has-sub')}"></slot>
        <slot name="sub-end" @slotchange="${this.noted('has-sub-end')}"></slot>
      </button>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-list-item': ListItemComponent;
  }
}
