import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';

import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';

export interface AskOptions {
  title: string;
  okLabel: string;
  cancelLabel: string;
  /** What cannot be taken back (delete, discard, replace): the button says so in red */
  danger?: boolean;
  /** A second action beside the main one, for a choice of two ways (`choose`) */
  altLabel?: string;
}

/** What the user picked: the main action, the second one, or nothing (cancel, Escape) */
export type Choice = 'ok' | 'alt' | null;

/** A yes-or-no question (or a choice of two) in a dialog of the app, made and removed by `ask` */
@customElement('erng-ask')
export class AskComponent extends ErngElement {
  @property({ attribute: false }) message = '';
  @property({ attribute: false }) options: AskOptions = { title: '', okLabel: '', cancelLabel: '' };
  /** Called once the dialog is gone, with the answer */
  @property({ attribute: false }) answered: (choice: Choice) => void = () => undefined;

  @state() private open = true;
  #choice: Choice = null;

  private close(choice: Choice) {
    this.#choice = choice;
    this.open = false;
  }

  override render() {
    const { title, okLabel, cancelLabel, danger, altLabel } = this.options;
    return html`
      <wa-dialog
        label="${title}"
        ?open="${this.open}"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && this.answered(this.#choice)}"
      >
        ${this.message}
        <div slot="footer" class="dialog-footer">
          <wa-button appearance="outlined" @click="${() => this.close(null)}">
            ${cancelLabel}
          </wa-button>
          <div class="end">
            ${
              altLabel
                ? html`
                    <wa-button appearance="outlined" @click="${() => this.close('alt')}">
                      ${altLabel}
                    </wa-button>
                  `
                : nothing
            }
            <wa-button variant="${danger ? 'danger' : 'brand'}" @click="${() => this.close('ok')}">
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
export const ask = async (message: string, options: AskOptions): Promise<boolean> =>
  (await choose(message, { ...options, altLabel: undefined })) === 'ok';

/**
 * A choice of two ways in a dialog of the app: `ok` for the main one (`okLabel`), `alt` for
 * the other (`altLabel`), null for cancel, Escape or closing it.
 */
export const choose = (message: string, options: AskOptions): Promise<Choice> =>
  new Promise(resolve => {
    const dialog = document.createElement('erng-ask');
    dialog.message = message;
    dialog.options = options;
    dialog.answered = choice => {
      dialog.remove();
      resolve(choice);
    };
    document.body.append(dialog);
  });

declare global {
  interface HTMLElementTagNameMap {
    'erng-ask': AskComponent;
  }
}
