import '@awesome.me/webawesome/dist/components/button/button.js';

import { error as logError, warn } from '@tauri-apps/plugin-log';
import { relaunch } from '@tauri-apps/plugin-process';
import type { Update } from '@tauri-apps/plugin-updater';
import { check } from '@tauri-apps/plugin-updater';
import type { CSSResultGroup } from 'lit';
import { html, nothing } from 'lit';
import { customElement, eventOptions, state } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import styles from './updater.component.css';

type UpdaterState = 'idle' | 'checking' | 'available' | 'installing';

/**
 * Checks for a new release on start and offers to install it.
 * The button only shows up when there is something to do.
 */
@customElement('erng-updater')
export class UpdaterComponent extends ErngElement {
  static override styles: CSSResultGroup = [ErngElement.styles, styles];

  #update: Update | undefined;

  @state()
  private status: UpdaterState = 'idle';

  /** Looks for an update, silent by default: failures (offline, dev build) only reach the log */
  async checkUpdate({ silent = true } = {}) {
    this.status = 'checking';
    try {
      this.#update = (await check()) ?? undefined;
      this.status = this.#update ? 'available' : 'idle';
    } catch (error) {
      this.status = 'idle';
      if (silent) {
        void warn(`update check failed: ${String(error)}`);
      } else {
        throw error;
      }
    }
  }

  @eventOptions({ passive: true })
  private async handleInstall() {
    if (!this.#update) {
      return;
    }
    this.status = 'installing';
    try {
      await this.#update.downloadAndInstall();
      await relaunch();
    } catch (error) {
      void logError(`update failed: ${String(error)}`);
      this.status = 'available';
    }
  }

  override connectedCallback() {
    super.connectedCallback();
    void this.checkUpdate({ silent: true });
  }

  override render() {
    if (this.status !== 'available' && this.status !== 'installing') {
      return nothing;
    }

    return html`
      <wa-button
        size="s"
        variant="brand"
        ?loading="${this.status === 'installing'}"
        @click="${this.handleInstall}"
      >
        Update to ${this.#update?.version}
      </wa-button>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-updater': UpdaterComponent;
  }
}
