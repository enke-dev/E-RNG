import '@awesome.me/webawesome/dist/components/toast/toast.js';
import '@awesome.me/webawesome/dist/components/toast-item/toast-item.js';

import type WaToast from '@awesome.me/webawesome/dist/components/toast/toast.js';

import { errorText } from './api.utils.js';

export type NotifyVariant = 'success' | 'warning' | 'danger' | 'neutral';

let stack: WaToast | undefined;

/** One stack of toasts for the whole app, in the page itself so no view or dialog hides it */
function toasts(): WaToast {
  if (!stack) {
    stack = document.createElement('wa-toast');
    stack.placement = 'bottom-start';
    document.body.append(stack);
  }
  return stack;
}

/**
 * Tells the user what just happened, bottom left, for a moment: the answer to something they did
 * (saved, failed). What stays true while they work (a locked invoice, what is still missing)
 * belongs on the page as a callout, not here.
 */
export function notify(message: string, variant: NotifyVariant = 'success') {
  // a failure stays longer, it may need reading twice
  void toasts().create(message, { variant, duration: variant === 'danger' ? 10_000 : 4_000 });
}

export function notifyError(error: unknown) {
  notify(errorText(error), 'danger');
}
