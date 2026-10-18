/**
 * Drag and drop reordering of rows with pointer events, so it works the same in every webview
 * (the HTML5 drag and drop API is unreliable inside them).
 *
 * Call it from `pointerdown` on a drag handle. The dragged row follows the pointer, the other rows
 * slide apart to open the gap where it would land, and a dashed `.drop-slot` marks that gap (the
 * rows' container needs `position: relative`, the styles of both live with the caller). On release
 * `done` gets the new index.
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
  const centers = boxes.map(box => box.top + box.height / 2);
  const draggedHeight = boxes[from]?.height ?? 0;
  const containerTop = container.getBoundingClientRect().top;
  let to = from;

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
    const center = (centers[from] ?? 0) + offset;
    // the new index is the number of the other rows whose middle lies above the dragged one
    const next = centers.filter((c, i) => i !== from && c < center).length;
    if (next !== to) {
      to = next;
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
