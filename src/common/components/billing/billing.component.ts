import { html, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import type { OfferBilling } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { amount } from '../../utils/billing.utils.js';
import { unitLabel } from '../../utils/units.utils.js';
import styles from './billing.component.css';

/**
 * What is billed of a created offer, position by position: how many it offers, how many created
 * invoices billed and what is open; what invoice drafts hold is said apart, they bill nothing yet.
 */
@customElement('erng-billing')
export class BillingComponent extends ErngElement.withStyles(styles) {
  @property({ attribute: false }) billing: OfferBilling | undefined;

  override render() {
    const billing = this.billing;
    if (!billing) {
      return nothing;
    }
    const open = billing.positions.filter(p => Number(p.open) > 0).length;
    return html`
      <table>
        <thead>
          <tr>
            <th>Position</th>
            <th class="number">Angeboten</th>
            <th class="number">Berechnet</th>
            <th class="number">Offen</th>
          </tr>
        </thead>
        <tbody>
          ${billing.positions.map(
            p => html`
              <tr class="${Number(p.open) > 0 ? '' : 'done'}">
                <td>
                  ${p.index + 1}. ${p.position.description || '–'}
                  ${
                    Number(p.drafted) > 0
                      ? html`<span class="muted">(${amount(p.drafted)} in Entwürfen)</span>`
                      : nothing
                  }
                </td>
                <td class="number">
                  ${amount(p.position.quantity ?? '1')} ${unitLabel(p.position)}
                </td>
                <td class="number">${amount(p.billed)}</td>
                <td class="number">${amount(p.open)}</td>
              </tr>
            `
          )}
        </tbody>
      </table>
      <p class="muted">
        ${
          open === 0
            ? 'Alles ist in Rechnung gestellt.'
            : open === billing.positions.length && billing.invoices.length === 0
              ? 'Noch nichts in Rechnung gestellt.'
              : `${open} von ${billing.positions.length} Positionen ${open === 1 ? 'ist' : 'sind'} noch offen.`
        }
      </p>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-billing': BillingComponent;
  }
}
