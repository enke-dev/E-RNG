import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/callout/callout.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/icon/icon.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '../page/page.component.js';

import { open, save } from '@tauri-apps/plugin-dialog';
import { html, nothing } from 'lit';
import { customElement, state } from 'lit/decorators.js';

import type { BackupStatus, DataCounts } from '../../utils/api.utils.js';
import { api } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { text, today } from '../../utils/form.utils.js';
import { formatMoment, megabytes } from '../../utils/format.utils.js';
import { notify, notifyError } from '../../utils/notify.utils.js';
import { ask } from '../ask/ask.component.js';
import { reloadReplaced, revealLink } from '../replaced/replaced.component.js';
import styles from './backup.component.css';

const EXTENSION = 'erng-backup';

/** The word that confirms removing everything, the backend asks for it as well */
const REMOVE_WORD = 'ENTFERNEN';

const FILTERS = [{ name: 'e-rng-Datensicherung', extensions: [EXTENSION] }];

const plural = (count: number, one: string, many: string) => `${count} ${count === 1 ? one : many}`;

const countsText = (counts: DataCounts) =>
  [
    plural(counts.contexts, 'Kontext', 'Kontexte'),
    plural(counts.invoices, 'Rechnung', 'Rechnungen'),
    plural(counts.offers, 'Angebot', 'Angebote'),
    plural(counts.layouts, 'Layout', 'Layouts'),
  ].join(', ');

/**
 * Backup of all data, app-wide (not per context): make one, restore one, remove everything. A
 * restore and a removal first save the data in use as a safety copy, then replace it; the app
 * reloads at once so no view shows the old data (`reloadReplaced`).
 */
@customElement('erng-backup')
export class BackupComponent extends ErngElement.withStyles(styles) {
  @state() private status: BackupStatus | undefined;
  /** What is running, buttons wait for it */
  @state() private busy: 'backup' | 'restore' | 'remove' | undefined;
  /** The question of removing everything: what is typed so far */
  @state() private removing: { word: string } | undefined;

  override connectedCallback() {
    super.connectedCallback();
    void this.load();
  }

  private async load() {
    try {
      this.status = await api.backupStatus();
    } catch (error) {
      notifyError(error);
    }
  }

  private async discard(folder: string) {
    const sure = await ask(
      `Den Ordner mit den damals ersetzten Daten endgültig löschen? Prüfe vorher, ob alles da ist, was du davon brauchst: ${folder}`,
      { title: 'Ordner löschen', okLabel: 'Löschen', cancelLabel: 'Abbrechen', danger: true }
    );
    if (!sure) {
      return;
    }
    try {
      await api.backupDiscardInterrupted(folder);
      await this.load();
    } catch (error) {
      notifyError(error);
    }
  }

  /** What a restore or removal that was cut off left behind, the user decides about it */
  private interrupted() {
    const folders = this.status?.interrupted ?? [];
    return folders.length > 0
      ? html`
          <wa-callout variant="warning">
            <wa-icon slot="icon" name="warning"></wa-icon>
            <div class="stack">
              <span>
                Eine Wiederherstellung oder das Entfernen aller Daten wurde unterbrochen. Die Daten
                von davor liegen noch hier, falls etwas fehlt:
              </span>
              ${folders.map(
                folder => html`
                  <div class="action">
                    ${revealLink(folder)}
                    <wa-button
                      size="s"
                      appearance="outlined"
                      @click="${() => this.discard(folder)}"
                    >
                      Löschen
                    </wa-button>
                  </div>
                `
              )}
            </div>
          </wa-callout>
        `
      : nothing;
  }

  private async create() {
    this.busy = 'backup';
    try {
      const path = await save({
        defaultPath: `${today()}-e-rng-backup.${EXTENSION}`,
        filters: FILTERS,
      });
      if (path) {
        const made = await api.backupCreate(path);
        notify(
          `Die Datensicherung ist gespeichert (${plural(made.files, 'Datei', 'Dateien')}, ${megabytes(made.bytes)}).`
        );
        await this.load();
      }
    } catch (error) {
      notifyError(error);
    } finally {
      this.busy = undefined;
    }
  }

  private async restore() {
    this.busy = 'restore';
    try {
      const path = await open({ multiple: false, directory: false, filters: FILTERS });
      if (!path) {
        return;
      }
      const sure = await ask(
        `Alle jetzigen Daten werden durch die der Sicherung ersetzt. Vorher legt e-rng eine Kopie der jetzigen Daten an: ${this.status?.safety_dir ?? 'im Ordner „backups“ der App-Daten'}. Fortfahren?`,
        {
          title: 'Datensicherung einspielen',
          danger: true,
          okLabel: 'Einspielen',
          cancelLabel: 'Abbrechen',
        }
      );
      if (!sure) {
        return;
      }
      const restored = await api.backupRestore(path);
      reloadReplaced({
        title: 'Datensicherung eingespielt',
        lines: [
          `Die Daten der Sicherung vom ${formatMoment(restored.created_at)} sind eingespielt: ${countsText(restored.counts)}.`,
        ],
        safetyCopy: restored.safety_copy,
        problems: restored.problems,
      });
    } catch (error) {
      notifyError(error);
    } finally {
      this.busy = undefined;
    }
  }

  private async removeAll() {
    const word = this.removing?.word.trim();
    if (word !== REMOVE_WORD) {
      return;
    }
    this.busy = 'remove';
    try {
      const removed = await api.dataRemove(word);
      reloadReplaced({
        title: 'Alle Daten entfernt',
        lines: ['e-rng ist wieder im Zustand nach der Installation.'],
        safetyCopy: removed.safety_copy,
        problems: [],
      });
    } catch (error) {
      notifyError(error);
    } finally {
      this.busy = undefined;
    }
  }

  private removeDialog() {
    const asked = this.removing;
    const waiting = this.busy === 'remove';
    return html`
      <wa-dialog
        ?open="${asked !== undefined}"
        label="Alle Daten entfernen"
        @wa-after-hide="${(e: Event) => e.target === e.currentTarget && (this.removing = undefined)}"
      >
        ${
          asked
            ? html`
                <div class="stack">
                  <wa-callout variant="danger">
                    Entfernt werden alle Kontexte, Kunden, Rechnungen und Angebote mit ihren PDF-
                    und XML-Dateien, alle Layouts samt Schriften und Bildern und alle Einstellungen.
                  </wa-callout>
                  <wa-callout variant="warning">
                    Rechnungen müssen Jahre aufbewahrt werden (Aufbewahrungspflicht: Buchungsbelege
                    8 Jahre, Bücher 10 Jahre). Entferne nur, wenn eine Sicherung an einem anderen
                    Ort liegt.
                  </wa-callout>
                  <span>
                    Vorher legt e-rng eine Kopie der jetzigen Daten an, in
                    ${revealLink(this.status?.safety_dir ?? '')} als „…-vor-entfernen“. Dort bleiben
                    die letzten drei liegen.
                  </span>
                  <wa-input
                    label="${`Zur Bestätigung ${REMOVE_WORD} eingeben`}"
                    autocomplete="off"
                    ?disabled="${waiting}"
                    .value="${asked.word}"
                    @input="${(e: Event) => (this.removing = { word: text(e) })}"
                  ></wa-input>
                </div>
              `
            : nothing
        }
        <div slot="footer" class="dialog-footer">
          <wa-button
            appearance="outlined"
            ?disabled="${waiting}"
            @click="${() => (this.removing = undefined)}"
          >
            Abbrechen
          </wa-button>
          <wa-button
            variant="danger"
            ?loading="${waiting}"
            ?disabled="${asked?.word.trim() !== REMOVE_WORD}"
            @click="${this.removeAll}"
          >
            Alle Daten entfernen
          </wa-button>
        </div>
      </wa-dialog>
    `;
  }

  override render() {
    const last = this.status?.last_backup_at;
    const busy = this.busy !== undefined;
    return html`
      <erng-page heading="Datensicherung">
        ${this.interrupted()}
        <section>
          <div class="last">
            <strong>Letzte Sicherung</strong>
            <span>${this.status ? (last ? formatMoment(last) : 'Noch keine Sicherung') : '…'}</span>
          </div>
          <div class="action">
            <wa-button
              variant="brand"
              ?loading="${this.busy === 'backup'}"
              ?disabled="${busy}"
              @click="${this.create}"
            >
              Datensicherung erstellen
            </wa-button>
            <span class="hint"
              >Schreibt alle Daten in eine Datei, die du ablegst, wo du willst.</span
            >
          </div>
          <div class="action">
            <wa-button
              appearance="outlined"
              ?loading="${this.busy === 'restore'}"
              ?disabled="${busy}"
              @click="${this.restore}"
            >
              Datensicherung einspielen
            </wa-button>
            <span class="hint">
              Ersetzt alle jetzigen Daten durch die einer Sicherung, nach einer Kopie der jetzigen.
            </span>
          </div>
          <wa-callout variant="brand" size="small">
            <wa-icon slot="icon" name="info"></wa-icon>
            Eine Sicherung ist eine vollständige Kopie aller Daten, auch der Rechnungen und Angebote
            als PDF- und XML-Datei.<br>
            Bewahre sie zusätzlich an einem anderen Ort auf, etwa auf einer externen Festplatte oder
            in einem Cloud-Ordner, nicht nur auf diesem Rechner.
          </wa-callout>
        </section>
        <section class="danger">
          <strong>Alle Daten entfernen</strong>
          <div class="action">
            <wa-button
              variant="danger"
              appearance="outlined"
              ?disabled="${busy}"
              @click="${() => (this.removing = { word: '' })}"
            >
              Alle Daten entfernen
            </wa-button>
            <span class="hint">
              Setzt e-rng zurück auf den Anfang, wie nach der Installation, nach einer Kopie der
              jetzigen Daten.
            </span>
          </div>
        </section>
      </erng-page>
      ${this.removeDialog()}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-backup': BackupComponent;
  }
}
