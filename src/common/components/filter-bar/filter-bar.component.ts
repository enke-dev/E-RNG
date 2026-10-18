import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/input/input.js';

import type { CSSResultGroup } from 'lit';
import { html } from 'lit';
import { customElement, eventOptions, property } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import { text } from '../../utils/form.utils.js';
import styles from './filter-bar.component.css';

/**
 * A search field with room for filters and actions, as the header of lists and card views. It lays
 * itself out by the width of its container: one column when narrow, one row when wide.
 *
 * @slot filters - filter controls (selects, toggles)
 * @slot actions - buttons, e.g. "new"
 * @fires query-change - `detail` is the search text
 */
@customElement('erng-filter-bar')
export class FilterBarComponent extends ErngElement {
  static override styles: CSSResultGroup = [ErngElement.styles, styles];

  @property() query = '';

  @property() placeholder = 'Suchen …';

  /** Filters as the user types, the button and Enter are for those who expect to ask for a search */
  @eventOptions({ passive: true })
  private handleInput(event: Event) {
    this.query = text(event);
    this.announce();
  }

  private announce() {
    this.dispatchEvent(new CustomEvent<string>('query-change', { detail: this.query }));
  }

  private handleKey(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      this.announce();
    }
  }

  override render() {
    return html`
      <section>
        <wa-input
          type="search"
          with-clear
          aria-label="Suchen"
          placeholder="${this.placeholder}"
          .value="${this.query}"
          @input="${this.handleInput}"
          @wa-clear="${this.handleInput}"
          @keydown="${this.handleKey}"
        ></wa-input>
        <slot name="filters"></slot>
        <slot name="actions"></slot>
      </section>
    `;
  }
}

declare global {
  interface HTMLElementEventMap {
    'query-change': CustomEvent<string>;
  }

  interface HTMLElementTagNameMap {
    'erng-filter-bar': FilterBarComponent;
  }
}
