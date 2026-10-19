import '@awesome.me/webawesome/dist/components/split-panel/split-panel.js';

import { html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import styles from './editor.layout.css';

/** The room each side needs at the least, in rem */
const FORM_MIN = 25;
const PREVIEW_MIN = 20;

/**
 * A form with the document it edits beside it, the shape of the invoice and the layout editors.
 * The divider between them is the user's to move (not remembered), the preview scales with its
 * panel. Where the two minimums do not fit, only the form is shown.
 *
 * @slot form - the fields
 * @slot preview - an `erng-sheet`
 */
@customElement('erng-editor')
export class EditorLayout extends ErngElement.withStyles(styles) {
  /** Only the form, there is no room for both */
  @property({ type: Boolean, reflect: true }) compact = false;

  #observer = new ResizeObserver(([entry]) => {
    const rem = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
    this.compact = (entry?.contentRect.width ?? Infinity) < (FORM_MIN + PREVIEW_MIN) * rem;
  });

  override connectedCallback() {
    super.connectedCallback();
    this.#observer.observe(this);
  }

  override disconnectedCallback() {
    this.#observer.disconnect();
    super.disconnectedCallback();
  }

  override render() {
    if (this.compact) {
      return html`<div class="form"><slot name="form"></slot></div>`;
    }
    return html`
      <wa-split-panel
        primary="start"
        position="60"
        style="--form-min: ${FORM_MIN}rem; --preview-min: ${PREVIEW_MIN}rem"
      >
        <div slot="start" class="form"><slot name="form"></slot></div>
        <div slot="end" class="preview"><slot name="preview"></slot></div>
      </wa-split-panel>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-editor': EditorLayout;
  }
}
