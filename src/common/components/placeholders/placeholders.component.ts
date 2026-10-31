import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/icon/icon.js';
import '@awesome.me/webawesome/dist/components/input/input.js';

import type WaInput from '@awesome.me/webawesome/dist/components/input/input.js';
import type WaTextarea from '@awesome.me/webawesome/dist/components/textarea/textarea.js';
import { html } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import { text } from '../../utils/form.utils.js';
import type { PlaceholderGroup } from '../../utils/placeholders.utils.js';
import { searchPlaceholders } from '../../utils/placeholders.utils.js';
import styles from './placeholders.component.css';

type Field = WaInput | WaTextarea;

/**
 * A small button for a text field that takes placeholders: it opens the list of them, grouped,
 * with what each stands for and how it may look, and puts the one picked where the cursor was.
 * It belongs inside its field: in the `end` slot of a `wa-input`, in the label of a `wa-textarea`
 * (which has no `end` slot). The field gets an `input` event, as if it had been typed.
 *
 * @example html`<wa-input …><erng-placeholders slot="end" .groups="${PAGE_PLACEHOLDERS}"></erng-placeholders></wa-input>`
 */
@customElement('erng-placeholders')
export class PlaceholdersComponent extends ErngElement.withStyles(styles) {
  @property({ attribute: false }) groups: PlaceholderGroup[] = [];

  @state() private open = false;
  @state() private query = '';
  /** Where the cursor was in the field when the list opened, the placeholder goes there */
  #range: [number, number] | undefined;

  private get field(): Field | null {
    return this.closest<Field>('wa-input, wa-textarea');
  }

  /** The native control inside the field, it knows where the cursor is */
  private get control(): HTMLInputElement | HTMLTextAreaElement | null | undefined {
    return this.field?.shadowRoot?.querySelector<HTMLInputElement | HTMLTextAreaElement>(
      'input, textarea'
    );
  }

  private show(event: Event) {
    // a click in the label of a textarea would focus it, the list opens instead
    event.preventDefault();
    event.stopPropagation();
    const control = this.control;
    const length = String(this.field?.value ?? '').length;
    this.#range = [control?.selectionStart ?? length, control?.selectionEnd ?? length];
    this.query = '';
    this.open = true;
  }

  /** Keeps an event of the dialog inside it, the field around would take it for its own */
  private contain(event: Event) {
    // Escape still closes the dialog
    if (!(event instanceof KeyboardEvent && event.key === 'Escape')) {
      event.stopPropagation();
    }
  }

  private async insert(name: string) {
    const field = this.field;
    if (!field) {
      return;
    }
    const value = String(field.value ?? '');
    const [start, end] = this.#range ?? [value.length, value.length];
    const snippet = `{${name}}`;
    field.value = value.slice(0, start) + snippet + value.slice(end);
    field.dispatchEvent(new InputEvent('input', { bubbles: true, composed: true }));
    field.dispatchEvent(new Event('change', { bubbles: true, composed: true }));
    this.open = false;
    await field.updateComplete;
    field.focus();
    field.setSelectionRange(start + snippet.length, start + snippet.length);
  }

  override render() {
    const groups = searchPlaceholders(this.groups, this.query);
    return html`
      <wa-button
        class="trigger"
        appearance="outlined"
        variant="neutral"
        size="small"
        pill
        title="Platzhalter einfügen"
        @click="${this.show}"
      >
        <wa-icon name="placeholder" label="Platzhalter einfügen"></wa-icon>
      </wa-button>
      <wa-dialog
        label="Platzhalter einfügen"
        ?open="${this.open}"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && (this.open = false)}"
      >
        <!-- the dialog sits inside the field it fills: what is typed here must not reach that field -->
        <div
          class="stack"
          @input="${this.contain}"
          @change="${this.contain}"
          @keydown="${this.contain}"
        >
          <wa-input
            placeholder="Platzhalter suchen"
            size="small"
            with-clear
            .value="${this.query}"
            @input="${(e: Event) => (this.query = text(e))}"
          ></wa-input>
          ${
            groups.length === 0
              ? html`<p class="muted">Kein Platzhalter passt.</p>`
              : html`
                  <table>
                    <thead>
                      <tr>
                        <th>Platzhalter</th>
                        <th>Bedeutung</th>
                        <th>Beispiel</th>
                      </tr>
                    </thead>
                    ${groups.map(
                      group => html`
                        <tbody>
                          <tr class="group">
                            <th colspan="3">${group.heading}</th>
                          </tr>
                          ${group.placeholders.map(
                            p => html`
                              <tr
                                tabindex="0"
                                @click="${() => void this.insert(p.name)}"
                                @keydown="${(e: KeyboardEvent) =>
                                  (e.key === 'Enter' || e.key === ' ') && void this.insert(p.name)}"
                              >
                                <td><code>{${p.name}}</code></td>
                                <td>${p.label}</td>
                                <td class="muted">${p.example}</td>
                              </tr>
                            `
                          )}
                        </tbody>
                      `
                    )}
                  </table>
                `
          }
        </div>
        <div slot="footer" class="dialog-footer">
          <wa-button appearance="outlined" @click="${() => (this.open = false)}">
            Schließen
          </wa-button>
        </div>
      </wa-dialog>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-placeholders': PlaceholdersComponent;
  }
}
