import type { ReactiveControllerHost, TemplateResult } from 'lit';
import { html } from 'lit';

/**
 * Drag and drop reordering of rows with pointer events, so it works the same in every webview
 * (the HTML5 drag and drop API is unreliable inside them).
 *
 * Call it from `pointerdown` on a drag handle. The dragged row follows the pointer, the other rows
 * slide apart to open the gap where it would land, and a dashed `.drop-slot` marks that gap (the
 * rows' container needs `position: relative`, the styles are `reorder.utils.css`). The slot is
 * the one nearest to the dragged row, with a dead band so a swap needs a deliberate move back, too.
 * On release `done` gets the new index.
 */
export function startReorder(
  event: PointerEvent,
  rows: HTMLElement[],
  from: number,
  done: (to: number) => void
) {
  const handle = event.currentTarget as HTMLElement;
  const dragged = rows[from];
  const container = dragged?.parentElement;
  if (!dragged || !container || event.button !== 0) {
    return;
  }
  event.preventDefault();
  handle.setPointerCapture(event.pointerId);

  const startY = event.clientY;
  const boxes = rows.map(row => row.getBoundingClientRect());
  const tops = boxes.map(box => box.top);
  const draggedHeight = boxes[from]?.height ?? 0;
  const containerTop = container.getBoundingClientRect().top;
  let to = from;

  /** Where the top of the dragged row would be if it landed on index `k`, rows of any height */
  const slotTop = (k: number) =>
    k <= from ? (tops[k] ?? 0) : (tops[k] ?? 0) + (boxes[k]?.height ?? 0) - draggedHeight;
  const slotCenter = (k: number) => slotTop(k) + draggedHeight / 2;
  /**
   * A swap only counts when the dragged row is past the middle between two slots by this much, in
   * either direction. Without it a swap is undone by the slightest move back across the same line,
   * which feels like one direction needs a real move and the other none.
   */
  const margin = Math.min(12, draggedHeight * 0.2);

  const slot = document.createElement('div');
  slot.className = 'drop-slot';
  slot.style.height = `${draggedHeight}px`;
  container.append(slot);

  dragged.classList.add('dragging');
  rows.forEach((row, i) => {
    if (i !== from) {
      row.style.transition = 'transform 120ms ease';
    }
  });

  /** The other rows move into the places the dragged one leaves, the slot sits where it lands */
  const arrange = () => {
    rows.forEach((row, i) => {
      if (i === from) {
        return;
      }
      let shift = 0;
      if (to > from && i > from && i <= to) {
        shift = (tops[i - 1] ?? 0) - (tops[i] ?? 0);
      } else if (to < from && i >= to && i < from) {
        shift = (tops[i + 1] ?? 0) - (tops[i] ?? 0);
      }
      row.style.transform = shift === 0 ? '' : `translateY(${shift}px)`;
    });
    const landing =
      to > from ? (tops[to] ?? 0) + (boxes[to]?.height ?? 0) - draggedHeight : (tops[to] ?? 0);
    slot.style.top = `${landing - containerTop}px`;
  };
  arrange();

  const move = (e: PointerEvent) => {
    const offset = e.clientY - startY;
    dragged.style.transform = `translateY(${offset}px)`;
    const center = slotCenter(from) + offset;
    const before = to;
    // one slot at a time, each past the middle between two slots plus the margin
    while (to < rows.length - 1 && center > (slotCenter(to) + slotCenter(to + 1)) / 2 + margin) {
      to++;
    }
    while (to > 0 && center < (slotCenter(to - 1) + slotCenter(to)) / 2 - margin) {
      to--;
    }
    if (to !== before) {
      arrange();
    }
  };

  const finish = (e: PointerEvent, commit: boolean) => {
    handle.removeEventListener('pointermove', move);
    handle.removeEventListener('pointerup', up);
    handle.removeEventListener('pointercancel', cancel);
    if (handle.hasPointerCapture(e.pointerId)) {
      handle.releasePointerCapture(e.pointerId);
    }
    slot.remove();
    dragged.classList.remove('dragging');
    rows.forEach(row => {
      row.style.transform = '';
      row.style.transition = '';
    });
    if (commit && to !== from) {
      done(to);
    }
  };
  const up = (e: PointerEvent) => finish(e, true);
  const cancel = (e: PointerEvent) => finish(e, false);

  handle.addEventListener('pointermove', move);
  handle.addEventListener('pointerup', up);
  handle.addEventListener('pointercancel', cancel);
}

/** Where the row that was at `index` is after the row at `from` was moved to `to` */
export function indexAfterMove(index: number, from: number, to: number) {
  if (index === from) {
    return to;
  }
  if (from < to && index > from && index <= to) {
    return index - 1;
  }
  if (from > to && index >= to && index < from) {
    return index + 1;
  }
  return index;
}

/** What a host tells `Reorder` about its rows */
export interface ReorderOptions {
  /** Selector of the rows in the host's shadow root, in order */
  rows: string;
  /** What is moved, as the handle's title says it: "Position", "Einheit" */
  noun: string;
  /** How many rows there are, a single one cannot move */
  count: () => number;
  /** Moves the row; the handle keeps its focus on the row's new place once this is done */
  move: (from: number, to: number) => void | Promise<void>;
}

/**
 * The handle of a reorderable row, for the pointer (`startReorder`) and the keyboard (arrow keys,
 * and the focus follows the row). The host renders `handle(index)` as the first child of each row
 * and adds `reorder.utils.css` to its styles. It needs no lifecycle, so it is no Lit controller.
 */
export class Reorder {
  constructor(
    private readonly host: ReactiveControllerHost & { shadowRoot: ShadowRoot | null },
    private readonly options: ReorderOptions
  ) {}

  private query<T extends HTMLElement>(selector: string) {
    return [...(this.host.shadowRoot?.querySelectorAll<T>(selector) ?? [])];
  }

  private start(event: PointerEvent, index: number) {
    startReorder(
      event,
      this.query(this.options.rows),
      index,
      to => void this.options.move(index, to)
    );
  }

  private async key(event: KeyboardEvent, index: number) {
    const to = event.key === 'ArrowUp' ? index - 1 : event.key === 'ArrowDown' ? index + 1 : index;
    if (to === index || to < 0 || to >= this.options.count()) {
      return;
    }
    event.preventDefault();
    await this.options.move(index, to);
    await this.host.updateComplete;
    this.query('.handle')[to]?.focus();
  }

  /** The button to drag or to press the arrow keys on; `locked` disables it for a row that cannot move */
  handle(index: number, locked = false): TemplateResult {
    return html`
      <button
        class="handle"
        title="${this.options.noun} verschieben (ziehen oder mit den Pfeiltasten)"
        ?disabled="${locked || this.options.count() === 1}"
        @pointerdown="${(e: PointerEvent) => this.start(e, index)}"
        @keydown="${(e: KeyboardEvent) => void this.key(e, index)}"
      >
        ⠿
      </button>
    `;
  }
}
