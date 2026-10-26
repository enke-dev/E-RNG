import '@awesome.me/webawesome/dist/components/badge/badge.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '../common/components/updater/updater.component.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import { listen } from '@tauri-apps/api/event';
import { platform } from '@tauri-apps/plugin-os';
import { html, nothing } from 'lit';
import { customElement, eventOptions } from 'lit/decorators.js';
import { keyed } from 'lit/directives/keyed.js';
import { when } from 'lit/directives/when.js';

import { ErngElement } from '../common/utils/base.utils.js';
import {
  activeContext,
  applyBrand,
  available,
  label,
  loadContexts,
  ready,
  switchContext,
} from '../common/utils/context.utils.js';
import { rememberRoute } from '../common/utils/memory.utils.js';
import { notifyError } from '../common/utils/notify.utils.js';
import { followOpenedFiles } from '../common/utils/opened.utils.js';
import { goto, registerRouter } from '../common/utils/routing.utils.js';
import { errors, isErrored, isLoading } from '../state.js';
import { routes } from './root.routes.js';
import styles from './root.view.css';

const NAV = [
  ['/', 'Rechnungen'],
  ['/customers', 'Kunden'],
  ['/templates', 'Positionen'],
  ['/settings', 'Einstellungen'],
] as const;

/** Views that issue invoices wait for a context that can */
const NEEDS_CONTEXT = new Set(['/', '/customers', '/templates']);

const isActive = (href: string) =>
  href === '/'
    ? location.pathname === '/' || location.pathname.startsWith('/invoices')
    : location.pathname.startsWith(href);

@customElement('erng-root')
export class RootView extends ErngElement.withStyles(styles) {
  readonly #router = registerRouter(this, routes);
  #stopMenu: (() => void) | undefined;

  @eventOptions({ passive: true })
  private handleErrorClick() {
    location.href = '/';
  }

  override connectedCallback() {
    super.connectedCallback();
    // the overlay title bar puts the traffic lights on top of the header
    this.toggleAttribute('macos', platform() === 'macos');
    void followOpenedFiles();
    loadContexts().catch((error: unknown) => notifyError(error));
    // the menu bar entry "Einstellungen…" (⌘,), the settings stay where they are if open already
    void listen('open-settings', () => {
      if (!location.pathname.startsWith('/settings')) {
        goto('/settings');
      }
    }).then(stop => {
      if (this.isConnected) {
        this.#stopMenu = stop;
      } else {
        stop();
      }
    });
  }

  override disconnectedCallback() {
    super.disconnectedCallback();
    this.#stopMenu?.();
  }

  /** Another context: its brand color tints the UI, what shows is what it owns */
  private contextSwitch() {
    const active = activeContext.get();
    const list = available.get();
    // there is nothing to switch between with one context
    if (!active || list.length < 2) {
      return nothing;
    }
    return html`<wa-select
      size="small"
      aria-label="Kontext"
      .value="${String(active.id)}"
      @change="${async (e: Event) => {
        const select = e.target as WaSelect;
        if (!(await switchContext(Number(select.value)))) {
          select.value = String(activeContext.get()?.id);
        }
      }}"
    >
      ${list.map(c => html`<wa-option value="${String(c.id)}">${label(c)}</wa-option>`)}
    </wa-select>`;
  }

  /** The router renders again after every navigation (links, back and forth): that is the view open now */
  protected override updated() {
    rememberRoute(location.pathname);
    applyBrand(activeContext.get()?.brand);
  }

  protected override async firstUpdated() {
    // remove initial loading indicator
    await this.updateComplete;
    document.getElementById('wait')?.remove();
    this.setAttribute('hydrated', '');
    isLoading.set(false);
  }

  protected override render() {
    return html`
      <header data-tauri-drag-region>
        <div id="start">
          <h1>E-RNG</h1>
          <h2>ZUGFeRD, Factur-X PDF, XRechnung XML</h2>
        </div>
        <nav>
          ${NAV.map(([href, text]) =>
            NEEDS_CONTEXT.has(href) && !ready.get()
              ? html`<a
                  class="disabled"
                  aria-disabled="true"
                  title="Angaben zum Kontext fehlen"
                  href="${href}"
                  >${text}</a
                >`
              : html`<a href="${href}" ?aria-current="${isActive(href)}">${text}</a>`
          )}
        </nav>
        <div id="end">
          ${this.contextSwitch()}
          <erng-updater></erng-updater>
        </div>
      </header>

      ${when(
        isErrored.get(),
        () => html`
          <wa-dialog open with-footer without-header label="Something went wrong">
            ${errors.get().join('\n') || 'An unexpected error occurred.'}
            <wa-button slot="footer" variant="brand" @click="${this.handleErrorClick}">
              Reload application
            </wa-button>
          </wa-dialog>
        `,
        () => html`<main>${keyed(activeContext.get()?.id, this.#router.outlet())}</main>`
      )}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-root': RootView;
  }
}
