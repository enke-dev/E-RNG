import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';
import '@awesome.me/webawesome/dist/components/icon/icon.js';

import { html, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import type { Deduction, OnAccountInvoice } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { deductionOf } from '../../utils/billing.utils.js';
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
    const deductions = on ? [...kept, deductionOf(invoice)] : kept;
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

  /**
   * The picked ones not paid completely yet: deducted as invoiced all the same, what is missing stays
   * open on them (§ 14 Abs. 5 UStG speaks of what was received, the warning says so)
   */
  private renderUnpaid(picked: Set<number>) {
    const unpaid = this.available.filter(i => picked.has(i.id) && i.paid_at === null);
    if (unpaid.length === 0) {
      return nothing;
    }
    const names = unpaid.map(i => {
      const open = Number(i.gross) - Number(i.paid ?? 0);
      return `${i.number} (offen ${money(open.toFixed(2))})`;
    });
    return html`
      <wa-callout variant="warning" size="s">
        <wa-icon slot="icon" name="warning"></wa-icon>
        Noch nicht ganz bezahlt: ${names.join(', ')}. Abgezogen wird, was berechnet ist; der Rest
        bleibt auf ${unpaid.length === 1 ? 'dieser Rechnung' : 'diesen Rechnungen'} offen.
      </wa-callout>
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
                invoice.paid_at
                  ? ' · bezahlt'
                  : invoice.paid
                    ? ` · ${money(invoice.paid)} bezahlt`
                    : ' · unbezahlt'
              }
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
      ${this.renderUnpaid(picked)}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-deductions': DeductionsComponent;
  }
}
