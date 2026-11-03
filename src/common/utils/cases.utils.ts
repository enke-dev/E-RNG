import type { CaseStage, Kind } from './api.utils.js';

/** Where a Vorgang stands, as the UI says it */
export const CASE_STAGES: Record<CaseStage, string> = {
  draft: 'Entwurf',
  offered: 'Angeboten',
  accepted: 'Angenommen',
  declined: 'Abgelehnt',
  invoiced: 'Berechnet',
  'partly-paid': 'Teilweise bezahlt',
  paid: 'Bezahlt',
};

/** The tag colour of a stage */
export const STAGE_VARIANTS: Record<CaseStage, string> = {
  draft: 'neutral',
  offered: 'brand',
  accepted: 'success',
  declined: 'danger',
  invoiced: 'brand',
  'partly-paid': 'warning',
  paid: 'success',
};

/** What a Vorgang is called: its customer and, last, its internal title if it has one */
/**
 * The address of the Vorgänge, or of a document in them: `new` becomes `new-offer` or
 * `new-invoice`, the kind is all a new one has; `new-offer@12` makes it in Vorgang 12. A document
 * of Vorgang 12 is `case-12/7`; without its Vorgang, `7` finds it.
 */
export function casesRoute(kind: Kind, id?: number | string, inCase?: number): string {
  if (id === undefined) {
    return '/cases';
  }
  if (id === 'new') {
    return `/cases/new-${kind}${inCase === undefined ? '' : `@${inCase}`}`;
  }
  return inCase === undefined || id === 'none' ? `/cases/${id}` : `/cases/case-${inCase}/${id}`;
}

/** The address of a Vorgang itself, its overview */
export const caseRoute = (id: number) => `/cases/case-${id}`;

/** What the address of the Vorgänge selects: a document, a new one (in a Vorgang), a Vorgang, nothing */
export type Selection =
  | { type: 'newest' }
  | { type: 'none' }
  | { type: 'document'; id: number; caseId: number | null }
  | { type: 'new'; kind: Kind; caseId: number | null }
  | { type: 'case'; id: number };

export function parseSelection(selected: string): Selection {
  if (selected === '') {
    return { type: 'newest' };
  }
  const fresh = /^new-(offer|invoice)(?:@(\d+))?$/.exec(selected);
  if (fresh) {
    return { type: 'new', kind: fresh[1] as Kind, caseId: fresh[2] ? Number(fresh[2]) : null };
  }
  const vorgang = /^case-(\d+)(?:\/(\d+))?$/.exec(selected);
  if (vorgang) {
    return vorgang[2]
      ? { type: 'document', id: Number(vorgang[2]), caseId: Number(vorgang[1]) }
      : { type: 'case', id: Number(vorgang[1]) };
  }
  const id = Number(selected);
  return Number.isInteger(id) && id > 0 ? { type: 'document', id, caseId: null } : { type: 'none' };
}
