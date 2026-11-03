import '@awesome.me/webawesome/dist/components/badge/badge.js';
import '@awesome.me/webawesome/dist/components/icon/icon.js';
import '@awesome.me/webawesome/dist/components/tag/tag.js';

import { html, nothing, svg } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

import type { CaseGraph, EdgeKind, GraphEdge, ListEntry } from '../../utils/api.utils.js';
import { ErngElement } from '../../utils/base.utils.js';
import { caseRoute, casesRoute } from '../../utils/cases.utils.js';
import documentStyles from '../../utils/documents.utils.css';
import { marksOf, tagsOf } from '../../utils/documents.utils.js';
import { formatDate, money } from '../../utils/format.utils.js';
import type { Placed } from '../../utils/graph.utils.js';
import { layoutGraph } from '../../utils/graph.utils.js';
import { KINDS } from '../../utils/kind.utils.js';
import { goto } from '../../utils/routing.utils.js';
import styles from './case-graph.component.css';

/** The width of a document's card and the room around, in pixels; a card is as tall as it needs */
const WIDTH = 288;
const GAP_Y = 12;
/** Between the columns, room for the lines crossing over */
const GAP_X = 112;
/** How far a branch of a tree moves in, at most `DEEPEST` steps */
const INDENT = 20;
const DEEPEST = 3;
/** Right of a column, room for the lines within it (an Abschlag deducted), each one further out */
const ARC = 18;
const ARC_STEP = 12;

/** What a line says, in the legend and on hovering it */
const EDGES: Record<EdgeKind, string> = {
  bills: 'berechnet',
  share: 'Anteil eines Angebots',
  deducts: 'abgezogen',
  supplement: 'Nachtrag',
  alternative: 'Alternative',
  replacement: 'Ersatzangebot',
  cancels: 'Storno',
  replaces: 'Ersatzrechnung',
};

/** Where a card was drawn, in the canvas */
interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** What a document is called on its card: its kind and number, or the kind's draft */
const titleOf = (entry: ListEntry) =>
  entry.number ? `${KINDS[entry.kind].noun} ${entry.number}` : KINDS[entry.kind].draftTitle;

/**
 * The documents of a Vorgang as a graph (see `layoutGraph`): offers left, invoices right, each a
 * tree by date, a card per document with its status, opening it. Branches join a Nachtrag or a
 * Storno to what it belongs to, lines run from what is billed to what bills it and from an
 * Abschlag to what deducts it, styled by kind (`EDGES`, explained below the graph). Hovering a
 * card lights up what it relates to. Documents of other Vorgänge are faint and open their
 * Vorgang. On a narrow card the documents are a plain list by date.
 *
 * The cards flow in their columns, as tall as their tags wrap; the lines are drawn to where they
 * ended up, measured after each rendering and whenever the canvas changes its size.
 */
@customElement('erng-case-graph')
export class CaseGraphComponent extends ErngElement.withStyles(documentStyles, styles) {
  @property({ attribute: false }) graph: CaseGraph | undefined;
  @property({ type: Number, attribute: 'case-id' }) caseId!: number;

  /** The document hovered or focused, what it relates to lights up */
  @state() private lit: number | undefined;
  /** Where the cards are, by document */
  @state() private boxes = new Map<number, Box>();

  private readonly resized = new ResizeObserver(() => this.measure());
  private observed: Element | undefined;

  override disconnectedCallback() {
    super.disconnectedCallback();
    this.resized.disconnect();
    this.observed = undefined;
  }

  protected override updated() {
    const canvas = this.renderRoot.querySelector('.canvas') ?? undefined;
    if (canvas !== this.observed) {
      this.resized.disconnect();
      if (canvas) {
        this.resized.observe(canvas);
      }
      this.observed = canvas;
    }
    this.measure();
  }

  /** Reads where the cards are; only a change renders again */
  private measure() {
    const cards = [...this.renderRoot.querySelectorAll<HTMLElement>('.canvas .card[data-id]')];
    const boxes = new Map(
      cards.map(card => [
        Number(card.dataset['id']),
        {
          left: card.offsetLeft,
          top: card.offsetTop,
          width: card.offsetWidth,
          height: card.offsetHeight,
        },
      ])
    );
    const same =
      boxes.size === this.boxes.size &&
      [...boxes].every(([id, box]) => {
        const was = this.boxes.get(id);
        return (
          was?.left === box.left &&
          was.top === box.top &&
          was.width === box.width &&
          was.height === box.height
        );
      });
    if (!same) {
      this.boxes = boxes;
    }
  }

  private open(entry: ListEntry) {
    goto(
      entry.case_id === this.caseId
        ? casesRoute(entry.kind, entry.id, this.caseId)
        : caseRoute(entry.case_id)
    );
  }

  /** Where a document of another Vorgang is, by that Vorgang's number */
  private elsewhere(entry: ListEntry) {
    const number = this.graph?.others.find(o => o.id === entry.case_id)?.number;
    return entry.case_id === this.caseId ? nothing : html`<span class="muted">in ${number}</span>`;
  }

  private renderCard(entry: ListEntry, style = '', dim = false) {
    const other = entry.case_id !== this.caseId;
    const light = (on: boolean) => () => (this.lit = on ? entry.id : undefined);
    const tags = tagsOf(entry, false, false);
    return html`
      <button
        class="card ${other ? 'other' : ''} ${dim ? 'dim' : ''}"
        data-id="${entry.id}"
        style="${style}"
        title="${other ? 'Zum anderen Vorgang' : 'Öffnen'}"
        @click="${() => this.open(entry)}"
        @mouseenter="${light(true)}"
        @mouseleave="${light(false)}"
        @focus="${light(true)}"
        @blur="${light(false)}"
      >
        <span class="title">
          <strong>${titleOf(entry)}</strong>
          ${marksOf(entry)}
        </span>
        <span class="muted">
          ${formatDate(entry.issue_date)} · ${money(entry.total)} ${this.elsewhere(entry)}
        </span>
        ${tags.length > 0 ? html`<span class="tags">${tags}</span>` : nothing}
      </button>
    `;
  }

  private renderGraph(graph: CaseGraph) {
    const { placed, branches, lines } = layoutGraph(graph.documents, graph.edges, this.caseId);
    const entries = new Map(graph.documents.map(d => [d.id, d]));
    const found = new Map(placed.map(p => [p.id, p]));
    // every document is placed, an edge names placed ones only
    const at = (id: number): Placed => found.get(id) ?? { id, column: 0, row: 0, depth: 0 };
    const indent = (depth: number) => Math.min(depth, DEEPEST) * INDENT;
    const columns = ([0, 1] as const)
      .map(column => ({
        column,
        label: column === 0 ? 'Angebote' : 'Rechnungen',
        placed: placed.filter(p => p.column === column).sort((a, b) => a.row - b.row),
      }))
      .filter(c => c.placed.length > 0);

    // until the cards are measured there is nothing to draw to
    const boxes = this.boxes;
    const drawable = (e: GraphEdge) => boxes.has(e.from) && boxes.has(e.to);
    const box = (id: number): Box => boxes.get(id) ?? { left: 0, top: 0, width: 0, height: 0 };
    const mid = (id: number) => box(id).top + box(id).height / 2;
    const right = (id: number) => box(id).left + box(id).width;
    const edge = (column: 0 | 1) =>
      Math.max(
        0,
        ...placed.filter(p => p.column === column && boxes.has(p.id)).map(p => right(p.id))
      );

    // lines within a column bend out on its right, the longer ones further out
    const within = lines
      .filter(e => at(e.from).column === at(e.to).column)
      .sort(
        (a, b) => Math.abs(at(a.to).row - at(a.from).row) - Math.abs(at(b.to).row - at(b.from).row)
      );
    const reach = (e: GraphEdge) => ARC + ARC_STEP * within.indexOf(e);
    const path = (e: GraphEdge) => {
      const [from, to] = [at(e.from), at(e.to)];
      if (from.column === to.column) {
        // out to the right of the column and back, a cubic bulges 3/4 of its control points
        const o = edge(from.column) + (reach(e) * 4) / 3;
        return `M ${right(e.from)} ${mid(e.from)} C ${o} ${mid(e.from)}, ${o} ${mid(e.to)}, ${right(e.to)} ${mid(e.to)}`;
      }
      const [x1, x2] =
        from.column < to.column ? [right(e.from), box(e.to).left] : [box(e.from).left, right(e.to)];
      const bend = (x2 - x1) / 2;
      return `M ${x1} ${mid(e.from)} C ${x1 + bend} ${mid(e.from)}, ${x2 - bend} ${mid(e.to)}, ${x2} ${mid(e.to)}`;
    };
    // a branch drops from below its parent and turns into the child's left edge
    const branch = (e: GraphEdge) => {
      const trunk = box(e.from).left + INDENT / 2;
      const [top, bottom, r] = [box(e.from).top + box(e.from).height, mid(e.to), 6];
      return `M ${trunk} ${top} V ${bottom - r} Q ${trunk} ${bottom}, ${trunk + r} ${bottom} H ${box(e.to).left}`;
    };

    const lit = this.lit;
    const touches = (e: GraphEdge) => lit !== undefined && (e.from === lit || e.to === lit);
    const related = new Set([...branches, ...lines].filter(touches).flatMap(e => [e.from, e.to]));
    const state = (e: GraphEdge) => (lit === undefined ? '' : touches(e) ? 'lit' : 'dim');
    const drawn = [
      ...branches.filter(drawable).map(e => ({ e, d: branch(e) })),
      ...lines.filter(drawable).map(e => ({ e, d: path(e) })),
    ]
      // the lit ones on top
      .sort((a, b) => Number(touches(a.e)) - Number(touches(b.e)))
      .map(
        ({ e, d }) => svg`
          <path class="${e.kind} ${state(e)}" d="${d}">
            <title>${EDGES[e.kind]}</title>
          </path>
        `
      );

    const outmost = within.at(-1);
    const used = (Object.keys(EDGES) as EdgeKind[]).filter(kind =>
      [...branches, ...lines].some(e => e.kind === kind)
    );
    const dim = (id: number) => lit !== undefined && lit !== id && !related.has(id);
    return html`
      <div class="scroll">
        <div
          class="canvas"
          style="column-gap: ${GAP_X}px; padding-inline-end: ${outmost ? reach(outmost) : 0}px"
        >
          <svg aria-hidden="true">${drawn}</svg>
          ${columns.map(
            ({ label, placed: cards }) => html`
              <div class="column" style="row-gap: ${GAP_Y}px">
                <span class="head">${label}</span>
                ${cards.map(p => {
                  const entry = entries.get(p.id);
                  return entry
                    ? this.renderCard(
                        entry,
                        `width: ${WIDTH}px; margin-inline-start: ${indent(p.depth)}px`,
                        dim(p.id)
                      )
                    : nothing;
                })}
              </div>
            `
          )}
        </div>
      </div>
      ${
        used.length > 0
          ? html`
              <ul class="legend">
                ${used.map(
                  kind => html`
                    <li>
                      <svg width="28" height="8" aria-hidden="true">
                        <path class="${kind}" d="M 0 4 L 28 4"></path>
                      </svg>
                      ${EDGES[kind]}
                    </li>
                  `
                )}
              </ul>
            `
          : nothing
      }
    `;
  }

  override render() {
    const graph = this.graph;
    if (!graph) {
      return nothing;
    }
    if (graph.documents.length === 0) {
      return html`<p class="muted">Noch keine Dokumente.</p>`;
    }
    return html`
      <div class="graph">${this.renderGraph(graph)}</div>
      <div class="list">${graph.documents.map(entry => this.renderCard(entry))}</div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-case-graph': CaseGraphComponent;
  }
}
