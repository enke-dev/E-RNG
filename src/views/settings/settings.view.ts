import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/card/card.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/textarea/textarea.js';
import '../../common/components/address-fields/address-fields.component.js';

import { open } from '@tauri-apps/plugin-dialog';
import type { CSSResultGroup } from 'lit';
import { html, nothing } from 'lit';
import { customElement, state } from 'lit/decorators.js';

import type { Settings } from '../../common/utils/api.utils.js';
import { api, errorText } from '../../common/utils/api.utils.js';
import { ErngElement } from '../../common/utils/base.utils.js';
import { nullable, text } from '../../common/utils/form.utils.js';
import styles from './settings.view.css';

@customElement('erng-settings')
export class SettingsView extends ErngElement {
  static override styles: CSSResultGroup = [ErngElement.styles, styles];

  @state() private settings: Settings | undefined;
  @state() private message: { variant: 'success' | 'danger'; text: string } | undefined;

  override connectedCallback() {
    super.connectedCallback();
    api.settingsGet().then(
      settings => (this.settings = settings),
      (error: unknown) => (this.message = { variant: 'danger', text: errorText(error) })
    );
  }

  private edit(change: (settings: Settings) => void) {
    if (!this.settings) {
      return;
    }
    change(this.settings);
    this.message = undefined;
    this.requestUpdate();
  }

  private seller(
    key: 'name' | 'contact_name' | 'phone' | 'email' | 'iban',
    label: string,
    type = 'text'
  ) {
    const settings = this.settings as Settings;
    return html`<wa-input
      label="${label}"
      type="${type}"
      .value="${settings.seller[key]}"
      @input="${(e: Event) => this.edit(s => (s.seller[key] = text(e)))}"
    ></wa-input>`;
  }

  private optional(key: 'vat_id' | 'tax_number' | 'bic' | 'account_holder', label: string) {
    const settings = this.settings as Settings;
    return html`<wa-input
      label="${label}"
      .value="${settings.seller[key] ?? ''}"
      @input="${(e: Event) => this.edit(s => (s.seller[key] = nullable(text(e))))}"
    ></wa-input>`;
  }

  private async chooseLogo() {
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Bild', extensions: ['png', 'jpg', 'jpeg', 'svg'] }],
    });
    if (typeof picked === 'string') {
      this.edit(s => (s.logo = picked));
    }
  }

  private async save() {
    const settings = this.settings;
    if (!settings) {
      return;
    }
    const rate = Number(settings.tax_rate.replace(',', '.'));
    if (!Number.isFinite(rate) || rate <= 0 || rate > 100) {
      this.message = {
        variant: 'danger',
        text: 'Der Steuersatz muss zwischen 0 und 100 liegen (in Prozent).',
      };
      return;
    }
    try {
      await api.settingsSave(settings);
      this.message = {
        variant: 'success',
        text: 'Gespeichert. Bereits exportierte Rechnungen behalten ihre alten Angaben.',
      };
    } catch (error) {
      this.message = { variant: 'danger', text: errorText(error) };
    }
  }

  override render() {
    const settings = this.settings;
    if (!settings) {
      return html`${this.message ? html`<wa-callout variant="danger">${this.message.text}</wa-callout>` : nothing}`;
    }

    return html`
      <div class="stack">
        <h2>Einstellungen</h2>

        <wa-card>
          <div slot="header">Unternehmen</div>
          <div class="stack">
            ${this.seller('name', 'Name des Unternehmens')}
            <erng-address-fields
              .address="${settings.seller.address}"
              @change="${() => this.edit(() => undefined)}"
            ></erng-address-fields>
            <div class="grid">
              ${this.optional('vat_id', 'USt-IdNr.')} ${this.optional('tax_number', 'Steuernummer')}
            </div>
            <div class="grid">
              ${this.seller('contact_name', 'Ansprechpartner')}
              ${this.seller('phone', 'Telefon', 'tel')} ${this.seller('email', 'E-Mail', 'email')}
            </div>
            <wa-textarea
              label="Zusatzangaben im Fuß (Handelsregister, Geschäftsführer …)"
              rows="2"
              .value="${settings.footer ?? ''}"
              @input="${(e: Event) => this.edit(s => (s.footer = nullable(text(e))))}"
            ></wa-textarea>
            <div class="logo">
              <wa-button @click="${this.chooseLogo}">Logo wählen …</wa-button>
              ${
                settings.logo
                  ? html`<span class="muted">${settings.logo.split('/').pop()}</span>
                      <wa-button
                        appearance="plain"
                        @click="${() => this.edit(s => (s.logo = null))}"
                        >Entfernen</wa-button
                      >`
                  : nothing
              }
            </div>
          </div>
        </wa-card>

        <wa-card>
          <div slot="header">Bankverbindung</div>
          <div class="grid">
            ${this.seller('iban', 'IBAN')} ${this.optional('bic', 'BIC')}
            ${this.optional('account_holder', 'Kontoinhaber')}
          </div>
        </wa-card>

        <wa-card>
          <div slot="header">Rechnungen</div>
          <div class="grid">
            <wa-input
              label="Umsatzsteuersatz (%)"
              inputmode="decimal"
              .value="${settings.tax_rate}"
              @input="${(e: Event) => this.edit(s => (s.tax_rate = text(e)))}"
            ></wa-input>
            <wa-input
              label="Zahlungsziel (Tage)"
              type="number"
              min="0"
              .value="${String(settings.payment_days)}"
              @input="${(e: Event) => this.edit(s => (s.payment_days = Math.max(0, Number(text(e)) || 0)))}"
            ></wa-input>
            <wa-input
              label="Präfix der Rechnungsnummer"
              hint="z. B. RE- ergibt RE-2026-001"
              .value="${settings.number_prefix}"
              @input="${(e: Event) => this.edit(s => (s.number_prefix = text(e)))}"
            ></wa-input>
          </div>
        </wa-card>

        ${this.message ? html`<wa-callout variant="${this.message.variant}">${this.message.text}</wa-callout>` : nothing}
        <div class="row end">
          <wa-button variant="brand" @click="${this.save}">Speichern</wa-button>
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-settings': SettingsView;
  }
}
