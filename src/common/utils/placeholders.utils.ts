/** A placeholder a text can use: its name (written `{name}`), what it stands for, how it may look */
export interface Placeholder {
  name: string;
  label: string;
  /** Several lines are joined by ` ↵ ` */
  example: string;
}

/** Placeholders that belong together, under a heading */
export interface PlaceholderGroup {
  heading: string;
  placeholders: Placeholder[];
}

const VERSIONS: Placeholder[] = [
  { name: 'version', label: 'Version: wie oft der Entwurf exportiert wurde', example: '2' },
  { name: 'vorgang_number', label: 'Nummer des Vorgangs', example: 'VG-2026-012' },
];

const DOCUMENT: PlaceholderGroup = {
  heading: 'Dokument',
  placeholders: [
    { name: 'number', label: 'Nummer, im Entwurf ihr Muster', example: 'RE-2026-001' },
    { name: 'date', label: 'Datum', example: '31.10.2026' },
    { name: 'due_date', label: 'Fällig am (Rechnungen)', example: '14.11.2026' },
    { name: 'customer_name', label: 'Name des Kunden', example: 'Beispiel AG' },
    {
      name: 'offer_number',
      label: 'Angebot: das eine Rechnung berechnet, auf das ein Nachtrag sich bezieht',
      example: 'AN-2026-003',
    },
    ...VERSIONS,
  ],
};

/**
 * What the text parts of a layout and the letter texts (intro and closing) can use
 * (`render/parts.rs::placeholder`)
 */
export const PART_PLACEHOLDERS: PlaceholderGroup[] = [
  DOCUMENT,
  {
    heading: 'Unternehmen',
    placeholders: [
      { name: 'seller_name', label: 'Name', example: 'Muster GmbH' },
      { name: 'seller_street', label: 'Straße', example: 'Hauptstraße 5' },
      { name: 'seller_postcode_city', label: 'PLZ und Ort', example: '50667 Köln' },
      { name: 'seller_contact_name', label: 'Ansprechpartner', example: 'Max Muster' },
      { name: 'seller_phone', label: 'Telefon', example: '0221 123456' },
      { name: 'seller_mobile', label: 'Mobil', example: '0171 1234567' },
      { name: 'seller_fax', label: 'Fax', example: '0221 123457' },
      { name: 'seller_email', label: 'E-Mail', example: 'info@muster.de' },
      { name: 'seller_website', label: 'Website', example: 'www.muster.de' },
      { name: 'seller_vat_id', label: 'USt-IdNr.', example: 'DE123456789' },
      { name: 'seller_tax_number', label: 'Steuernummer', example: '21/815/08150' },
    ],
  },
  {
    heading: 'Bankverbindung',
    placeholders: [
      { name: 'bank_name', label: 'Name der Bank', example: 'Sparkasse Köln' },
      { name: 'account_holder', label: 'Kontoinhaber', example: 'Muster GmbH' },
      { name: 'iban', label: 'IBAN', example: 'DE02 1203 0000 0000 2020 51' },
      { name: 'bic', label: 'BIC', example: 'BYLADEM1001' },
    ],
  },
  {
    heading: 'Mehrere Zeilen',
    placeholders: [
      {
        name: 'seller_address',
        label: 'Anschrift',
        example: 'Muster GmbH ↵ Hauptstraße 5 ↵ 50667 Köln',
      },
      {
        name: 'seller_contact',
        label: 'Ansprechpartner, Telefon, E-Mail',
        example: 'Max Muster ↵ Telefon 0221 123456 ↵ info@muster.de',
      },
      {
        name: 'seller_registration',
        label: 'USt-IdNr. und Steuernummer',
        example: 'USt-IdNr. DE123456789 ↵ St.-Nr. 21/815/08150',
      },
      {
        name: 'legal',
        label: 'Rechtliche Angaben',
        example: 'Geschäftsführer: Max Muster ↵ Amtsgericht Köln HRB 12345',
      },
      {
        name: 'bank',
        label: 'Bankverbindung',
        example: 'Kontoinhaber: Muster GmbH ↵ IBAN: DE02 1203 … ↵ BIC: BYLADEM1001',
      },
    ],
  },
  {
    heading: 'Angebote',
    placeholders: [{ name: 'valid_until', label: 'Gültig bis', example: '30. November 2026' }],
  },
];

/** The letter texts take what the text parts take */
export const LETTER_PLACEHOLDERS = PART_PLACEHOLDERS;

/** What the headline patterns of a layout can use (`render/view.rs::headline`) */
export const HEADLINE_PLACEHOLDERS: PlaceholderGroup[] = [
  {
    heading: 'Bezeichnung',
    placeholders: [
      { name: 'title', label: 'Überschrift der Dokumentart', example: 'Rechnung' },
      { name: 'draft_title', label: 'Überschrift eines Entwurfs', example: 'Rechnungsentwurf' },
      { name: 'number', label: 'Nummer des Dokuments', example: 'RE-2026-001' },
    ],
  },
  { heading: 'Dokument', placeholders: VERSIONS },
];

/** What the page number part can use */
export const PAGE_PLACEHOLDERS: PlaceholderGroup[] = [
  {
    heading: 'Seiten',
    placeholders: [
      { name: 'page', label: 'Diese Seite', example: '1' },
      { name: 'pages', label: 'Zahl der Seiten', example: '3' },
    ],
  },
];

/** The groups with what matches a search in name, meaning or example; empty groups are left out */
export function searchPlaceholders(groups: PlaceholderGroup[], query: string): PlaceholderGroup[] {
  const words = query.trim().toLowerCase();
  if (words === '') {
    return groups;
  }
  return groups
    .map(group => ({
      ...group,
      placeholders: group.placeholders.filter(p =>
        [p.name, p.label, p.example].some(text => text.toLowerCase().includes(words))
      ),
    }))
    .filter(group => group.placeholders.length > 0);
}
