import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/button-group/button-group.js';

import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import styles from './sheet.component.css';

/**
 * The pages of a rendered document (SVG strings) as a sheet of paper, one page at a time with a
 * pager. It is as tall as the pane around it allows (`cqb` of the nearest size container) minus
 * `--reserve`, and stays in view while that pane scrolls.
 *
 * @cssprop --reserve - height of what covers the bottom of the pane
 */
@customElement('erng-sheet')
export class SheetComponent extends ErngElement.withStyles(styles) {
  /** One SVG document per page */
  @property({ attribute: false }) pages: string[] = [];

  /** Shown instead of a page while there is none: the state, or why there is none */
  @property() hint = '';

  @state() private current = 0;

  /** Marks a document with more than one page, the styles leave room for the pager */
  @property({ type: Boolean, reflect: true }) paged = false;

  protected override willUpdate(changed: PropertyValues<this>) {
    if (changed.has('pages')) {
      this.paged = this.pages.length > 1;
      this.current = Math.min(this.current, Math.max(0, this.pages.length - 1));
    }
  }

  override render() {
    const page = this.pages[this.current];
    return html`
      <div class="page">
        ${
          page
            ? html`<img
                alt="Seite ${this.current + 1} von ${this.pages.length}"
                src="data:image/svg+xml;charset=utf-8,${encodeURIComponent(page)}"
              >`
            : html`<span class="hint">${this.hint}</span>`
        }
      </div>
      ${
        this.paged
          ? html`<wa-button-group label="Seiten">
              <wa-button
                appearance="outlined"
                size="s"
                ?disabled="${this.current === 0}"
                @click="${() => (this.current -= 1)}"
                >Zurück</wa-button
              >
              <wa-button appearance="outlined" size="s" disabled>
                Seite ${this.current + 1} von ${this.pages.length}
              </wa-button>
              <wa-button
                appearance="outlined"
                size="s"
                ?disabled="${this.current >= this.pages.length - 1}"
                @click="${() => (this.current += 1)}"
                >Weiter</wa-button
              >
            </wa-button-group>`
          : nothing
      }
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-sheet': SheetComponent;
  }
}
