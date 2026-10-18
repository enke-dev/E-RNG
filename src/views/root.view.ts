import '@awesome.me/webawesome/dist/components/badge/badge.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '../common/components/updater/updater.component.js';

import { platform } from '@tauri-apps/plugin-os';
import { html } from 'lit';
import { customElement, eventOptions } from 'lit/decorators.js';
import { when } from 'lit/directives/when.js';

import { ErngElement } from '../common/utils/base.utils.js';
import { registerRouter } from '../common/utils/routing.utils.js';
import { errors, isErrored, isLoading } from '../state.js';
import { routes } from './root.routes.js';
import styles from './root.view.css';

const NAV = [
  ['/', 'Rechnungen'],
  ['/customers', 'Kunden'],
  ['/templates', 'Vorlagen'],
  ['/settings', 'Einstellungen'],
] as const;

const isActive = (href: string) =>
  href === '/'
    ? location.pathname === '/' || location.pathname.startsWith('/invoices')
    : location.pathname.startsWith(href);

@customElement('erng-root')
export class RootView extends ErngElement.withStyles(styles) {
  readonly #router = registerRouter(this, routes);

  @eventOptions({ passive: true })
  private handleErrorClick() {
    location.href = '/';
  }

  override connectedCallback() {
    super.connectedCallback();
    // the overlay title bar puts the traffic lights on top of the header
    this.toggleAttribute('macos', platform() === 'macos');
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
          ${NAV.map(
            ([href, label]) => html`
              <a href="${href}" ?aria-current="${isActive(href)}">${label}</a>
            `
          )}
        </nav>
        <div id="end">
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
        () => html`<main>${this.#router.outlet()}</main>`
      )}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-root': RootView;
  }
}
