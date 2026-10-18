import { html } from 'lit';
import { customElement } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import styles from './cards.layout.css';

/**
 * A wide header with the cards below in a grid, the shape of the customers and the templates.
 * The dialog that edits a card is handed in, so the view keeps owning it.
 *
 * @slot header - an `erng-filter-bar`
 * @slot notice - `wa-callout` notices between header and cards
 * @slot cards - the `wa-card`s, each opens the dialog when clicked
 * @slot dialog - the `wa-dialog` of the selected card
 */
@customElement('erng-cards')
export class CardsLayout extends ErngElement.withStyles(styles) {
  override render() {
    return html`
      <div class="header"><slot name="header"></slot></div>
      <slot name="notice"></slot>
      <div class="cards"><slot name="cards"></slot></div>
      <slot name="dialog"></slot>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-cards': CardsLayout;
  }
}
