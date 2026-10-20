import { html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import { ErngElement } from '../../../utils/base.utils.js';
import styles from './list-group.component.css';

/**
 * A heading between the entries of a list, with room for an action.
 *
 * @slot - the grouped list items
 * @slot action - a small button
 */
@customElement('erng-list-group')
export class ListGroupComponent extends ErngElement.withStyles(styles) {
  @property({ type: String, reflect: true })
  heading = '';

  /** Stands for something that is being made and is not stored yet: the heading is set in italics */
  @property({ type: Boolean, reflect: true })
  placeholder = false;

  override render() {
    return html`
      <header>
        <span>${this.heading}</span>
        <slot name="action"></slot>
      </header>
      <slot></slot>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-list-group': ListGroupComponent;
  }
}
