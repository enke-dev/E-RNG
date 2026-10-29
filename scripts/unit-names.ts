/**
 * Generates `src-tauri/data/unit-names.json`: a name for every code the EN 16931 rules accept as a
 * unit (`en16931::codes::UNIT_CODES`, tested in `units.rs`).
 *
 * English names come from the XRepository genericode lists, German ones from `GERMAN` below, which
 * also decides what counts as common and in which order those are offered first.
 *
 *   mkdir /some/empty/dir && cd /some/empty/dir
 *   curl -o rec20.xml https://www.xrepository.de/api/version_codeliste/urn:xoev-de:kosit:codeliste:rec20_3/genericode
 *   curl -o rec21.xml https://www.xrepository.de/api/version_codeliste/urn:xoev-de:kosit:codeliste:rec21_2/genericode
 *   bun run scripts/unit-names.ts /some/empty/dir
 *
 * rec20 version 3 (UN/ECE Rec. 20 revision 17) and rec21 version 2 (Rec. 21 revision 11) together
 * hold exactly the 2162 codes of the crate's set; version 3 of rec21 lacks XSX.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { argv, stdout } from 'node:process';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const output = join(root, 'src-tauri', 'data', 'unit-names.json');

const SOURCE =
  'UN/ECE Recommendations 20 (revision 17) and 21 (revision 11) in English, from the XRepository ' +
  'of KoSIT: urn:xoev-de:kosit:codeliste:rec20_3 and urn:xoev-de:kosit:codeliste:rec21_2 ' +
  '(https://www.xrepository.de/details/urn:xoev-de:kosit:codeliste:rec20_3). The codes are those of ' +
  'en16931::codes::UNIT_CODES. German names for the common codes are curated by hand, they come ' +
  'first and take precedence.';

/** Common codes in the order they are offered first, with their German names */
const GERMAN: [string, string][] = [
  ['C62', 'Stück'],
  ['H87', 'Stück (Teil)'],
  ['HUR', 'Stunde'],
  ['MIN', 'Minute'],
  ['SEC', 'Sekunde'],
  ['DAY', 'Tag'],
  ['E49', 'Arbeitstag'],
  ['WEE', 'Woche'],
  ['MON', 'Monat'],
  ['ANN', 'Jahr'],
  ['LS', 'Pauschale'],
  ['E48', 'Serviceeinheit'],
  ['SET', 'Satz'],
  ['PR', 'Paar'],
  ['DZN', 'Dutzend'],
  ['XPK', 'Packung'],
  ['XBX', 'Box'],
  ['ZP', 'Seite'],
  ['P1', 'Prozent'],
  ['KGM', 'kg'],
  ['GRM', 'g'],
  ['TNE', 't'],
  ['MTR', 'm'],
  ['CMT', 'cm'],
  ['MMT', 'mm'],
  ['KMT', 'km'],
  ['MTK', 'm²'],
  ['MTQ', 'm³'],
  ['LTR', 'l'],
  ['MLT', 'ml'],
  ['KWH', 'kWh'],
  ['MWH', 'MWh'],
  ['KWT', 'kW'],
];

const entities: Record<string, string> = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'" };

const decode = (value: string) =>
  value.replace(/&(?:#(\d+)|#x([0-9a-f]+)|(\w+));/gi, (match, dec, hex, name) =>
    dec
      ? String.fromCodePoint(Number(dec))
      : hex
        ? String.fromCodePoint(parseInt(hex, 16))
        : (entities[name] ?? match)
  );

/** The code and name columns of a genericode list (the column ids differ in case between lists) */
function names(file: string): Map<string, string> {
  const rows = [...readFileSync(file, 'utf8').matchAll(/<Row>([\s\S]*?)<\/Row>/g)];
  return new Map(
    rows.map(([, row]) => {
      const cell = (column: string) =>
        decode(
          new RegExp(`ColumnRef="${column}"[^>]*>\\s*<SimpleValue>([^<]*)</SimpleValue>`, 'i').exec(
            row as string
          )?.[1] ?? ''
        ).trim();
      return [cell('code'), cell('name')] as const;
    })
  );
}

const dir = argv[2];
if (!dir) {
  throw new Error('usage: bun run scripts/unit-names.ts <dir with rec20.xml and rec21.xml>');
}

const english = new Map([...names(join(dir, 'rec20.xml')), ...names(join(dir, 'rec21.xml'))]);
const german = new Map(GERMAN);
const unknown = GERMAN.filter(([code]) => !english.has(code));
if (unknown.length > 0) {
  throw new Error(
    `German names for codes the lists do not have: ${unknown.map(([c]) => c).join(', ')}`
  );
}

const common = GERMAN.map(([code]) => code);
const rest = [...english.keys()].filter(code => !german.has(code)).sort();
// one entry per line: a diff of a new list stays readable
const lines = [...common, ...rest].map(code => {
  const de = german.get(code);
  const name = JSON.stringify(english.get(code));
  return `    { "code": "${code}", "name": ${name}${de ? `, "de": ${JSON.stringify(de)}` : ''} }`;
});
writeFileSync(
  output,
  `{\n  "source": ${JSON.stringify(SOURCE)},\n  "units": [\n${lines.join(',\n')}\n  ]\n}\n`
);
stdout.write(`${lines.length} names written to ${output}\n`);
