import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';

import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { html, nothing } from 'lit';
import { customElement, state } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import { notifyError } from '../../utils/notify.utils.js';
import styles from './replaced.component.css';

/** What a restore or a removal did */
export interface Replaced {
  title: string;
  lines: string[];
  safetyCopy: string;
  problems: string[];
}

const KEY = 'erng:replaced';

/**
 * The data changed under every view: the app reloads at once so nothing shows the old data, and
 * this tells what happened once it is back.
 */
export function reloadReplaced(replaced: Replaced) {
  try {
    sessionStorage.setItem(KEY, JSON.stringify(replaced));
  } catch {
    // without the storage the reload still shows the new data, only the summary is lost
  }
  location.reload();
}

const taken = (): Replaced | undefined => {
  try {
    const stored = sessionStorage.getItem(KEY);
    sessionStorage.removeItem(KEY);
    return stored ? (JSON.parse(stored) as Replaced) : undefined;
  } catch {
    return undefined;
  }
};

/** A file or folder as a plain link that shows it in the Finder (Explorer, file manager) */
export const revealLink = (path: string) => html`
  <a
    class="path"
    href="#"
    @click="${(e: Event) => {
      e.preventDefault();
      // the folder of the safety copies only exists after the first one, then its parent shows
      revealItemInDir(path)
        .catch(() => revealItemInDir(path.replace(/[/\\][^/\\]+[/\\]?$/, '')))
        .catch(notifyError);
    }}"
    >${path}</a
  >
`;

/** The summary of a restore or a removal after the reload, in the root so any route shows it */
@customElement('erng-replaced')
export class ReplacedComponent extends ErngElement.withStyles(styles) {
  @state() private replaced = taken();
  @state() private open = this.replaced !== undefined;

  override render() {
    const replaced = this.replaced;
    if (!replaced) {
      return nothing;
    }
    return html`
      <wa-dialog
        ?open="${this.open}"
        label="${replaced.title}"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && (this.open = false)}"
      >
        <div class="stack">
          ${replaced.lines.map(line => html`<span>${line}</span>`)}
          <span>
            Die bisherigen Daten liegen als Kopie unter ${revealLink(replaced.safetyCopy)}
          </span>
          ${
            replaced.problems.length > 0
              ? html`
                  <wa-callout variant="warning">
                    Bei der Prüfung der Rechnungsdateien fiel auf:
                    <ul>
                      ${replaced.problems.map(problem => html`<li>${problem}</li>`)}
                    </ul>
                  </wa-callout>
                `
              : nothing
          }
        </div>
        <div slot="footer" class="dialog-footer">
          <wa-button variant="brand" class="end" @click="${() => (this.open = false)}">
            Weiter
          </wa-button>
        </div>
      </wa-dialog>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-replaced': ReplacedComponent;
  }
}
