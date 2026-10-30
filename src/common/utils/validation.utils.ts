import type { CountryCode } from 'libphonenumber-js/max';
import { parsePhoneNumberFromString, validatePhoneNumberLength } from 'libphonenumber-js/max';

/**
 * What is wrong with what was typed into a field, as a sentence for the form, `undefined` when it
 * is fine. An empty field is fine for every check: whether it must be filled in is up to the field
 * (`required`) and to the checks before an invoice is issued.
 */
export type Check = (value: string) => string | undefined;

const compact = (value: string) => value.replace(/\s+/g, '').toUpperCase();

/** The length of an IBAN by its country, for the SEPA countries */
const IBAN_LENGTHS: Record<string, number> = {
  AD: 24,
  AT: 20,
  BE: 16,
  BG: 22,
  CH: 21,
  CY: 28,
  CZ: 24,
  DE: 22,
  DK: 18,
  EE: 20,
  ES: 24,
  FI: 18,
  FR: 27,
  GB: 22,
  GI: 23,
  GR: 27,
  HR: 21,
  HU: 28,
  IE: 22,
  IS: 26,
  IT: 27,
  LI: 21,
  LT: 20,
  LU: 20,
  LV: 21,
  MC: 27,
  MT: 31,
  NL: 18,
  NO: 15,
  PL: 28,
  PT: 25,
  RO: 24,
  SE: 24,
  SI: 19,
  SK: 24,
  SM: 27,
  VA: 22,
};

/** ISO 7064 MOD 97-10: the letters count as 10 to 35, the first four characters go to the end */
const mod97 = (text: string) =>
  [...`${text.slice(4)}${text.slice(0, 4)}`]
    .map(c => parseInt(c, 36).toString())
    .join('')
    .split('')
    .reduce((rest, digit) => (rest * 10 + Number(digit)) % 97, 0);

export const iban: Check = value => {
  const text = compact(value);
  if (text === '') {
    return undefined;
  }
  if (!/^[A-Z]{2}\d{2}[A-Z0-9]+$/.test(text)) {
    return 'Eine IBAN beginnt mit zwei Buchstaben für das Land und zwei Prüfziffern, danach nur Buchstaben und Ziffern.';
  }
  const length = IBAN_LENGTHS[text.slice(0, 2)];
  if (length !== undefined && text.length !== length) {
    return `Eine IBAN aus ${text.slice(0, 2)} hat ${length} Zeichen, diese hat ${text.length}.`;
  }
  if (text.length < 15 || text.length > 34) {
    return `Eine IBAN hat 15 bis 34 Zeichen, diese hat ${text.length}.`;
  }
  return mod97(text) === 1
    ? undefined
    : 'Die Prüfziffern der IBAN stimmen nicht, wahrscheinlich ist ein Zeichen vertippt.';
};

export const bic: Check = value => {
  const text = compact(value);
  if (text === '' || /^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$/.test(text)) {
    return undefined;
  }
  return 'Eine BIC hat 8 oder 11 Zeichen: vier Buchstaben für die Bank, zwei für das Land, dann Ort und Filiale.';
};

export const email: Check = value => {
  const text = value.trim();
  if (text === '' || /^[^\s@]+@[^\s@]+\.[^\s@.]{2,}$/.test(text)) {
    return undefined;
  }
  return 'Eine E-Mail-Adresse sieht so aus: name@firma.de';
};

/** With or without `https://`, as long as there is a domain */
export const website: Check = value => {
  const text = value.trim();
  if (text === '') {
    return undefined;
  }
  const host = text.replace(/^https?:\/\//i, '').split(/[/?#]/)[0] ?? '';
  return /^([\p{L}\d]([\p{L}\d-]*[\p{L}\d])?\.)+\p{L}{2,}(:\d+)?$/u.test(host)
    ? undefined
    : 'Eine Website sieht so aus: www.firma.de oder https://firma.de';
};

/** Phone numbers by the country of their owner, which a number without `+49` belongs to */
export const phone =
  (country: string): Check =>
  value => {
    const text = value.trim();
    if (text === '') {
      return undefined;
    }
    const home = /^[A-Z]{2}$/.test(country) ? (country as CountryCode) : undefined;
    if (!home && !text.startsWith('+') && !text.startsWith('00')) {
      return 'Ohne Land in der Adresse braucht die Nummer die Landesvorwahl, z. B. +49 221 1234567.';
    }
    const number = parsePhoneNumberFromString(text, home);
    if (number?.isValid()) {
      return undefined;
    }
    switch (validatePhoneNumberLength(text, home)) {
      case 'TOO_SHORT':
        return 'Die Nummer ist zu kurz, fehlt die Vorwahl?';
      case 'TOO_LONG':
        return 'Die Nummer ist zu lang.';
      case 'INVALID_COUNTRY':
        return 'Die Landesvorwahl gibt es nicht.';
      case 'NOT_A_NUMBER':
        return 'Das ist keine Telefonnummer.';
      default:
        return 'Die Nummer gibt es so nicht, ist die Vorwahl richtig?';
    }
  };

/** What a VAT id looks like after its country code, by the VIES formats */
const VAT_IDS: Record<string, [RegExp, string]> = {
  AT: [/^U\d{8}$/, 'ATU und 8 Ziffern'],
  BE: [/^[01]\d{9}$/, 'BE und 10 Ziffern'],
  BG: [/^\d{9,10}$/, 'BG und 9 oder 10 Ziffern'],
  CY: [/^\d{8}[A-Z]$/, 'CY, 8 Ziffern und ein Buchstabe'],
  CZ: [/^\d{8,10}$/, 'CZ und 8 bis 10 Ziffern'],
  DE: [/^\d{9}$/, 'DE und 9 Ziffern'],
  DK: [/^\d{8}$/, 'DK und 8 Ziffern'],
  EE: [/^\d{9}$/, 'EE und 9 Ziffern'],
  EL: [/^\d{9}$/, 'EL und 9 Ziffern'],
  ES: [/^[A-Z\d]\d{7}[A-Z\d]$/, 'ES und 9 Zeichen'],
  FI: [/^\d{8}$/, 'FI und 8 Ziffern'],
  FR: [/^[A-HJ-NP-Z\d]{2}\d{9}$/, 'FR, 2 Zeichen und 9 Ziffern'],
  HR: [/^\d{11}$/, 'HR und 11 Ziffern'],
  HU: [/^\d{8}$/, 'HU und 8 Ziffern'],
  IE: [/^(\d{7}[A-W][A-I]?|\d[A-Z+*]\d{5}[A-W])$/, 'IE und 8 oder 9 Zeichen'],
  IT: [/^\d{11}$/, 'IT und 11 Ziffern'],
  LT: [/^(\d{9}|\d{12})$/, 'LT und 9 oder 12 Ziffern'],
  LU: [/^\d{8}$/, 'LU und 8 Ziffern'],
  LV: [/^\d{11}$/, 'LV und 11 Ziffern'],
  MT: [/^\d{8}$/, 'MT und 8 Ziffern'],
  NL: [/^\d{9}B\d{2}$/, 'NL, 9 Ziffern, B und 2 Ziffern'],
  PL: [/^\d{10}$/, 'PL und 10 Ziffern'],
  PT: [/^\d{9}$/, 'PT und 9 Ziffern'],
  RO: [/^\d{2,10}$/, 'RO und 2 bis 10 Ziffern'],
  SE: [/^\d{12}$/, 'SE und 12 Ziffern'],
  SI: [/^\d{8}$/, 'SI und 8 Ziffern'],
  SK: [/^\d{10}$/, 'SK und 10 Ziffern'],
  XI: [/^(\d{9}|\d{12}|GD\d{3}|HA\d{3})$/, 'XI und 9 oder 12 Ziffern'],
  CH: [/^E\d{9}(MWST|TVA|IVA)?$/, 'CHE, 9 Ziffern und MWST'],
  GB: [/^(\d{9}|\d{12}|GD\d{3}|HA\d{3})$/, 'GB und 9 oder 12 Ziffern'],
  NO: [/^\d{9}(MVA)?$/, 'NO, 9 Ziffern und MVA'],
};

/** The check digit of a German VAT id, ISO 7064 MOD 11,10 over the first eight digits */
const germanCheckDigit = (digits: string) => {
  const product = [...digits.slice(0, 8)].reduce((product, digit) => {
    const sum = (Number(digit) + product) % 10 || 10;
    return (2 * sum) % 11;
  }, 10);
  return (11 - product) % 10;
};

/** Spaces, dots and dashes as some write them are fine, they are not part of the id */
export const vatId: Check = value => {
  const text = compact(value).replace(/[.-]/g, '');
  if (text === '') {
    return undefined;
  }
  if (!/^[A-Z]{2}[A-Z\d+*]{2,14}$/.test(text)) {
    return 'Eine USt-IdNr. beginnt mit zwei Buchstaben für das Land, z. B. DE123456789.';
  }
  const [pattern, shape] = VAT_IDS[text.slice(0, 2)] ?? [];
  if (pattern && shape && !pattern.test(text.slice(2))) {
    return `Eine USt-IdNr. aus ${text.slice(0, 2)} besteht aus ${shape}.`;
  }
  if (text.startsWith('DE') && germanCheckDigit(text.slice(2)) !== Number(text.at(-1))) {
    return 'Die Prüfziffer der USt-IdNr. stimmt nicht, wahrscheinlich ist eine Ziffer vertippt.';
  }
  return undefined;
};

/** German tax numbers only: other countries write theirs in too many ways */
export const taxNumber =
  (country: string): Check =>
  value => {
    const text = value.trim();
    if (text === '' || country !== 'DE') {
      return undefined;
    }
    const digits = text.replace(/[\s/]/g, '');
    if (/^\d{10,13}$/.test(digits)) {
      return undefined;
    }
    return 'Eine Steuernummer hat 10 bis 13 Ziffern, z. B. 21/815/08150 oder 2181508150.';
  };

/** How many digits a post code has, for the countries whose codes are only digits */
const POST_CODES: Record<string, number> = { DE: 5, AT: 4, CH: 4, LI: 4, BE: 4, DK: 4, LU: 4 };

export const postCode =
  (country: string): Check =>
  value => {
    const text = value.trim();
    const digits = POST_CODES[country];
    if (text === '' || digits === undefined || new RegExp(`^\\d{${digits}}$`).test(text)) {
      return undefined;
    }
    return `Eine Postleitzahl in ${country} hat ${digits} Ziffern.`;
  };

const regions = new Intl.DisplayNames(['de'], { type: 'region' });

/** Withdrawn, reserved or no country, ICU names them all the same (as `render/countries.rs` has it) */
const NOT_COUNTRIES = new Set(
  'AC AN BU CP CQ CS DD DG DY EA EU EZ FX HV IC NH QO RH SU TA TP UK UN VD XA XB YD YU ZR ZZ'.split(
    ' '
  )
);

/** The German name of a country code, `undefined` for a code there is no country for */
export const countryName = (code: string): string | undefined => {
  if (!/^[A-Z]{2}$/.test(code) || NOT_COUNTRIES.has(code)) {
    return undefined;
  }
  try {
    const name = regions.of(code);
    return name && name !== code ? name : undefined;
  } catch {
    return undefined;
  }
};

export const country: Check = value => {
  const text = value.trim();
  if (text === '' || countryName(text)) {
    return undefined;
  }
  return /^[A-Z]{2}$/.test(text)
    ? `Ein Land mit dem Code ${text} gibt es nicht.`
    : 'Zwei Buchstaben, z. B. DE, AT oder CH.';
};
