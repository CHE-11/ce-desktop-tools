import { listen } from "@tauri-apps/api/event";
import { MoveHorizontal, MoveVertical, Scan, SquareDashed, X, type LucideIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { MeasureStyle } from "../../../../../types/screen-ruler";
import { cn } from "../../../lib/cn";
import { screenRulerStyle, screenRulerClose, screenRulerMeasure } from "../../../lib/screen-ruler-client";

const MODES: { id: Exclude<MeasureStyle, "none">; label: string; icon: LucideIcon }[] = [
  { id: "bounds", label: "Bounds", icon: SquareDashed },
  { id: "spacing", label: "Spacing", icon: Scan },
  { id: "horizontal", label: "Horizontal spacing", icon: MoveHorizontal },
  { id: "vertical", label: "Vertical spacing", icon: MoveVertical },
];

/** The Screen Ruler command bar. It runs in its own small window. */
export function RulerToolbar() {
  const [style, setStyle] = useState<MeasureStyle>("none");

  useEffect(() => {
    document.title = "Screen Ruler";
    void screenRulerStyle()
      .then(setStyle)
      .catch(() => undefined);
    const subscription = listen<MeasureStyle>("screen-ruler-style", (event) => setStyle(event.payload));
    return () => {
      void subscription.then((stop) => stop());
    };
  }, []);

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") {
        void screenRulerClose();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  function choose(next: MeasureStyle) {
    const target = next === style ? "none" : next;
    setStyle(target);
    void screenRulerMeasure(target);
  }

  return (
    <div id="ruler-toolbar" className="flex h-screen w-screen items-center justify-center p-1">
      <div
        id="ruler-toolbar-bar"
        data-tauri-drag-region
        className="flex items-center gap-1 rounded-lg border border-zinc-700 bg-zinc-900 px-1.5 py-1 shadow-lg"
      >
        {MODES.map((mode) => {
          const Icon = mode.icon;
          const active = style === mode.id;
          return (
            <button
              id={`ruler-toolbar-${mode.id}`}
              key={mode.id}
              type="button"
              title={mode.label}
              aria-label={mode.label}
              aria-pressed={active}
              onClick={() => choose(mode.id)}
              className={cn(
                "flex size-8 items-center justify-center rounded-md",
                active ? "bg-sky-600 text-white" : "text-zinc-300 hover:bg-zinc-800",
              )}
            >
              <Icon id={`ruler-toolbar-${mode.id}-icon`} className="size-4" aria-hidden="true" />
            </button>
          );
        })}
        <span id="ruler-toolbar-divider" className="mx-1 h-5 w-px bg-zinc-700" />
        <button
          id="ruler-toolbar-close"
          type="button"
          title="Close"
          aria-label="Close Screen Ruler"
          onClick={() => void screenRulerClose()}
          className="flex size-8 items-center justify-center rounded-md text-zinc-300 hover:bg-red-600 hover:text-white"
        >
          <X id="ruler-toolbar-close-icon" className="size-4" aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}
