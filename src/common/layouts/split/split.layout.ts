import '@awesome.me/webawesome/dist/components/split-panel/split-panel.js';

import type WaSplitPanel from '@awesome.me/webawesome/dist/components/split-panel/split-panel.js';
import { html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import { recall, remember } from '../../utils/memory.utils.js';
import styles from './split.layout.css';

/**
 * A list on the left and the selected entry on the right, the shape of the invoices and the
 * settings. The divider between them is the user's to move, its position (percent) is remembered
 * under its `position-key`, within the limits of the list. Both panes scroll on their
 * own, the detail pane is the size container its content may query (`cqb`).
 *
 * @slot list - an `erng-filter-bar`, `wa-callout` notices and an `erng-list`, in that order
 * @slot detail - an `erng-page` or a notice
 */
@customElement('erng-split')
export class SplitLayout extends ErngElement.withStyles(styles) {
  /** The name the position of the divider is stored under, one per view; none: not stored */
  @property({ attribute: 'position-key' }) positionKey = '';

  override render() {
    return html`
      <wa-split-panel
        position="${(this.positionKey ? recall(this.positionKey) : undefined) ?? 22}"
        @wa-reposition="${(e: Event) =>
          this.positionKey &&
          remember(this.positionKey, (e.currentTarget as WaSplitPanel).position)}"
      >
        <slot slot="start" name="list"></slot>
        <slot slot="end" name="detail"></slot>
      </wa-split-panel>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-split': SplitLayout;
  }
}
