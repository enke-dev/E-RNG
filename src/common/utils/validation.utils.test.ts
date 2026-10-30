import { describe, expect, it } from 'vitest';

import {
  bic,
  country,
  countryName,
  email,
  iban,
  phone,
  postCode,
  taxNumber,
  vatId,
  website,
} from './validation.utils.js';

describe('an empty field', () => {
  it('is fine for every check, whether it is needed is up to the field', () => {
    const checks = [
      iban,
      bic,
      email,
      website,
      phone('DE'),
      vatId,
      taxNumber('DE'),
      postCode('DE'),
      country,
    ];
    expect(checks.map(check => check('  '))).toEqual(checks.map(() => undefined));
  });
});

describe('iban', () => {
  it('takes a valid IBAN with or without spaces, in any case', () => {
    expect(iban('DE02 1203 0000 0000 2020 51')).toBeUndefined();
    expect(iban('de89370400440532013000')).toBeUndefined();
    expect(iban('AT61 1904 3002 3457 3201')).toBeUndefined();
  });

  it('finds a typo by the check digits', () => {
    expect(iban('DE02 1203 0000 0000 2020 52')).toMatch(/Prüfziffern/);
    expect(iban('DE20 1203 0000 0000 2020 51')).toMatch(/Prüfziffern/);
  });

  it('knows the length of a country', () => {
    expect(iban('DE02 1203 0000 0000 2020 5')).toBe(
      'Eine IBAN aus DE hat 22 Zeichen, diese hat 21.'
    );
  });

  it('wants the country and the check digits first', () => {
    expect(iban('1203 0000 0000 2020 51')).toMatch(/zwei Buchstaben/);
    expect(iban('DE02 1203-0000')).toMatch(/nur Buchstaben und Ziffern/);
  });
});

describe('bic', () => {
  it('takes 8 or 11 characters', () => {
    expect(bic('BYLADEM1001')).toBeUndefined();
    expect(bic('cobadeff')).toBeUndefined();
    expect(bic('BYLADEM100')).toMatch(/8 oder 11/);
    expect(bic('1YLADEM1')).toMatch(/8 oder 11/);
  });
});

describe('email', () => {
  it('wants a name, an @ and a domain', () => {
    expect(email('rechnung@firma.de')).toBeUndefined();
    expect(email(' max.muster+re@büro.example ')).toBeUndefined();
    expect(email('rechnung@firma')).toMatch(/name@firma\.de/);
    expect(email('firma.de')).toMatch(/name@firma\.de/);
    expect(email('max muster@firma.de')).toMatch(/name@firma\.de/);
  });
});

describe('website', () => {
  it('takes a domain with or without its scheme and path', () => {
    expect(website('www.firma.de')).toBeUndefined();
    expect(website('https://firma.de/impressum')).toBeUndefined();
    expect(website('http://müller-bau.de')).toBeUndefined();
    expect(website('firma')).toMatch(/www\.firma\.de/);
    expect(website('https://')).toMatch(/www\.firma\.de/);
    expect(website('ftp://firma.de')).toMatch(/www\.firma\.de/);
  });
});

describe('phone', () => {
  it('reads a number without country code by the country of its owner', () => {
    expect(phone('DE')('0221 1234567')).toBeUndefined();
    expect(phone('DE')('0221/123456-0')).toBeUndefined();
    expect(phone('DE')('+49 (0)171 1234567')).toBeUndefined();
    expect(phone('AT')('01 5321234')).toBeUndefined();
  });

  it('takes any country with its code', () => {
    expect(phone('DE')('+43 1 5321234')).toBeUndefined();
    expect(phone('DE')('0041 44 668 18 00')).toBeUndefined();
  });

  it('says what is wrong', () => {
    expect(phone('DE')('123')).toMatch(/zu kurz/);
    expect(phone('DE')('0221 12')).toMatch(/Vorwahl richtig/);
    expect(phone('DE')('0221 1234567890123')).toMatch(/zu lang/);
    expect(phone('DE')('Zentrale')).toMatch(/keine Telefonnummer/);
    expect(phone('DE')('+999 1234567')).toMatch(/Landesvorwahl/);
  });

  it('needs the country code without a country in the address', () => {
    expect(phone('')('0221 1234567')).toMatch(/\+49 221/);
    expect(phone('')('+49 221 1234567')).toBeUndefined();
  });
});

describe('vatId', () => {
  it('takes ids of the right shape, spaces and dots aside', () => {
    expect(vatId('DE123475223')).toBeUndefined();
    expect(vatId('de 143 454 214')).toBeUndefined();
    expect(vatId('ATU12345678')).toBeUndefined();
    expect(vatId('NL123456789B01')).toBeUndefined();
    expect(vatId('CHE-123.456.789 MWST')).toBeUndefined();
    // a country without a known shape only needs its code
    expect(vatId('ZA4123456789')).toBeUndefined();
  });

  it('says what an id of its country looks like', () => {
    expect(vatId('DE12347522')).toBe('Eine USt-IdNr. aus DE besteht aus DE und 9 Ziffern.');
    expect(vatId('AT12345678')).toMatch(/ATU und 8 Ziffern/);
  });

  it('checks the check digit of a German id', () => {
    expect(vatId('DE123475224')).toMatch(/Prüfziffer/);
  });

  it('wants the country first', () => {
    expect(vatId('123475223')).toMatch(/zwei Buchstaben/);
  });
});

describe('taxNumber', () => {
  it('checks German tax numbers only', () => {
    expect(taxNumber('DE')('21/815/08150')).toBeUndefined();
    expect(taxNumber('DE')('2181508150')).toBeUndefined();
    expect(taxNumber('DE')('21/815/0815')).toMatch(/10 bis 13 Ziffern/);
    expect(taxNumber('AT')('12-345/6789')).toBeUndefined();
  });
});

describe('postCode', () => {
  it('knows the digits of a few countries, takes anything elsewhere', () => {
    expect(postCode('DE')('50667')).toBeUndefined();
    expect(postCode('DE')('5066')).toBe('Eine Postleitzahl in DE hat 5 Ziffern.');
    expect(postCode('AT')('1010')).toBeUndefined();
    expect(postCode('NL')('1012 AB')).toBeUndefined();
  });
});

describe('country', () => {
  it('takes a code there is a country for', () => {
    expect(country('DE')).toBeUndefined();
    expect(countryName('AT')).toBe('Österreich');
    expect(country('XQ')).toBe('Ein Land mit dem Code XQ gibt es nicht.');
    // ICU still names withdrawn codes
    expect(country('DD')).toBe('Ein Land mit dem Code DD gibt es nicht.');
    expect(country('UK')).toBe('Ein Land mit dem Code UK gibt es nicht.');
    expect(country('D')).toMatch(/Zwei Buchstaben/);
  });
});
