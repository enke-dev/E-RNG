import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/button-group/button-group.js';

import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import type { PartBox } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import styles from './sheet.component.css';

/** What a region is called: the setup it is on and its place in the list of parts */
export const regionKey = (region: Pick<PartBox, 'first' | 'index'>) =>
  `${region.first ? 'first' : 'page'}:${region.index}`;

/**
 * The pages of a rendered document (SVG strings) as a sheet of paper, one page at a time with a
 * pager. It is as tall as the pane around it allows (`cqb` of the nearest size container) minus
 * `--reserve`, and stays in view while that pane scrolls.
 *
 * @cssprop --reserve - height of what covers the bottom of the pane
 * @fires page-change - `detail` is the page shown after the pager moved, the first is 0
 * @fires region-hover - `detail` is the key of the region under the pointer, or null
 * @fires region-click - `detail` is the key of the region that was clicked
 */
@customElement('erng-sheet')
export class SheetComponent extends ErngElement.withStyles(styles) {
  /** One SVG document per page */
  @property({ attribute: false }) pages: string[] = [];

  /** Shown instead of a page while there is none: the state, or why there is none */
  @property() hint = '';

  /** Where the parts of a layout are: what is under the pointer is the part to edit */
  @property({ attribute: false }) regions: PartBox[] = [];

  /** The selected region, as `regionKey` says */
  @property() active: string | null = null;

  /** The region the pointer is over somewhere (here or in the form), as `regionKey` says */
  @property() hover: string | null = null;

  /** A region to bring into view, the token tells a repeated request from the same one */
  @property({ attribute: false }) reveal: { key: string; token: number } | undefined;

  /** The page shown, the first is 0; the pager moves it */
  @property({ type: Number }) page = 0;

  /** Marks a document with more than one page, the styles leave room for the pager */
  @property({ type: Boolean, reflect: true }) paged = false;

  protected override updated(changed: PropertyValues<this>) {
    if (changed.has('reveal') && this.reveal) {
      this.bringIntoView(this.reveal.key);
    }
  }

  /** Scrolls to a region, it is in the document after the next frame */
  private bringIntoView(key: string) {
    requestAnimationFrame(() =>
      this.renderRoot
        .querySelector(`[data-key="${key}"]`)
        ?.scrollIntoView({ block: 'center', behavior: 'smooth' })
    );
  }

  protected override willUpdate(changed: PropertyValues<this>) {
    if (changed.has('pages')) {
      this.paged = this.pages.length > 1;
      this.page = Math.min(this.page, Math.max(0, this.pages.length - 1));
    }
  }

  /** The pager: another page is shown, the surroundings may want to follow */
  private turn(by: number) {
    this.page += by;
    this.dispatchEvent(new CustomEvent<number>('page-change', { detail: this.page }));
  }

  private emit(name: string, key: string | null) {
    this.dispatchEvent(new CustomEvent<string | null>(name, { detail: key }));
  }

  override render() {
    const page = this.pages[this.page];
    return html`
      <div class="page">
        ${
          page
            ? html`<img
                alt="Seite ${this.page + 1} von ${this.pages.length}"
                src="data:image/svg+xml;charset=utf-8,${encodeURIComponent(page)}"
              >`
            : html`<span class="hint">${this.hint}</span>`
        }
        ${this.regions
          .filter(region => region.page === this.page + 1)
          .map(region => {
            const key = regionKey(region);
            // a rule has no height, it still needs something to point at
            const height = Math.max(region.h, 4);
            return html`<button
              type="button"
              class="region ${key === this.active ? 'active' : key === this.hover ? 'hover' : ''}"
              style="left: ${(region.x / 210) * 100}%; top: ${(region.y / 297) * 100}%; width: ${(region.w / 210) * 100}%; height: ${(height / 297) * 100}%"
              data-key="${key}"
              aria-label="Teil bearbeiten"
              @mouseenter="${() => this.emit('region-hover', key)}"
              @mouseleave="${() => this.emit('region-hover', null)}"
              @click="${() => this.emit('region-click', key)}"
            ></button>`;
          })}
      </div>
      ${
        this.paged
          ? html`<wa-button-group label="Seiten">
              <wa-button
                appearance="outlined"
                size="s"
                ?disabled="${this.page === 0}"
                @click="${() => this.turn(-1)}"
                >Zurück</wa-button
              >
              <wa-button appearance="outlined" size="s" disabled>
                Seite ${this.page + 1} von ${this.pages.length}
              </wa-button>
              <wa-button
                appearance="outlined"
                size="s"
                ?disabled="${this.page >= this.pages.length - 1}"
                @click="${() => this.turn(1)}"
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
