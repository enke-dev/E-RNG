import type WaInput from '@awesome.me/webawesome/dist/components/input/input.js';

/** The text of a Web Awesome input, select or textarea event */
export const text = (event: Event): string => (event.target as WaInput).value ?? '';

/** Empty fields are stored as `null`, not as an empty string */
export const nullable = (value: string): string | null => (value.trim() === '' ? null : value);

/** Today as `2026-10-17` in the local time zone, for dates the user has not touched yet */
export const today = (): string => {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
};
