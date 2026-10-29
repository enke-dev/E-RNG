import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/select/select.js';

import type WaSelect from '@awesome.me/webawesome/dist/components/select/select.js';
import { html, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';

import type { Format } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { text } from '../../utils/form.utils.js';
import styles from './format-form.component.css';

const DECIMALS: [string, string][] = [
  [',', 'Komma (1,5)'],
  ['.', 'Punkt (1.5)'],
];

const THOUSANDS: [string, string][] = [
  ['.', 'Punkt (1.234)'],
  [',', 'Komma (1,234)'],
  [' ', 'Leerzeichen (1 234)'],
  ['’', 'Apostroph (1’234)'],
  ['none', 'keiner (1234)'],
];

/** Patterns as dayjs has them, with how they look; anything else is the user's own */
const DATES: [string, string][] = [
  ['DD.MM.YYYY', '18.10.2026'],
  ['D.M.YYYY', '8.1.2026'],
  ['DD.MM.YY', '18.10.26'],
  ['YYYY-MM-DD', '2026-10-18'],
  ['DD/MM/YYYY', '18/10/2026'],
  ['MM/DD/YYYY', '10/18/2026'],
];

const LONG_DATES: [string, string][] = [
  ['D. MMMM YYYY', '8. Oktober 2026'],
  ['DD. MMMM YYYY', '08. Oktober 2026'],
  ['D. MMM YYYY', '8. Okt. 2026'],
  ['D MMMM YYYY', '8 Oktober 2026'],
];

const CUSTOM = '\u0000custom';

/**
 * How numbers, money and dates are written. Edits the given object.
 *
 * @fires change
 */
@customElement('erng-format-form')
export class FormatFormComponent extends ErngElement.withStyles(styles) {
  @property({ attribute: false })
  format!: Format;

  private changed() {
    this.requestUpdate();
    this.dispatchEvent(new CustomEvent('change', { bubbles: true, composed: true }));
  }

  private select(
    label: string,
    value: string,
    options: [string, string][],
    apply: (value: string) => void
  ) {
    return html`
      <wa-select
        label="${label}"
        .value="${value}"
        @change="${(e: Event) => {
          apply(String((e.target as WaSelect).value));
          this.changed();
        }}"
      >
        ${options.map(([id, name]) => html`<wa-option value="${id}">${name}</wa-option>`)}
      </wa-select>
    `;
  }

  private check(label: string, value: boolean, apply: (value: boolean) => void) {
    return html`
      <wa-checkbox
        ?checked="${value}"
        @change="${(e: Event) => {
          apply((e.target as HTMLInputElement).checked);
          this.changed();
        }}"
        >${label}</wa-checkbox
      >
    `;
  }

  /** A pattern from the list, or a field for one's own */
  private pattern(
    label: string,
    value: string,
    presets: [string, string][],
    apply: (value: string) => void
  ) {
    const known = presets.some(([id]) => id === value) && !this.customDate.has(label);
    const options: [string, string][] = [
      ...presets.map(([id, example]): [string, string] => [id, example]),
      [CUSTOM, 'Eigenes Format …'],
    ];
    return html`
      ${this.select(label, known ? value : CUSTOM, options, picked => {
        // the field of one's own starts with what is there
        apply(picked === CUSTOM ? value : picked);
        if (picked === CUSTOM) {
          this.customDate.add(label);
        } else {
          this.customDate.delete(label);
        }
      })}
      ${
        !known
          ? html`
              <wa-input
                label="${label} (eigenes Format)"
                hint="YYYY oder YY Jahr, MMMM oder MMM Monatsname, MM oder M Monat, DD oder D Tag, [Text] bleibt wie er ist"
                .value="${value}"
                @input="${(e: Event) => {
                  apply(text(e));
                  this.changed();
                }}"
              ></wa-input>
            `
          : nothing
      }
    `;
  }

  /** Which date fields show their own field although the value is one of the list */
  private customDate = new Set<string>();

  override render() {
    const f = this.format;
    return html`
      <div class="stack">
        <div class="fields">
          ${this.select('Dezimaltrenner', f.decimal, DECIMALS, v => (f.decimal = v))}
          ${this.select(
            'Tausendertrenner',
            f.thousands === '' ? 'none' : f.thousands,
            THOUSANDS,
            v => (f.thousands = v === 'none' ? '' : v)
          )}
          ${this.select(
            'Währung als',
            f.currency_symbol ? 'symbol' : 'code',
            [
              ['symbol', 'Symbol (€)'],
              ['code', 'Kürzel (EUR)'],
            ],
            v => (f.currency_symbol = v === 'symbol')
          )}
          ${this.select(
            'Währung steht',
            f.currency_after ? 'after' : 'before',
            [
              ['after', 'nach dem Betrag'],
              ['before', 'vor dem Betrag'],
            ],
            v => (f.currency_after = v === 'after')
          )}
        </div>
        <div class="checks">
          ${this.check(
            'Leerzeichen zwischen Betrag und Währung',
            f.currency_space,
            v => (f.currency_space = v)
          )}
          ${this.check('Leerzeichen vor dem Prozentzeichen (19 %)', f.percent_space, v => (f.percent_space = v))}
          ${this.check('Echtes Minuszeichen (−) bei Abzügen', f.true_minus, v => (f.true_minus = v))}
          ${this.check('IBAN in Vierergruppen', f.iban_groups, v => (f.iban_groups = v))}
        </div>
        <div class="fields">
          ${this.pattern('Datum', f.date, DATES, v => (f.date = v))}
          ${this.pattern('Datum ausgeschrieben', f.date_long, LONG_DATES, v => (f.date_long = v))}
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-format-form': FormatFormComponent;
  }
}
