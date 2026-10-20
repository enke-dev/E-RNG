import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import { html, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import { available, contextById, label } from '../../utils/context.utils.js';

const SHARED = 'shared';

/**
 * Whose something is: every context's, or only one context's. Customers, saved positions and
 * layouts are shared unless the user says otherwise. Hidden while there is only one context.
 *
 * @fires scope-change - the context id, or null for all
 */
@customElement('erng-scope-select')
export class ScopeSelectComponent extends ErngElement {
  /** The context id the entry belongs to, null: shared */
  @property({ attribute: false })
  value: number | null = null;

  @property()
  heading = 'Gilt für';

  override render() {
    const list = available.get();
    // an archived context the entry still belongs to stays visible instead of silently turning shared
    const current = contextById(this.value);
    const options = current && !list.includes(current) ? [...list, current] : list;
    if (options.length < 2 && this.value === null) {
      return nothing;
    }
    return html`<wa-select
      label="${this.heading}"
      .value="${this.value === null ? SHARED : String(this.value)}"
      @change="${(e: Event) => {
        const value = String((e.target as WaSelect).value);
        this.dispatchEvent(
          new CustomEvent<number | null>('scope-change', {
            detail: value === SHARED ? null : Number(value),
            bubbles: true,
            composed: true,
          })
        );
      }}"
    >
      <wa-option value="${SHARED}">Alle Kontexte</wa-option>
      ${options.map(c => html`<wa-option value="${String(c.id)}">Nur ${label(c)}</wa-option>`)}
    </wa-select>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-scope-select': ScopeSelectComponent;
  }
}
