import { html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import { ErngElement } from '../../../utils/base.utils.js';
import styles from './list.component.css';

/**
 * A column of `erng-list-item`s and `erng-list-group`s.
 *
 * @slot - the items and groups
 */
@customElement('erng-list')
export class ListComponent extends ErngElement.withStyles(styles) {
  /** Tells assistive technology this is a list of entries */
  @property({ type: String, reflect: true })
  override readonly role = 'list';

  override render() {
    return html`<slot></slot>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-list': ListComponent;
  }
}
