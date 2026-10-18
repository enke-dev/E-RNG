import { html } from 'lit';
import { customElement } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import styles from './split.layout.css';

/**
 * A list on the left and the selected entry on the right, the shape of the invoices and the
 * settings. Both panes scroll on their own, the detail pane is the size container its content may
 * query (`cqb`).
 *
 * @slot list - an `erng-filter-bar`, `wa-callout` notices and an `erng-list`, in that order
 * @slot detail - an `erng-page` or a notice
 */
@customElement('erng-split')
export class SplitLayout extends ErngElement.withStyles(styles) {
  override render() {
    return html`
      <slot name="list"></slot>
      <slot name="detail"></slot>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-split': SplitLayout;
  }
}
