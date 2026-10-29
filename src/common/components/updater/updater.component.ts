import '@awesome.me/webawesome/dist/components/button/button.js';

import { listen } from '@tauri-apps/api/event';
import { error as logError, warn } from '@tauri-apps/plugin-log';
import { relaunch } from '@tauri-apps/plugin-process';
import type { Update } from '@tauri-apps/plugin-updater';
import { check } from '@tauri-apps/plugin-updater';
import { html, nothing } from 'lit';
import { customElement, eventOptions, state } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import { confirmLeave } from '../../utils/leave.utils.js';
import { notify, notifyError } from '../../utils/notify.utils.js';
import styles from './updater.component.css';

type UpdaterState = 'idle' | 'checking' | 'available' | 'downloading' | 'downloaded' | 'installing';

/**
 * Checks for a new release on start and offers it, and checks again when the menu bar asks for it.
 * The button first loads the update while the user goes on working, then offers to install it: the
 * app restarts only when the user says so. A loaded update lives in memory, a restart forgets it and
 * the check on start offers the newest again; a newer one found later replaces it. The button only
 * shows up when there is something to do, and tells how far the download is.
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

  /** How wide the button was before its shape changed, to animate from */
  #width: number | undefined;

  get #button() {
    return this.renderRoot.querySelector('wa-button') ?? undefined;
  }

  get #busy() {
    return this.status === 'downloading' || this.status === 'installing';
  }

  /** Busy with nothing to count: only the spinner shows */
  get #spinning() {
    return (
      this.status === 'installing' || (this.status === 'downloading' && this.progress === undefined)
    );
  }

  #wasSpinning = false;

  /**
   * Looks for an update, silent by default: failures (offline, dev build) only reach the log. A
   * loaded update stays while it is the newest, a newer one replaces it and is offered to load.
   */
  async checkUpdate({ silent = true } = {}) {
    const loaded = this.status === 'downloaded' ? this.#update : undefined;
    if (!loaded) {
      this.status = 'checking';
    }
    try {
      const found = (await check()) ?? undefined;
      if (loaded && (!found || found.version === loaded.version)) {
        void found?.close();
        return;
      }
      void loaded?.close();
      this.#update = found;
      this.status = found ? 'available' : 'idle';
    } catch (error) {
      if (!loaded) {
        this.status = 'idle';
      }
      if (silent) {
        void warn(`update check failed: ${String(error)}`);
      } else {
        throw error;
      }
    }
  }

  @eventOptions({ passive: true })
  private handleClick() {
    if (this.status === 'available') {
      void this.download();
    } else if (this.status === 'downloaded') {
      void this.install();
    }
  }

  /** Loads the update while the user goes on working, nothing is installed yet */
  private async download() {
    const update = this.#update;
    if (!update) {
      return;
    }
    this.status = 'downloading';
    this.progress = undefined;
    let total = 0;
    let received = 0;
    try {
      await update.download(event => {
        if (event.event === 'Started') {
          total = event.data.contentLength ?? 0;
          received = 0;
          this.progress = total > 0 ? 0 : undefined;
        } else if (event.event === 'Progress') {
          received += event.data.chunkLength;
          this.progress =
            total > 0 ? Math.min(100, Math.round((received / total) * 100)) : undefined;
        }
      });
      // a check while it loaded may have found a newer one
      if (this.#update === update) {
        this.status = 'downloaded';
      }
    } catch (error) {
      void logError(`update download failed: ${String(error)}`);
      this.status = 'available';
      notifyError(error);
    }
  }

  /** Installs the loaded update and restarts, when the user says so and nothing unsaved is lost */
  private async install() {
    const update = this.#update;
    if (!update || !(await confirmLeave('/'))) {
      return;
    }
    this.status = 'installing';
    try {
      await update.install();
      await relaunch();
    } catch (error) {
      void logError(`update install failed: ${String(error)}`);
      // the loaded package is used up, it has to be loaded again
      this.status = 'available';
      notifyError(error);
    }
  }

  /** The menu bar asked: look now, and say what came of it (the check on start is silent) */
  private async handleMenuCheck() {
    if (this.status === 'downloading') {
      notify('Das Update wird gerade geladen.', 'neutral');
      return;
    }
    if (this.status === 'installing') {
      notify('Das Update wird gerade installiert.', 'neutral');
      return;
    }
    try {
      await this.checkUpdate({ silent: false });
      if (this.status === 'downloaded') {
        notify(`Version ${this.#update?.version} ist geladen und kann installiert werden.`);
      } else if (this.#update) {
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

  protected override willUpdate() {
    this.#width = this.#button?.getBoundingClientRect().width;
  }

  /** Only the spinner left, the button shrinks to a square around it; a label that counts stays */
  protected override updated() {
    const button = this.#button;
    const spinning = this.#spinning;
    if (!button || spinning === this.#wasSpinning) {
      return;
    }
    this.#wasSpinning = spinning;
    button.style.width = spinning ? `${button.offsetHeight}px` : '';
    const from = this.#width;
    const to = button.getBoundingClientRect().width;
    if (from && from !== to && !matchMedia('(prefers-reduced-motion: reduce)').matches) {
      button.animate([{ width: `${from}px` }, { width: `${to}px` }], {
        duration: 200,
        easing: 'ease-out',
      });
    }
  }

  override render() {
    if (this.status !== 'available' && this.status !== 'downloaded' && !this.#busy) {
      return nothing;
    }
    // figure spaces keep `5%` as wide as `45%`, so the button does not jump while it counts
    const percent = String(this.progress ?? 0).padStart(3, '\u2007');
    const label =
      this.status === 'downloading'
        ? `Wird geladen ${percent}%`
        : this.status === 'installing'
          ? 'Wird installiert…'
          : this.status === 'downloaded'
            ? `Jetzt ${this.#update?.version} installieren`
            : `Auf ${this.#update?.version} aktualisieren`;

    return html`
      <wa-button
        size="s"
        variant="brand"
        class="${this.#spinning ? 'spinning' : ''}"
        title="${this.#spinning ? label : this.status === 'downloaded' ? `Version ${this.#update?.version} ist geladen, die App startet zum Installieren neu` : ''}"
        aria-label="${label}"
        ?loading="${this.#spinning}"
        ?disabled="${this.#busy && !this.#spinning}"
        @click="${this.handleClick}"
      >
        ${this.#spinning ? nothing : label}
      </wa-button>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-updater': UpdaterComponent;
  }
}
