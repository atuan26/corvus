import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

const MIN = 4;

function normalize(r: Rect): Rect {
  return {
    x: Math.min(r.x, r.x + r.w),
    y: Math.min(r.y, r.y + r.h),
    w: Math.abs(r.w),
    h: Math.abs(r.h),
  };
}

/** Full-screen picker over a frozen screenshot: drag, or use the keyboard, to choose an area. */
export default function SnipOverlay() {
  const [url, setUrl] = useState<string | null>(null);
  const [rect, setRect] = useState<Rect | null>(null);
  const [dragging, setDragging] = useState(false);
  const imgRef = useRef<HTMLImageElement>(null);
  const rootRef = useRef<HTMLDivElement>(null);

  const load = useCallback(async () => {
    setRect(null);
    setDragging(false);
    try {
      const bytes = await invoke<ArrayBuffer>("snip_image");
      setUrl((old) => {
        if (old) URL.revokeObjectURL(old);
        return URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
      });
    } catch {
      setUrl(null);
    }
    requestAnimationFrame(() => rootRef.current?.focus());
  }, []);

  useEffect(() => {
    void load();
    const un = listen("snip-ready", () => void load());
    return () => void un.then((f) => f());
  }, [load]);

  const finish = useCallback((r: Rect | null) => {
    const img = imgRef.current;
    if (!r || !img) return;
    const n = normalize(r);
    if (n.w < MIN || n.h < MIN) return;
    // CSS pixels → captured-image pixels (HiDPI screens differ).
    const sx = img.naturalWidth / img.clientWidth;
    const sy = img.naturalHeight / img.clientHeight;
    void invoke("snip_finish", {
      rect: { x: Math.round(n.x * sx), y: Math.round(n.y * sy), width: Math.round(n.w * sx), height: Math.round(n.h * sy) },
    });
    setUrl(null);
  }, []);

  const cancel = useCallback(() => {
    setUrl(null);
    void invoke("snip_cancel");
  }, []);

  const onKey = (e: React.KeyboardEvent) => {
    const W = window.innerWidth;
    const H = window.innerHeight;
    if (e.key === "Escape") {
      e.preventDefault();
      return cancel();
    }
    if (e.key === "Enter") {
      e.preventDefault();
      return finish(rect);
    }
    if ((e.ctrlKey || e.metaKey) && e.code === "KeyA") {
      e.preventDefault();
      return setRect({ x: 0, y: 0, w: W, h: H });
    }
    const arrows: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
    const dir = arrows[e.key];
    if (!dir) return;
    e.preventDefault();
    const step = e.altKey ? 1 : 10;
    const r = rect ? normalize(rect) : { x: W * 0.3, y: H * 0.4, w: W * 0.4, h: H * 0.15 };
    const next = e.shiftKey
      ? { ...r, w: Math.max(MIN, r.w + dir[0] * step), h: Math.max(MIN, r.h + dir[1] * step) }
      : { ...r, x: Math.min(Math.max(0, r.x + dir[0] * step), W - r.w), y: Math.min(Math.max(0, r.y + dir[1] * step), H - r.h) };
    setRect(next);
  };

  const n = rect ? normalize(rect) : null;
  return (
    <div
      ref={rootRef}
      className="snip"
      tabIndex={-1}
      role="application"
      aria-label="Choose a screen area to read"
      aria-describedby="snip-help"
      onKeyDown={onKey}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        (e.target as Element).setPointerCapture?.(e.pointerId);
        setDragging(true);
        setRect({ x: e.clientX, y: e.clientY, w: 0, h: 0 });
      }}
      onPointerMove={(e) => {
        if (dragging) setRect((r) => (r ? { ...r, w: e.clientX - r.x, h: e.clientY - r.y } : r));
      }}
      onPointerUp={() => {
        if (!dragging) return;
        setDragging(false);
        finish(rect);
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        cancel();
      }}
    >
      {url && <img ref={imgRef} src={url} alt="" className="snip-image" draggable={false} />}
      {!n && <div className="snip-dim" />}
      {n && <div className="snip-rect" style={{ left: n.x, top: n.y, width: n.w, height: n.h }} />}
      <p id="snip-help" className="snip-help" aria-live="polite">
        {n && n.w >= MIN
          ? `${Math.round(n.w)} × ${Math.round(n.h)}. Release or press Enter to read the text, Esc to cancel.`
          : "Drag over the text to read. Keyboard: arrows move, Shift+arrows resize, Ctrl+A whole screen, Enter confirms, Esc cancels."}
      </p>
    </div>
  );
}
