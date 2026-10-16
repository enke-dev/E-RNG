import { computed, signal } from '@lit-labs/signals';

/** A global loading state */
export const isLoading = signal(false);

/** A global error state */
export const errors = signal<string[]>([]);
export const isErrored = computed(() => errors.get().length > 0);

/** The current route of the application */
export const currentRoute = signal<string>(location.pathname);

/** The color scheme is not a setting: `wa-dark` follows the system */
export function followSystemScheme() {
  const prefersDark = matchMedia('(prefers-color-scheme: dark)');
  const apply = () => document.documentElement.classList.toggle('wa-dark', prefersDark.matches);
  apply();
  prefersDark.addEventListener('change', apply);
}
