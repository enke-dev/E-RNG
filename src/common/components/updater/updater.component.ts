import '@awesome.me/webawesome/dist/components/button/button.js';

import { listen } from '@tauri-apps/api/event';
import { error as logError, warn } from '@tauri-apps/plugin-log';
import { relaunch } from '@tauri-apps/plugin-process';
import type { Update } from '@tauri-apps/plugin-updater';
import { check } from '@tauri-apps/plugin-updater';
import { html, nothing } from 'lit';
import { customElement, eventOptions, state } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import { notify, notifyError } from '../../utils/notify.utils.js';
import styles from './updater.component.css';

type UpdaterState = 'idle' | 'checking' | 'available' | 'downloading' | 'installing';

/**
 * Checks for a new release on start and offers to install it, and checks again when the menu bar
 * asks for it. The button only shows up when there is something to do, and tells how far the update
 * is while it is being installed.
 */
@customElement('erng-updater')
export class UpdaterComponent extends ErngElement.withStyles(styles) {
  #update: Update | undefined;

  @state()
  private status: UpdaterState = 'idle';

  /** Percent of the download, unknown while the size is not told */
  @state()
  private progress: number | undefined;

  #stopListening: (() => void) | undefined;

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
    this.status = 'downloading';
    this.progress = undefined;
    let total = 0;
    let received = 0;
    try {
      await this.#update.downloadAndInstall(event => {
        if (event.event === 'Started') {
          total = event.data.contentLength ?? 0;
          received = 0;
          this.progress = total > 0 ? 0 : undefined;
        } else if (event.event === 'Progress') {
          received += event.data.chunkLength;
          this.progress =
            total > 0 ? Math.min(100, Math.round((received / total) * 100)) : undefined;
        } else {
          this.status = 'installing';
        }
      });
      await relaunch();
    } catch (error) {
      void logError(`update failed: ${String(error)}`);
      this.status = 'available';
      notifyError(error);
    }
  }

  /** The menu bar asked: look now, and say what came of it (the check on start is silent) */
  private async handleMenuCheck() {
    if (this.status === 'downloading' || this.status === 'installing') {
      notify('Das Update wird gerade installiert.', 'neutral');
      return;
    }
    try {
      await this.checkUpdate({ silent: false });
      if (this.#update) {
        notify(`Version ${this.#update.version} ist verfügbar.`);
      } else {
        notify('Die App ist auf dem neuesten Stand.', 'neutral');
      }
    } catch (error) {
      notifyError(error);
    }
  }

  override connectedCallback() {
    super.connectedCallback();
    void this.checkUpdate({ silent: true });
    void listen('check-for-updates', () => void this.handleMenuCheck()).then(stop => {
      // the element may be gone before the listener is in place
      if (this.isConnected) {
        this.#stopListening = stop;
      } else {
        stop();
      }
    });
  }

  override disconnectedCallback() {
    super.disconnectedCallback();
    this.#stopListening?.();
  }

  override render() {
    if (
      this.status !== 'available' &&
      this.status !== 'downloading' &&
      this.status !== 'installing'
    ) {
      return nothing;
    }
    const busy = this.status !== 'available';
    const label =
      this.status === 'downloading'
        ? `Wird geladen${this.progress === undefined ? '…' : ` ${this.progress} %`}`
        : this.status === 'installing'
          ? 'Wird installiert…'
          : `Auf ${this.#update?.version} aktualisieren`;

    return html`
      <wa-button size="s" variant="brand" ?loading="${busy}" @click="${this.handleInstall}">
        ${label}
      </wa-button>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-updater': UpdaterComponent;
  }
}
