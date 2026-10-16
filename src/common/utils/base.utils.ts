import { SignalWatcher } from '@lit-labs/signals';
import type { CSSResultGroup } from 'lit';
import { LitElement } from 'lit';

import styles from './base.utils.css';

/** Base for all elements: re-renders on signal reads, shared base styles */
export class ErngElement extends SignalWatcher(LitElement) {
  static override styles: CSSResultGroup = [styles];
}
