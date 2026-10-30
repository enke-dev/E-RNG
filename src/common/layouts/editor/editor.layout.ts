import '@awesome.me/webawesome/dist/components/split-panel/split-panel.js';

import type WaSplitPanel from '@awesome.me/webawesome/dist/components/split-panel/split-panel.js';
import { html, svg } from 'lit';
import { customElement, property, query } from 'lit/decorators.js';

import { ErngElement } from '../../utils/base.utils.js';
import { recall, remember } from '../../utils/memory.utils.js';
import styles from './editor.layout.css';

/** A line from something in the form to the same thing in the preview: pointed at or selected */
export interface Link {
  key: string;
  kind: 'hover' | 'active';
}

/** What a slotted element offers to be linked: the element that shows a key, if it shows it now */
export interface Anchoring {
  anchor(key: string): Element | null;
}

interface Box {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

/** The next box up in the composed tree: slots and shadow hosts count, as they do for layout */
const parentOf = (node: Element): Element | null =>
  node.assignedSlot ??
  node.parentElement ??
  (node.getRootNode() as Partial<ShadowRoot>).host ??
  null;

const ancestors = (node: Element): Element[] => {
  const parent = parentOf(node);
  return parent ? [parent, ...ancestors(parent)] : [];
};

/** A pane that cuts what is in it; slots and the like have no box, whatever their overflow says */
const clips = (node: Element) => {
  const { display, overflowX, overflowY } = getComputedStyle(node);
  return display !== 'contents' && (overflowX !== 'visible' || overflowY !== 'visible');
};

/** An element with the ring of an outline around it, and what of the view the panes around it leave */
interface Seen {
  own: Box;
  view: Box;
}

/**
 * Where an element is and what of it could be seen; none when it is hidden. The panes around it that
 * clip are collected, their scrolling moves the line.
 */
const seen = (element: Element, panes: Set<EventTarget>): Seen | null => {
  if (!element.checkVisibility({ visibilityProperty: true })) {
    return null;
  }
  const style = getComputedStyle(element);
  const ring =
    style.outlineStyle === 'none'
      ? 0
      : parseFloat(style.outlineOffset) + parseFloat(style.outlineWidth);
  const own = element.getBoundingClientRect();
  const view = ancestors(element)
    .filter(clips)
    .reduce<Box>(
      (cut, pane) => {
        panes.add(pane);
        const r = pane.getBoundingClientRect();
        return {
          top: Math.max(cut.top, r.top),
          right: Math.min(cut.right, r.right),
          bottom: Math.min(cut.bottom, r.bottom),
          left: Math.max(cut.left, r.left),
        };
      },
      { top: -Infinity, right: Infinity, bottom: Infinity, left: -Infinity }
    );
  return {
    own: {
      top: own.top - ring,
      right: own.right + ring,
      bottom: own.bottom + ring,
      left: own.left - ring,
    },
    view,
  };
};

/**
 * The middle of what is seen of a span; when nothing is, its end nearest to the view, so the curve
 * leaves the panes there and moves with the scroll
 */
const middle = (start: number, end: number, from: number, to: number) =>
  end < from ? end : start > to ? start : (Math.max(start, from) + Math.min(end, to)) / 2;

/**
 * A form with the document it edits beside it, the shape of the invoice and the layout editors.
 * The divider between them is the user's to move, its position (percent) is remembered under
 * its `position-key`; the preview scales with its panel. Where the two minimums do not fit, only the
 * form is shown.
 *
 * Links are drawn as curves from the form to the preview, between what both sides show for a key
 * (s. `Anchoring`). The curves are in client coordinates, so every scroll of a pane around either end,
 * a resize, the divider or something opening in the form draws them again, once per frame, and on
 * every frame while it opens or closes.
 *
 * @slot form - the fields
 * @slot preview - an `erng-sheet`
 */
@customElement('erng-editor')
export class EditorLayout extends ErngElement.withStyles(styles) {
  /** The name the position of the divider is stored under, one per view; none: not stored */
  @property({ attribute: 'position-key' }) positionKey = '';

  /** What to connect, the selected one first so the pointed at one is drawn above it */
  @property({ attribute: false }) links: Link[] = [];

  @query('svg') private canvas!: SVGSVGElement;

  #frame = 0;
  #panes = new Set<EventTarget>();
  #moving = new Set<Element>();

  readonly #schedule = () => {
    this.#frame ||= requestAnimationFrame(() => this.draw());
  };

  /** Something opening or closing animates in its shadow root: the curves follow it frame by frame */
  readonly #animating = (e: Event) => {
    this.#moving.add(e.composedPath()[0] as Element);
    this.#schedule();
  };

  override connectedCallback() {
    super.connectedCallback();
    // what moves or resizes parts of the form without scrolling
    ['wa-after-show', 'wa-after-hide', 'wa-tab-show', 'input'].forEach(name =>
      this.addEventListener(name, this.#schedule, { passive: true })
    );
    ['wa-show', 'wa-hide'].forEach(name =>
      this.addEventListener(name, this.#animating, { passive: true })
    );
  }

  override disconnectedCallback() {
    super.disconnectedCallback();
    cancelAnimationFrame(this.#frame);
    this.#frame = 0;
    this.#moving.clear();
    this.follow(new Set());
  }

  protected override updated() {
    this.#schedule();
  }

  /** The element a slot shows for a key, asked from the slotted elements that know */
  private anchor(slot: 'form' | 'preview', key: string): Element | null {
    const slotted =
      this.renderRoot.querySelector<HTMLSlotElement>(`slot[name="${slot}"]`)?.assignedElements() ??
      [];
    return (
      slotted.map(element => (element as Partial<Anchoring>).anchor?.(key) ?? null).find(Boolean) ??
      null
    );
  }

  /** Listens to the panes the ends are in, and only to those */
  private follow(panes: Set<EventTarget>) {
    [...this.#panes]
      .filter(pane => !panes.has(pane))
      .forEach(pane => {
        pane.removeEventListener('scroll', this.#schedule);
        pane.removeEventListener('resize', this.#schedule);
      });
    [...panes]
      .filter(pane => !this.#panes.has(pane))
      .forEach(pane => {
        pane.addEventListener('scroll', this.#schedule, { passive: true });
        pane.addEventListener('resize', this.#schedule, { passive: true });
      });
    this.#panes = panes;
  }

  /** Measures both ends of every link first, then draws: no layout in between */
  private draw() {
    this.#frame = 0;
    const panes = new Set<EventTarget>(this.links.length ? [window] : []);
    const origin = this.canvas.getBoundingClientRect();
    const curves = this.links.map(({ key }) => {
      const from = this.anchor('form', key);
      const to = this.anchor('preview', key);
      const start = from && seen(from, panes);
      const end = to && seen(to, panes);
      if (!start || !end) {
        return '';
      }
      // from the middle of what is seen of either end, leaving and arriving level
      const x1 = Math.min(start.own.right, start.view.right) - origin.left;
      const y1 =
        middle(start.own.top, start.own.bottom, start.view.top, start.view.bottom) - origin.top;
      const x2 = Math.max(end.own.left, end.view.left) - origin.left;
      const y2 = middle(end.own.top, end.own.bottom, end.view.top, end.view.bottom) - origin.top;
      const bend = (x2 - x1) / 2;
      return `M ${x1} ${y1} C ${x1 + bend} ${y1}, ${x2 - bend} ${y2}, ${x2} ${y2}`;
    });
    this.canvas
      .querySelectorAll('path')
      .forEach((path, index) => path.setAttribute('d', curves[index] ?? ''));
    this.follow(panes);
    // again next frame while anything still animates, an interrupted one ends without `wa-after-*`
    this.#moving = new Set(
      [...this.#moving].filter(element => element.shadowRoot?.getAnimations().length)
    );
    if (this.#moving.size) {
      this.#schedule();
    }
  }

  override render() {
    return html`
      <svg aria-hidden="true">
        ${this.links.map(
          ({ kind }) => svg`
            <path class="${kind}"></path>
          `
        )}
      </svg>
      <wa-split-panel
        position="${(this.positionKey ? recall(this.positionKey) : undefined) ?? 60}"
        @wa-reposition="${(e: Event) => {
          this.#schedule();
          if (this.positionKey) {
            remember(this.positionKey, (e.currentTarget as WaSplitPanel).position);
          }
        }}"
      >
        <slot slot="start" name="form"></slot>
        <slot slot="end" name="preview"></slot>
      </wa-split-panel>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'erng-editor': EditorLayout;
  }
}
