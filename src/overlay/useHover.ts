import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";

// Overlay okno je click-through, DOM mouse eventy nechodí — pozici myši
// polluje Rust (get_mouse_position, globální souřadnice = CSS px, okno
// kryje celý primární monitor od 0,0).
//
// Hover cíle: maskot (.act/.bd) I KARTA (.hover-card), jinak by karta
// zmizela cestou myši k ní. K tomu ochranná lhůta GRACE_MS — karta
// nezmizí, dokud je myš mimo cíle kratší dobu (přejezd mezi maskotem
// a kartou).

const POLL_MS = 100;
const PADDING = 14;
const GRACE_MS = 1000;

export function useHover() {
  const [hover, setHover] = useState(false);
  const hoverRef = useRef(false);
  const lastInsideAt = useRef(0);
  // Maskot letí přes celou obrazovku a může podletět pod nehybným
  // kurzorem — to není záměr uživatele. Hover proto povolíme až
  // poté, co s myší skutečně pohne.
  const startPos = useRef<{ x: number; y: number } | null>(null);
  const moved = useRef(false);

  useEffect(() => {
    let alive = true;

    const timer = setInterval(async () => {
      if (!alive) return;
      let pos: { clientX: number; clientY: number } | null = null;
      try {
        pos = await invoke<{ clientX: number; clientY: number } | null>(
          "get_mouse_position"
        );
      } catch {
        return;
      }
      if (!pos) return;

      if (!moved.current) {
        if (startPos.current === null) {
          startPos.current = { x: pos.clientX, y: pos.clientY };
          return;
        }
        const dx = Math.abs(pos.clientX - startPos.current.x);
        const dy = Math.abs(pos.clientY - startPos.current.y);
        if (dx + dy < 8) return; // myš stojí → žádný hover
        moved.current = true;
      }

      const targets = document.querySelectorAll<HTMLElement>(
        ".act, .bd, .hover-card"
      );
      let inside = false;
      targets.forEach((el) => {
        const r = el.getBoundingClientRect();
        if (
          pos!.clientX >= r.left - PADDING &&
          pos!.clientX <= r.right + PADDING &&
          pos!.clientY >= r.top - PADDING &&
          pos!.clientY <= r.bottom + PADDING
        ) {
          inside = true;
        }
      });

      const now = Date.now();
      if (inside) lastInsideAt.current = now;

      // zapnout hned, vypnout až po GRACE_MS mimo cíle
      const next = inside || now - lastInsideAt.current < GRACE_MS
        ? hoverRef.current || inside
        : false;

      if (next !== hoverRef.current) {
        hoverRef.current = next;
        setHover(next);
        invoke("set_overlay_interactive", { interactive: next }).catch(
          () => undefined
        );
      }
    }, POLL_MS);

    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);

  return hover;
}
