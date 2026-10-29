import '@awesome.me/webawesome/dist/components/card/card.js';
import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/tab/tab.js';
import '@awesome.me/webawesome/dist/components/tab-group/tab-group.js';
import '@awesome.me/webawesome/dist/components/tab-panel/tab-panel.js';
import '../font-list/font-list.component.js';
import '../format-form/format-form.component.js';
import '../page-setup-form/page-setup-form.component.js';

import type { PropertyValues } from 'lit';
import { html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import type { CustomLayout, PageSetup } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { text } from '../../utils/form.utils.js';
import styles from './layout-form.component.css';

/** The fields of an own layout, edits the given object and fires `change` */
@customElement('erng-layout-form')
export class LayoutFormComponent extends ErngElement.withStyles(styles) {
  @property({ attribute: false })
  layout!: CustomLayout;

  private changed() {
    this.requestUpdate();
    this.dispatchEvent(new CustomEvent('change', { bubbles: true, composed: true }));
  }

  /** Tells the sample what the user looks at: a page setup, and the part pointed at or open in it */
  private pointAt(first: boolean, parts: { hovered: number | null; opened: number | null }) {
    this.dispatchEvent(
      new CustomEvent<{ first: boolean; hovered: number | null; opened: number | null }>(
        'part-focus',
        { detail: { first, ...parts }, bubbles: true, composed: true }
      )
    );
  }

  private setup(setup: PageSetup, families: string[], first: boolean) {
    const reveal = this.reveal?.first === first ? this.reveal : undefined;
    return html`
      <erng-page-setup-form
        .setup="${setup}"
        .families="${families}"
        .hover="${this.hover?.first === first ? this.hover.index : null}"
        .reveal="${reveal}"
        @change="${() => this.changed()}"
        @part-focus="${(e: CustomEvent<{ hovered: number | null; opened: number | null }>) => {
          e.stopPropagation();
          this.pointAt(first, e.detail);
        }}"
        @part-open="${(e: CustomEvent<number>) => {
          e.stopPropagation();
          this.dispatchEvent(
            new CustomEvent<{ first: boolean; index: number }>('part-open', {
              detail: { first, index: e.detail },
              bubbles: true,
              composed: true,
            })
          );
        }}"
      ></erng-page-setup-form>
    `;
  }

  /** The part to mark in the form, because the pointer is over it in the sample */
  @property({ attribute: false }) hover: { first: boolean; index: number } | null = null;

  /** Opens a part of a page setup, the token tells a repeated request from the same one */
  @property({ attribute: false }) reveal:
    { first: boolean; index: number; token: number } | undefined;

  /** The setup that is in front, when the first page has its own: kept in step with the sample */
  @property() tab: 'first' | 'page' | undefined;

  /** Draws the aids into the sample */
  @property({ type: Boolean }) guides = false;

  protected override updated(changed: PropertyValues<this>) {
    if (changed.has('tab') && this.tab && this.layout.first) {
      const tabs = this.renderRoot.querySelector('wa-tab-group');
      if (tabs) {
        tabs.active = this.tab;
      }
    }
    if (changed.has('reveal') && this.reveal && this.layout.first) {
      // the setup the part is on comes to the front
      const tabs = this.renderRoot.querySelector('wa-tab-group');
      if (tabs) {
        tabs.active = this.reveal.first ? 'first' : 'page';
      }
    }
  }

  override render() {
    const l = this.layout;
    const families = [...new Set(l.fonts.map(f => f.family))];
    return html`
      <div class="cards">
        <wa-card>
          <div slot="header">${l.id === null ? 'Neues Layout' : 'Layout'}</div>
          <div class="stack">
            <wa-input
              label="Name"
              placeholder="Name des Layouts"
              required
              .value="${l.name}"
              @input="${(e: Event) => {
                l.name = text(e);
                this.changed();
              }}"
            ></wa-input>
            <wa-checkbox
              ?checked="${this.guides}"
              hint="Zeichnet Linien an die Ränder des Inhalts, die Maße eines Briefs und den druckbaren Bereich"
              @change="${(e: Event) =>
                this.dispatchEvent(
                  new CustomEvent<boolean>('guides-change', {
                    detail: (e.target as HTMLInputElement).checked,
                  })
                )}"
            >
              Hilfslinien in der Vorschau
            </wa-checkbox>
          </div>
        </wa-card>

        <wa-card>
          <div slot="header">Schrift</div>
          <erng-font-list .layout="${l}" @change="${() => this.changed()}"></erng-font-list>
        </wa-card>

        <wa-card>
          <div slot="header">Formate</div>
          <erng-format-form
            .format="${l.format}"
            @change="${() => this.changed()}"
          ></erng-format-form>
        </wa-card>

        <wa-card>
          <div slot="header">Seiten</div>
          <div class="stack">
            <wa-checkbox
              ?checked="${l.first !== null}"
              @change="${(e: Event) => {
                // the first page starts as a copy of the others, with the same tools
                l.first = (e.target as HTMLInputElement).checked ? structuredClone(l.page) : null;
                this.pointAt(l.first !== null, { hovered: null, opened: null });
                this.changed();
              }}"
            >
              Erste Seite weicht ab
            </wa-checkbox>

            ${
              l.first
                ? html`
                    <wa-tab-group
                      @wa-tab-show="${(e: CustomEvent<{ name: string }>) =>
                        e.target === e.currentTarget &&
                        this.pointAt(e.detail.name === 'first', { hovered: null, opened: null })}"
                    >
                      <wa-tab slot="nav" panel="first">Erste Seite</wa-tab>
                      <wa-tab slot="nav" panel="page">Folgeseiten</wa-tab>
                      <wa-tab-panel name="first"
                        >${this.setup(l.first, families, true)}</wa-tab-panel
                      >
                      <wa-tab-panel name="page"
                        >${this.setup(l.page, families, false)}</wa-tab-panel
                      >
                    </wa-tab-group>
                  `
                : this.setup(l.page, families, false)
            }
          </div>
        </wa-card>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-layout-form': LayoutFormComponent;
  }
}
