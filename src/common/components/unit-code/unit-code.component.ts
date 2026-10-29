import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/popup/popup.js';

import type WaInput from '@awesome.me/webawesome/dist/components/input/input.js';
import { openUrl } from '@tauri-apps/plugin-opener';
import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { CatalogEntry } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { text } from '../../utils/form.utils.js';
import { catalogName, describe, searchCatalog } from '../../utils/units.utils.js';
import styles from './unit-code.component.css';

const LIST_URL = 'https://www.xrepository.de/details/urn:xoev-de:kosit:codeliste:rec20_3';

/** More matches than this are counted, not drawn */
const SHOWN = 40;

/**
 * Picks a UN/ECE rec. 20 code by searching the code or its name, German or English. The common
 * codes are offered until something is typed. Only a code of the list can be picked.
 *
 * @fires code-change - the picked code is the `detail`
 */
@customElement('erng-unit-code')
export class UnitCodeComponent extends ErngElement.withStyles(styles) {
  @property({ attribute: false }) catalog: CatalogEntry[] = [];
  /** The picked code */
  @property() value = '';
  @property() label = 'Code';

  /** What is typed, undefined while the field shows the picked code */
  @state() private search: string | undefined;
  @state() private open = false;
  @state() private active = 0;

  private get results() {
    return searchCatalog(this.catalog, this.search ?? '');
  }

  private get picked() {
    return this.catalog.find(e => e.code === this.value);
  }

  protected override updated(changed: PropertyValues) {
    if (changed.has('active') || changed.has('search')) {
      this.renderRoot.querySelector('.option[aria-selected="true"]')?.scrollIntoView({
        block: 'nearest',
      });
    }
  }

  private show() {
    this.open = true;
  }

  private close() {
    this.open = false;
    this.search = undefined;
    this.active = 0;
  }

  private choose(entry: CatalogEntry | undefined) {
    if (!entry) {
      return;
    }
    this.value = entry.code;
    this.close();
    this.dispatchEvent(new CustomEvent<string>('code-change', { detail: entry.code }));
  }

  /** The option under the pointer is the active one, a click picks it; the keys belong to the field */
  private pointed(event: Event, shown: CatalogEntry[], pick: boolean) {
    const index = Number(
      (event.target as HTMLElement).closest('[data-index]')?.getAttribute('data-index')
    );
    if (Number.isNaN(index)) {
      return;
    }
    this.active = index;
    if (pick) {
      this.choose(shown[index]);
    }
  }

  private onKeydown(event: KeyboardEvent) {
    const shown = this.results.slice(0, SHOWN);
    switch (event.key) {
      case 'ArrowDown':
      case 'ArrowUp': {
        event.preventDefault();
        if (!this.open) {
          this.show();
          return;
        }
        const step = event.key === 'ArrowDown' ? 1 : -1;
        this.active = Math.min(Math.max(this.active + step, 0), shown.length - 1);
        return;
      }
      case 'Enter':
        if (this.open) {
          // the dialog around saves on Enter, here it picks
          event.preventDefault();
          event.stopPropagation();
          this.choose(shown[this.active]);
        }
        return;
      case 'Escape':
        if (this.open) {
          // closes the list, not the dialog around
          event.preventDefault();
          event.stopPropagation();
          this.close();
          // the next word replaces the picked code again
          void this.updateComplete.then(() => this.renderRoot.querySelector('wa-input')?.select());
        }
        return;
      default:
    }
  }

  private status(shown: CatalogEntry[], total: number) {
    if (!this.open) {
      return '';
    }
    const entry = shown[this.active];
    return entry ? `${total} Treffer, ${describe(entry)}` : 'Keine Treffer';
  }

  // the keys belong to the search field, which keeps the focus, so the options take none
  /* eslint-disable lit-a11y/click-events-have-key-events */
  private renderList() {
    const results = this.results;
    const shown = results.slice(0, SHOWN);
    return html`
      <div
        class="list"
        role="listbox"
        aria-label="Einheiten"
        @mousedown="${(e: Event) => e.preventDefault()}"
        @mousemove="${(e: Event) => this.pointed(e, shown, false)}"
        @click="${(e: Event) => this.pointed(e, shown, true)}"
      >
        ${
          shown.length === 0
            ? html`<div class="none muted">Keine Einheit gefunden.</div>`
            : shown.map(
                (entry, index) => html`
                  <div
                    class="option"
                    role="option"
                    aria-selected="${index === this.active}"
                    ?data-picked="${entry.code === this.value}"
                    data-index="${index}"
                  >
                    <strong>${entry.code}</strong> – ${catalogName(entry)}
                  </div>
                `
              )
        }
        ${
          results.length > shown.length
            ? html`
                <div class="none muted">
                  ${results.length - shown.length} weitere Treffer, Suche eingrenzen.
                </div>
              `
            : nothing
        }
      </div>
      <span class="visually-hidden" role="status" aria-live="polite">
        ${this.status(shown, results.length)}
      </span>
    `;
  }

  /* eslint-enable lit-a11y/click-events-have-key-events */

  override render() {
    const picked = this.picked;
    return html`
      <wa-popup
        ?active="${this.open}"
        placement="bottom-start"
        distance="4"
        strategy="fixed"
        sync="width"
        flip
        shift
        auto-size="vertical"
        auto-size-padding="8"
      >
        <wa-input
          slot="anchor"
          label="${this.label}"
          placeholder="Code oder Name suchen"
          autocomplete="off"
          .value="${this.search ?? (picked ? describe(picked) : this.value)}"
          @focus="${(e: FocusEvent) => {
            // a typed word replaces what is shown
            (e.target as WaInput).select();
            this.show();
          }}"
          @click="${(e: MouseEvent) => {
            // the field may be focused already, the click selects again
            (e.target as WaInput).select();
            this.show();
          }}"
          @blur="${this.close}"
          @input="${(e: Event) => {
            this.search = text(e);
            this.active = 0;
            this.show();
          }}"
          @keydown="${this.onKeydown}"
        >
          <span slot="hint">
            Steht in der XML-Rechnung.
            <a
              href="${LIST_URL}"
              @click="${(e: Event) => {
                e.preventDefault();
                void openUrl(LIST_URL);
              }}"
              >UN/ECE-Empfehlung 20 im XRepository</a
            >
          </span>
        </wa-input>
        ${this.open ? this.renderList() : nothing}
      </wa-popup>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-unit-code': UnitCodeComponent;
  }
}
