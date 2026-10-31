import { html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import { ErngElement } from '../../../utils/base.utils.js';
import styles from './list-group.component.css';

/**
 * A heading between the entries of a list, with room for an action. A heading that stands for
 * something of its own (a Vorgang) can be picked like an entry (`pickable`).
 *
 * @slot - the grouped list items
 * @slot action - a small button
 * @fires group-pick - the heading of a pickable group was clicked
 */
@customElement('erng-list-group')
export class ListGroupComponent extends ErngElement.withStyles(styles) {
  @property({ type: String, reflect: true })
  heading = '';

  /** Stands for something that is being made and is not stored yet: the heading is set in italics */
  @property({ type: Boolean, reflect: true })
  placeholder = false;

  /** The heading opens what the group stands for */
  @property({ type: Boolean, reflect: true })
  pickable = false;

  /** What the group stands for is shown on the other side */
  @property({ type: Boolean, reflect: true })
  selected = false;

  override render() {
    return html`
      <header>
        ${
          this.pickable
            ? html`
                <button
                  class="heading"
                  @click="${() => this.dispatchEvent(new CustomEvent('group-pick'))}"
                >
                  ${this.heading}
                </button>
              `
            : html`<span>${this.heading}</span>`
        }
        <slot name="action"></slot>
      </header>
      <slot></slot>
    `;
  }
}

declare global {
  interface HTMLElementEventMap {
    'group-pick': CustomEvent<void>;
  }

  interface HTMLElementTagNameMap {
    'erng-list-group': ListGroupComponent;
  }
}
