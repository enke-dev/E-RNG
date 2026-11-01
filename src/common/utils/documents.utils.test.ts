import { describe, expect, it, vi } from 'vitest';

import type { Standing } from './documents.utils.js';
import { statusOf } from './documents.utils.js';

// the contexts come with the app's router, the status needs none of them
vi.mock('./context.utils.js', () => ({ contextById: () => undefined, label: () => '' }));

const invoice: Standing = {
  kind: 'invoice',
  status: 'exported',
  archived_at: null,
  on_account: null,
  closing: false,
  partial: false,
  corrects: null,
  valid_until: null,
  paid_at: null,
  paid: null,
  answer: null,
  answered_on: null,
  relation: null,
  related_id: null,
  related_number: null,
  lapsed: false,
  superseded: false,
  cancelled: false,
};

describe('where a document stands', () => {
  it('names a Storno in preparation and opens its draft', () => {
    expect(statusOf({ ...invoice, cancel_draft: 12 })).toEqual([
      expect.objectContaining({
        label: 'Storno in Vorbereitung',
        to: { kind: 'invoice', id: 12 },
      }),
    ]);
  });

  it('says in a row what, on the document which', () => {
    const [cancelled] = statusOf({
      ...invoice,
      cancelled: true,
      cancelled_by: { id: 9, number: 'RE-2026-008' },
    });
    expect(cancelled).toMatchObject({
      label: 'Storniert',
      detail: 'Storniert durch RE-2026-008',
      to: { kind: 'invoice', id: 9 },
    });
    const [storno] = statusOf({
      ...invoice,
      corrects: { invoice_id: 3, kind: 'cancel', number: 'RE-2026-007', issue_date: '2026-10-01' },
    });
    expect(storno).toMatchObject({ label: 'Storno', detail: 'Storno zu RE-2026-007' });
  });

  it('shows paid as a check mark in a row', () => {
    expect(statusOf({ ...invoice, paid: '10', paid_at: '2026-11-01' })).toEqual([
      expect.objectContaining({ label: 'Bezahlt', mark: true }),
    ]);
  });
});
