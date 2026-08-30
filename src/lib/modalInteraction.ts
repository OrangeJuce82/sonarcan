type PointerCoordinates = Pick<MouseEvent, "clientX" | "clientY">;
type ModalBounds = EventTarget & {
  getBoundingClientRect: () => Pick<DOMRect, "left" | "right" | "top" | "bottom">;
};

export function isPointOutsideModal(
  event: PointerCoordinates,
  dialog: ModalBounds,
): boolean {
  const bounds = dialog.getBoundingClientRect();
  return event.clientX < bounds.left
    || event.clientX > bounds.right
    || event.clientY < bounds.top
    || event.clientY > bounds.bottom;
}

export function shouldDismissModalFromBackdrop(
  event: PointerCoordinates & Pick<MouseEvent, "target">,
  dialog: ModalBounds,
): boolean {
  return event.target === dialog && isPointOutsideModal(event, dialog);
}
