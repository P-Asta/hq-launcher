import { useEffect } from "react";

/**
 * Keeps a positioned context menu inside the viewport and closes it on
 * outside-click, Escape, resize or scroll.
 *
 * @param menu       state object shaped `{ open, x, y }`
 * @param setMenu    its setter
 * @param menuRef    ref to the rendered menu element
 * @param closePatch extra fields merged in when closing (e.g. `{ mod: null }`)
 */
export function useDismissableContextMenu(menu, setMenu, menuRef, closePatch) {
  const { open, x, y } = menu;

  useEffect(() => {
    if (!open) return;

    const close = () =>
      setMenu((prev) => ({ ...prev, open: false, ...closePatch }));

    const handlePointerDown = (event) => {
      if (menuRef.current?.contains(event.target)) return;
      close();
    };

    const handleEscape = (event) => {
      if (event.key === "Escape") close();
    };

    const handleWindowChange = () => close();

    const adjustPosition = () => {
      const menuEl = menuRef.current;
      if (!menuEl) return;
      const rect = menuEl.getBoundingClientRect();
      const nextX = Math.min(x, Math.max(8, window.innerWidth - rect.width - 8));
      const nextY = Math.min(y, Math.max(8, window.innerHeight - rect.height - 8));
      if (nextX !== x || nextY !== y) {
        setMenu((prev) => ({ ...prev, x: nextX, y: nextY }));
      }
    };

    const raf = window.requestAnimationFrame(adjustPosition);
    window.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("keydown", handleEscape);
    window.addEventListener("resize", handleWindowChange);
    window.addEventListener("scroll", handleWindowChange, true);

    return () => {
      window.cancelAnimationFrame(raf);
      window.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("keydown", handleEscape);
      window.removeEventListener("resize", handleWindowChange);
      window.removeEventListener("scroll", handleWindowChange, true);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, x, y]);
}
