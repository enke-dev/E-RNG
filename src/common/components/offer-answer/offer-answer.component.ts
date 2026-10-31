import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { Answer, StoredInvoice } from '../../utils/api.utils.js';
import { api } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { text, today } from '../../utils/form.utils.js';
import { notifyError } from '../../utils/notify.utils.js';

/**
 * What the customer said to a created offer: still open, accepted or declined, each with its day.
 * Changing it is noted in the offer's history.
 *
 * @fires invoice-updated - the offer as the store has it now is the `detail`
 */
@customElement('erng-offer-answer')
export class OfferAnswerComponent extends ErngElement {
  @property({ attribute: false }) offer!: StoredInvoice;

  @state() private busy = false;

  private async set(answer: Answer | null, on: string | null) {
    this.busy = true;
    try {
      const offer = await api.offerSetAnswer(this.offer.id, answer, on);
      this.dispatchEvent(new CustomEvent('invoice-updated', { detail: offer }));
    } catch (error) {
      notifyError(error);
    } finally {
      this.busy = false;
    }
  }

  override render() {
    const { answer, answered_on } = this.offer;
    return html`
      <div class="grid">
        <wa-select
          label="Antwort des Kunden"
          ?disabled="${this.busy}"
          .value="${answer ?? 'open'}"
          @change="${(e: Event) => {
            const value = String((e.target as WaSelect).value);
            void this.set(value === 'open' ? null : (value as Answer), answered_on ?? today());
          }}"
        >
          <wa-option value="open">Noch offen</wa-option>
          <wa-option value="accepted">Angenommen</wa-option>
          <wa-option value="declined">Abgelehnt</wa-option>
        </wa-select>
        ${
          answer
            ? html`
                <wa-input
                  type="date"
                  label="${answer === 'accepted' ? 'Angenommen am' : 'Abgelehnt am'}"
                  ?disabled="${this.busy}"
                  .value="${answered_on ?? ''}"
                  @change="${(e: Event) => text(e) && void this.set(answer, text(e))}"
                ></wa-input>
              `
            : nothing
        }
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-offer-answer': OfferAnswerComponent;
  }
}
