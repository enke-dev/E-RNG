import table from '../../../src-tauri/data/invisible-chars.json' with { type: 'json' };

/**
 * The characters that come along with a copy and paste and cannot be seen or behave unlike they
 * look, by character: the code the layout preview marks them with and what cleaning makes of them.
 * The table is the one `invisible.rs` reads.
 */
const KNOWN = new Map(
  table.chars.map(({ code, label, to }) => [
    String.fromCodePoint(parseInt(code, 16)),
    { label, to },
  ])
);

/** Control characters but the tab and the line feed, which a text can have, are counted together */
const CONTROL_LABEL = 'Steuerzeichen';

const isControl = (char: string) => {
  const code = char.codePointAt(0) ?? 0;
  return (code < 0x20 && code !== 0x09 && code !== 0x0a) || (code >= 0x7f && code <= 0x9f);
};

const describe = (char: string): { label: string; to: string } | undefined =>
  KNOWN.get(char) ?? (isControl(char) ? { label: CONTROL_LABEL, to: '' } : undefined);

/** The invisible characters of a text with how often each occurs, in the order of their first appearance */
export const invisibleIn = (value: string): { label: string; count: number }[] => {
  const counts = new Map<string, number>();
  [...value].forEach(char => {
    const known = describe(char);
    if (known) {
      counts.set(known.label, (counts.get(known.label) ?? 0) + 1);
    }
  });
  return [...counts].map(([label, count]) => ({ label, count }));
};

/** The text with no-break and other special spaces as normal ones, line separators as line breaks and the rest gone */
export const cleanInvisible = (value: string): string =>
  [...value].map(char => describe(char)?.to ?? char).join('');

/** Cleaned for a field of one line, which has no use for line breaks and tabs */
export const cleanInvisibleLine = (value: string): string =>
  cleanInvisible(value).replace(/[\n\t]/g, ' ');
