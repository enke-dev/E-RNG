import { html } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { when } from 'lit/directives/when.js';

import { ErngElement } from '../../utils/base.utils.js';
import styles from './page.component.css';

/**
 * The detail of a list: a heading, the content, and the actions at the bottom that stay in reach
 * while the content scrolls.
 *
 * @slot - the content, stacked
 * @slot actions - at the start of the action bar, e.g. a destructive action
 * @slot primary - at the end of the action bar, e.g. save
 *
 * @csspart content - the main content area
 * @csspart footer - the footer containing action slots
 */
@customElement('erng-page')
export class PageComponent extends ErngElement.withStyles(styles) {
  @property()
  heading?: string | undefined;

  /** Hides the surrounding frame, for content that manages its own layout */
  @property({ type: Boolean, reflect: true })
  frameless = false;

  override render() {
    return html`
      <main part="content">
        ${when(this.heading, heading => html`<h2>${heading}</h2>`)}
        <slot></slot>
      </main>
      <footer part="footer">
        <div class="actions"><slot name="actions"></slot></div>
        <slot name="primary"></slot>
      </footer>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-page': PageComponent;
  }
}
