import nativeStyles from '@awesome.me/webawesome/dist/styles/native.css';
import { SignalWatcher } from '@lit-labs/signals';
import type { CSSResultGroup } from 'lit';
import { LitElement } from 'lit';

import baseStyles from './base.utils.css';

/**
 * Base for all elements: re-renders on signal reads, shared base styles. Web Awesome's styles of
 * native elements (tables, headings, lists, …) are global and do not reach into shadow roots.
 */
export class ErngElement extends SignalWatcher(LitElement) {
  static override styles: CSSResultGroup = [nativeStyles, baseStyles];

  /**
   * The element with more styles after the shared ones, so a component names its own stylesheet
   * in the class declaration.
   * @example class ListComponent extends ErngElement.withStyles(styles) {}
   */
  static withStyles<T extends typeof ErngElement>(this: T, ...more: CSSResultGroup[]): T {
    const inherited = this.styles;
    class Styled extends (this as typeof ErngElement) {
      static override styles: CSSResultGroup = [inherited, ...more];
    }
    return Styled as T;
  }
}
