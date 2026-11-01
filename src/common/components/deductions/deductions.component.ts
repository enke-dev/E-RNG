import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';

import { html, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import type { Deduction, OnAccountInvoice } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { formatDate, money } from '../../utils/format.utils.js';
import styles from './deductions.component.css';

const KIND = { prepayment: 'Anzahlung', progress: 'Abschlag' } as const;

/**
 * The invoices on account a Schlussrechnung deducts. A draft picks from the created ones of its
 * Vorgang (one another created invoice deducts already cannot be picked), a created one lists what
 * it deducted.
 *
 * @fires deductions-change - the deductions picked now are the `detail`, the store fills in their figures
 */
@customElement('erng-deductions')
export class DeductionsComponent extends ErngElement.withStyles(styles) {
  @property({ attribute: false }) available: OnAccountInvoice[] = [];
  @property({ attribute: false }) deductions: Deduction[] = [];
  @property({ type: Boolean }) locked = false;

  private toggle(invoice: OnAccountInvoice, on: boolean) {
    const kept = this.deductions.filter(d => d.invoice_id !== invoice.id);
    const deductions = on
      ? [
          ...kept,
          {
            invoice_id: invoice.id,
            number: invoice.number,
            issue_date: invoice.issue_date,
            net: invoice.net,
            vat: invoice.vat,
            gross: invoice.gross,
          },
        ]
      : kept;
    this.dispatchEvent(new CustomEvent<Deduction[]>('deductions-change', { detail: deductions }));
  }

  private renderLocked() {
    return html`
      <ul>
        ${this.deductions.map(
          d => html`
            <li>
              <span>${d.number} vom ${formatDate(d.issue_date)}</span>
              <span class="amount">${money(d.gross)}</span>
            </li>
          `
        )}
      </ul>
    `;
  }

  override render() {
    if (this.locked) {
      return this.renderLocked();
    }
    const picked = new Set(this.deductions.map(d => d.invoice_id));
    const total = this.deductions.reduce((sum, d) => sum + Number(d.gross || 0), 0);
    return html`
      ${this.available.map(invoice => {
        // a created invoice deducts it for good, a draft only says so
        const taken = invoice.deducted_by !== null && invoice.deducted_by !== 'Entwurf';
        return html`
          <wa-checkbox
            ?checked="${picked.has(invoice.id)}"
            ?disabled="${taken && !picked.has(invoice.id)}"
            @change="${(e: Event) => this.toggle(invoice, (e.target as HTMLInputElement).checked)}"
          >
            ${KIND[invoice.on_account]} ${invoice.number} vom ${formatDate(invoice.issue_date)}
            <span class="muted">
              ${money(invoice.gross)}
              ${
                taken
                  ? ` · abgezogen in ${invoice.deducted_by}`
                  : invoice.deducted_by === 'Entwurf'
                    ? ' · auch in einem Entwurf'
                    : nothing
              }
            </span>
          </wa-checkbox>
        `;
      })}
      ${
        this.deductions.length > 0
          ? html`<p class="muted">Abgezogen werden ${money(total.toFixed(2))} brutto.</p>`
          : nothing
      }
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-deductions': DeductionsComponent;
  }
}
