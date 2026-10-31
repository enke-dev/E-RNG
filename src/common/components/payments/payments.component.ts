import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/icon/icon.js';
import '@awesome.me/webawesome/dist/components/input/input.js';

import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { StoredInvoice } from '../../utils/api.utils.js';
import { api } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { text, today } from '../../utils/form.utils.js';
import { formatDate, money } from '../../utils/format.utils.js';
import { notifyError } from '../../utils/notify.utils.js';
import styles from './payments.component.css';

/** Cents, so sums of decimals stay exact */
const cents = (amount: string | null | undefined) => Math.round(Number(amount ?? 0) * 100);

/** What is still open on an invoice, as a decimal; nothing below zero */
export function openAmount(invoice: StoredInvoice): string {
  const paid = invoice.payments.reduce((sum, p) => sum + cents(p.amount), 0);
  return (Math.max(0, cents(invoice.total) - paid) / 100).toFixed(2);
}

/** A typed amount as a number, either way it is written (`1.130,50`, `1130.50`) */
const parsed = (typed: string) => {
  const text = typed.trim();
  return Number(text.includes(',') ? text.replace(/\./g, '').replace(',', '.') : text);
};

/** `630.5` as the field shows it: `630,50` */
const typed = (amount: string) => amount.replace('.', ',');

/**
 * The payments of a created invoice, each with its amount and day, and what is still open.
 * `Zahlung erfassen` starts with what is open and takes no more; less is a part payment. A
 * payment is taken back by its remove button.
 *
 * @fires invoice-updated - the invoice as the store has it now is the `detail`
 */
@customElement('erng-payments')
export class PaymentsComponent extends ErngElement.withStyles(styles) {
  @property({ attribute: false }) invoice!: StoredInvoice;

  /** The form of a part payment, open while it is typed */
  @state() private adding = false;
  @state() private amount = '';
  @state() private paidOn = today();
  @state() private busy = false;

  private async run(call: () => Promise<StoredInvoice>) {
    this.busy = true;
    try {
      const invoice = await call();
      this.dispatchEvent(new CustomEvent('invoice-updated', { detail: invoice }));
      return true;
    } catch (error) {
      notifyError(error);
      return false;
    } finally {
      this.busy = false;
    }
  }

  private startAdding() {
    this.amount = typed(openAmount(this.invoice));
    this.paidOn = today();
    this.adding = true;
  }

  /** More than is open becomes what is open */
  private capAmount() {
    const open = Number(openAmount(this.invoice));
    if (parsed(this.amount) > open) {
      this.amount = typed(open.toFixed(2));
    }
  }

  private async add() {
    const id = this.invoice.id;
    if (await this.run(() => api.invoiceAddPayment(id, this.amount, this.paidOn))) {
      this.adding = false;
    }
  }

  private renderForm() {
    return html`
      <div class="form">
        <wa-input
          label="Betrag (brutto)"
          inputmode="decimal"
          size="s"
          hint="Höchstens ${money(openAmount(this.invoice))}, weniger ist eine Teilzahlung."
          .value="${this.amount}"
          @input="${(e: Event) => (this.amount = text(e))}"
          @change="${this.capAmount}"
        ></wa-input>
        <wa-input
          type="date"
          label="Eingegangen am"
          size="s"
          .value="${this.paidOn}"
          @input="${(e: Event) => (this.paidOn = text(e))}"
        ></wa-input>
        <div class="actions">
          <wa-button size="s" appearance="outlined" @click="${() => (this.adding = false)}">
            Abbrechen
          </wa-button>
          <wa-button
            size="s"
            variant="brand"
            ?loading="${this.busy}"
            ?disabled="${!(parsed(this.amount) > 0)}"
            @click="${() => {
              this.capAmount();
              void this.add();
            }}"
          >
            Erfassen
          </wa-button>
        </div>
      </div>
    `;
  }

  override render() {
    const invoice = this.invoice;
    const paid = invoice.paid_at !== null;
    return html`
      <div class="stack">
        ${
          invoice.payments.length > 0
            ? html`
                <ul class="payments">
                  ${invoice.payments.map(
                    p => html`
                      <li>
                        <span>${formatDate(p.paid_on)}</span>
                        <span class="amount">${money(p.amount)}</span>
                        <wa-button
                          size="s"
                          appearance="plain"
                          ?disabled="${this.busy}"
                          @click="${() => void this.run(() => api.invoiceRemovePayment(p.id))}"
                        >
                          <wa-icon name="trash" label="Zahlung zurücknehmen"></wa-icon>
                        </wa-button>
                      </li>
                    `
                  )}
                  <li class="${paid ? 'paid' : 'open'}">
                    <span
                      >${paid ? `Bezahlt am ${formatDate(invoice.paid_at ?? '')}` : 'Offen'}</span
                    >
                    <span class="amount">${paid ? '' : money(openAmount(invoice))}</span>
                    <span></span>
                  </li>
                </ul>
              `
            : html`<span class="muted">Noch keine Zahlung eingegangen.</span>`
        }
        ${
          paid
            ? nothing
            : this.adding
              ? this.renderForm()
              : html`
                  <div>
                    <wa-button size="s" appearance="outlined" @click="${this.startAdding}">
                      Zahlung erfassen
                    </wa-button>
                  </div>
                `
        }
      </div>
    `;
  }
}

declare global {
  interface HTMLElementEventMap {
    'invoice-updated': CustomEvent<StoredInvoice>;
  }

  interface HTMLElementTagNameMap {
    'erng-payments': PaymentsComponent;
  }
}
