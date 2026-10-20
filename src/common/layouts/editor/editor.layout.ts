import '@awesome.me/webawesome/dist/components/split-panel/split-panel.js';

import type WaSplitPanel from '@awesome.me/webawesome/dist/components/split-panel/split-panel.js';
import { html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import { recall, remember } from '../../utils/memory.utils.js';
import styles from './editor.layout.css';

/**
 * A form with the document it edits beside it, the shape of the invoice and the layout editors.
 * The divider between them is the user's to move, its position (percent) is remembered under
 * its `position-key`; the preview scales with its panel. Where the two minimums do not fit, only the
 * form is shown.
 *
 * @slot form - the fields
 * @slot preview - an `erng-sheet`
 */
@customElement('erng-editor')
export class EditorLayout extends ErngElement.withStyles(styles) {
  /** The name the position of the divider is stored under, one per view; none: not stored */
  @property({ attribute: 'position-key' }) positionKey = '';

  override render() {
    return html`
      <wa-split-panel
        position="${(this.positionKey ? recall(this.positionKey) : undefined) ?? 60}"
        @wa-reposition="${(e: Event) =>
          this.positionKey &&
          remember(this.positionKey, (e.currentTarget as WaSplitPanel).position)}"
      >
        <slot slot="start" name="form"></slot>
        <slot slot="end" name="preview"></slot>
      </wa-split-panel>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-editor': EditorLayout;
  }
}
