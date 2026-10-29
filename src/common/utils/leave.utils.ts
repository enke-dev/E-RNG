import { ask } from '../components/ask/ask.component.js';

/**
 * A view with unsaved edits registers a check: would going to this address lose them? Whoever
 * navigates asks `confirmLeave` first, the user then decides to leave or to stay.
 */
type LeaveCheck = (target: string) => boolean;

let check: LeaveCheck | undefined;

/** Returns the function that takes the check back, call it when the view goes away */
export function registerLeaveCheck(fn: LeaveCheck) {
  check = fn;
  return () => {
    if (check === fn) {
      check = undefined;
    }
  };
}

/** Whether to go on to `target`: yes at once if nothing is lost, else as the user decides */
export async function confirmLeave(target: string): Promise<boolean> {
  if (!check?.(target)) {
    return true;
  }
  return ask('Es gibt ungespeicherte Änderungen. Beim Verlassen gehen sie verloren.', {
    title: 'Änderungen verwerfen?',
    danger: true,
    okLabel: 'Verlassen',
    cancelLabel: 'Bleiben',
  });
}
