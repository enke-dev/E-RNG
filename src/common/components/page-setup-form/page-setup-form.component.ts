import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/details/details.js';
import '@awesome.me/webawesome/dist/components/dropdown/dropdown.js';
import '@awesome.me/webawesome/dist/components/dropdown-item/dropdown-item.js';
import '@awesome.me/webawesome/dist/components/icon/icon.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '../part-form/part-form.component.js';

import type { PropertyValues } from 'lit';
import { html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { Insets, PageSetup, Part } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { text } from '../../utils/form.utils.js';
import styles from './page-setup-form.component.css';

const EDGES: [keyof Insets, string][] = [
  ['top', 'Oben'],
  ['right', 'Rechts'],
  ['bottom', 'Unten'],
  ['left', 'Links'],
];

type Kind = Part['kind'];

/** What can be added, with the part it starts as */
const KINDS: Record<Kind, { label: string; make: () => Part }> = {
  infos: {
    label: 'Infos (Datum, Nummer, …)',
    make: () => ({
      kind: 'infos',
      anchor: 'top-right',
      x: 20,
      y: 45,
      width: 75,
      fields: ['number', 'date', 'service-date', 'due-date'],
      labels: true,
      align: 'right',
      long_dates: false,
      size: 9,
    }),
  },
  address: {
    label: 'Anschrift',
    make: () => ({
      kind: 'address',
      anchor: 'top-left',
      x: 25,
      y: 50,
      width: 80,
      sender_line: true,
      size: 10,
    }),
  },
  image: {
    label: 'Bild (Logo, …)',
    make: () => ({ kind: 'image', anchor: 'top-left', x: 25, y: 15, width: 40, image: null }),
  },
  text: {
    label: 'Text',
    make: () => ({
      kind: 'text',
      anchor: 'top-left',
      x: 25,
      y: 20,
      width: 60,
      text: 'Text',
      family: null,
      size: 9,
      align: 'left',
    }),
  },
  'page-number': {
    label: 'Seitenzahl',
    make: () => ({
      kind: 'page-number',
      anchor: 'top-right',
      x: 20,
      y: 285,
      width: 60,
      text: 'Seite {page} von {pages}',
      size: 7.5,
      align: 'right',
    }),
  },
  line: {
    label: 'Linie',
    make: () => ({ kind: 'line', anchor: 'top-left', x: 25, y: 269, width: 165, thickness: 0.5 }),
  },
};

/**
 * The tools for one page of a layout: the frame the content stays inside, and the parts placed
 * around it. Used for all pages, and again for the first one when it differs. Edits the given
 * object.
 *
 * @fires change
 */
@customElement('erng-page-setup-form')
export class PageSetupFormComponent extends ErngElement.withStyles(styles) {
  @property({ attribute: false })
  setup!: PageSetup;

  /** The part to mark, because the pointer is over it in the sample */
  @property({ type: Number }) hover: number | null = null;

  /** Opens a part, the token tells a repeated request from the same one */
  @property({ attribute: false }) reveal: { index: number; token: number } | undefined;

  /** The font families of the layout, a text can be set in them */
  @property({ attribute: false })
  families: string[] = [];

  /** What the user points at or has open, the sample shows where it is */
  @state() private hovered: number | null = null;
  @state() private opened: number | null = null;

  protected override updated(changed: PropertyValues<this>) {
    if (changed.has('reveal') && this.reveal) {
      // opens the part, the others close
      const target = this.renderRoot.querySelectorAll('wa-details')[this.reveal.index];
      if (target) {
        const show = () =>
          // after the tab and the part are visible, else there is nothing to scroll to
          requestAnimationFrame(() =>
            target.scrollIntoView({ block: 'center', behavior: 'smooth' })
          );
        if (target.open) {
          show();
        } else {
          target.addEventListener('wa-after-show', show, { once: true });
          target.open = true;
        }
      }
    }
  }

  /** A part was opened, by the user or by a click in the sample */
  private opening(index: number) {
    this.pointAt(() => (this.opened = index));
    this.dispatchEvent(
      new CustomEvent<number>('part-open', { detail: index, bubbles: true, composed: true })
    );
  }

  private pointAt(change: () => void) {
    change();
    this.dispatchEvent(
      new CustomEvent<{ hovered: number | null; opened: number | null }>('part-focus', {
        detail: { hovered: this.hovered, opened: this.opened },
        bubbles: true,
        composed: true,
      })
    );
  }

  private changed() {
    this.requestUpdate();
    this.dispatchEvent(new CustomEvent('change', { bubbles: true, composed: true }));
  }

  /** The title of a part: what it is, and for a text the start of it, quiet */
  private summary(part: Part) {
    const what = KINDS[part.kind].label.replace(/ \(.*\)$/, '');
    const snippet = part.kind === 'text' ? (part.text.split('\n')[0]?.slice(0, 28) ?? '') : '';
    return html`
      <span slot="summary"
        >${what}${snippet ? html` <span class="muted">${snippet}</span>` : nothing}</span
      >
    `;
  }

  override render() {
    const setup = this.setup;
    return html`
      <div class="stack">
        <strong>Rand für den Inhalt</strong>
        <div class="edges">
          ${EDGES.map(
            ([key, label]) => html`
              <wa-input
                label="${label}"
                type="number"
                min="0"
                step="1"
                .value="${String(setup.margins[key])}"
                @input="${(e: Event) => {
                  setup.margins[key] = Math.max(0, Number(text(e)) || 0);
                  this.changed();
                }}"
              >
                <span slot="end">mm</span>
              </wa-input>
            `
          )}
        </div>

        <strong>Elemente</strong>
        ${setup.parts.map(
          (part, index) => html`
            <wa-details
              name="part"
              ?data-hover="${this.hover === index}"
              @mouseenter="${() => this.pointAt(() => (this.hovered = index))}"
              @mouseleave="${() => this.pointAt(() => (this.hovered = null))}"
              @wa-show="${(e: Event) => e.target === e.currentTarget && this.opening(index)}"
              @wa-hide="${(e: Event) =>
                e.target === e.currentTarget &&
                this.pointAt(() => {
                  if (this.opened === index) {
                    this.opened = null;
                  }
                })}"
            >
              ${this.summary(part)}
              <div class="stack">
                <erng-part-form
                  .item="${part}"
                  .families="${this.families}"
                  @change="${() => this.changed()}"
                ></erng-part-form>
                <div class="remove">
                  <wa-button
                    appearance="outlined"
                    variant="danger"
                    title="Element entfernen"
                    aria-label="Element entfernen"
                    @click="${() => {
                      setup.parts.splice(index, 1);
                      this.pointAt(() => {
                        this.hovered = null;
                        this.opened = null;
                      });
                      this.changed();
                    }}"
                  >
                    <wa-icon name="trash"></wa-icon>
                  </wa-button>
                </div>
              </div>
            </wa-details>
          `
        )}
        <div>
          <wa-dropdown
            @wa-select="${(e: CustomEvent<{ item: { value: string } }>) => {
              const kind = e.detail.item.value as Kind;
              setup.parts.push(KINDS[kind].make());
              this.changed();
            }}"
          >
            <wa-button slot="trigger" appearance="outlined" with-caret
              >Element hinzufügen</wa-button
            >
            ${(Object.keys(KINDS) as Kind[]).map(
              kind =>
                html`<wa-dropdown-item value="${kind}">${KINDS[kind].label}</wa-dropdown-item>`
            )}
          </wa-dropdown>
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-page-setup-form': PageSetupFormComponent;
  }
}
