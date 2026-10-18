import { html } from 'lit';
import { customElement } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import styles from './editor.layout.css';

/**
 * A form with the document it edits beside it, the shape of the invoice and the layout editors.
 * The preview is sized by the pane around it, an `erng-sheet` fits right in.
 *
 * @slot form - the fields
 * @slot preview - an `erng-sheet`
 */
@customElement('erng-editor')
export class EditorLayout extends ErngElement.withStyles(styles) {
  override render() {
    return html`
      <slot name="form"></slot>
      <slot name="preview"></slot>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-editor': EditorLayout;
  }
}
