import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';

import { html } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';

export interface AskOptions {
  title: string;
  okLabel: string;
  cancelLabel: string;
  /** What cannot be taken back (delete, discard, replace): the button says so in red */
  danger?: boolean;
}

/** A yes-or-no question in a dialog of the app, made and removed by `ask` */
@customElement('erng-ask')
export class AskComponent extends ErngElement {
  @property({ attribute: false }) message = '';
  @property({ attribute: false }) options: AskOptions = { title: '', okLabel: '', cancelLabel: '' };
  /** Called once the dialog is gone, with the answer */
  @property({ attribute: false }) answered: (yes: boolean) => void = () => undefined;

  @state() private open = true;
  #yes = false;

  private close(yes: boolean) {
    this.#yes = yes;
    this.open = false;
  }

  override render() {
    const { title, okLabel, cancelLabel, danger } = this.options;
    return html`
      <wa-dialog
        label="${title}"
        ?open="${this.open}"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && this.answered(this.#yes)}"
      >
        ${this.message}
        <div slot="footer" class="dialog-footer">
          <wa-button appearance="outlined" @click="${() => this.close(false)}">
            ${cancelLabel}
          </wa-button>
          <div class="end">
            <wa-button variant="${danger ? 'danger' : 'brand'}" @click="${() => this.close(true)}">
              ${okLabel}
            </wa-button>
          </div>
        </div>
      </wa-dialog>
    `;
  }
}

/**
 * Asks a yes-or-no question in a dialog of the app rather than one of the system, so it looks like
 * every other dialog: true for the action, false for cancel, Escape or closing it.
 */
export const ask = (message: string, options: AskOptions): Promise<boolean> =>
  new Promise(resolve => {
    const dialog = document.createElement('erng-ask');
    dialog.message = message;
    dialog.options = options;
    dialog.answered = yes => {
      dialog.remove();
      resolve(yes);
    };
    document.body.append(dialog);
  });

declare global {
  interface HTMLElementTagNameMap {
    'erng-ask': AskComponent;
  }
}
