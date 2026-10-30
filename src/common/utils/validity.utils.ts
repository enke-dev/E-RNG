import type WaInput from '@awesome.me/webawesome/dist/components/input/input.js';
import { nothing } from 'lit';
import type { ElementPart, PartInfo } from 'lit/directive.js';
import { Directive, directive, PartType } from 'lit/directive.js';

import type { Check } from './validation.utils.js';

/**
 * Checks a Web Awesome input and shows what is wrong: the field is marked (`data-invalid`, styled
 * in `base.utils.css`, Web Awesome has no look for it) and the error stands in its hint, which else
 * shows `hint`, how the value should look. A stored value is judged at once, what is being typed
 * only when the field is left; from then on every key checks again, so a fixed value is fine at once.
 *
 * @example html`<wa-input .value="${v}" ${validity(iban, v, 'z. B. DE02 1203 …')}></wa-input>`
 */
class ValidityDirective extends Directive {
  #field: WaInput | undefined;
  #check: Check = () => undefined;
  #hint = '';
  /** An error is showing, so each key checks again */
  #shown = false;

  constructor(part: PartInfo) {
    super(part);
    if (part.type !== PartType.ELEMENT) {
      throw new Error('validity() belongs on an input');
    }
  }

  override render(_check: Check, _value: string | null | undefined, _hint?: string) {
    return nothing;
  }

  override update(part: ElementPart, [check, value, hint = '']: Parameters<this['render']>) {
    const field = part.element as WaInput;
    if (!this.#field) {
      this.#field = field;
      field.addEventListener('input', () => this.#shown && this.#show(field.value ?? ''));
      field.addEventListener('focusout', () => this.#show(field.value ?? ''));
    }
    this.#check = check;
    this.#hint = hint;
    if (this.#shown || !field.matches(':focus-within')) {
      this.#show(value ?? '');
    } else {
      // while the first value is typed the field stays quiet, it only knows already
      field.setCustomValidity(check(value ?? '') ?? '');
      field.hint = hint;
    }
    return nothing;
  }

  #show(value: string) {
    const field = this.#field;
    if (!field) {
      return;
    }
    const problem = this.#check(value);
    field.setCustomValidity(problem ?? '');
    field.hint = problem ?? this.#hint;
    field.toggleAttribute('data-invalid', problem !== undefined);
    this.#shown = problem !== undefined;
  }
}

export const validity = directive(ValidityDirective);
