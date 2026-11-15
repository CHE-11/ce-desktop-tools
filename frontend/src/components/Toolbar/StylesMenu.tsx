import { ALargeSmall, Check } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { patchAppSettings } from "../../lib/app-client";
import { DENSITY_EVENT, DENSITY_OPTIONS, normalizeDensity, publishDensity, readDensity, type Density } from "../../lib/density";
import { cn } from "../../lib/cn";
import { FONT_SIZE_OPTIONS, ZOOM_EVENT, clampZoom, publishZoom, readZoom } from "../../lib/zoom";
import { Button } from "../Button";

type StylesMenuProps = {
  labels: boolean;
};

export function StylesMenu({ labels }: StylesMenuProps) {
  const [open, setOpen] = useState(false);
  const [density, setDensity] = useState(readDensity);
  const [zoom, setZoom] = useState(readZoom);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function onDensity(event: Event) {
      const detail = (event as CustomEvent<string>).detail;
      if (typeof detail !== "string") {
        return;
      }
      setDensity(normalizeDensity(detail));
    }

    function onZoom(event: Event) {
      const detail = (event as CustomEvent<number>).detail;
      if (typeof detail !== "number" || !Number.isFinite(detail)) {
        return;
      }
      setZoom(clampZoom(detail));
    }

    window.addEventListener(DENSITY_EVENT, onDensity);
    window.addEventListener(ZOOM_EVENT, onZoom);
    return () => {
      window.removeEventListener(DENSITY_EVENT, onDensity);
      window.removeEventListener(ZOOM_EVENT, onZoom);
    };
  }, []);

  useEffect(() => {
    if (!open) {
      return;
    }

    function onPointerDown(event: PointerEvent) {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    }

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setOpen(false);
      }
    }

    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  function chooseDensity(next: Density) {
    const applied = publishDensity(next);
    setDensity(applied);
    void patchAppSettings({ density: applied }).catch(() => undefined);
  }

  function chooseFontSize(percent: number) {
    const applied = publishZoom(percent);
    setZoom(applied);
    void patchAppSettings({ fontZoom: applied }).catch(() => undefined);
  }

  return (
    <div id="toolbar-styles" ref={rootRef} className="relative">
      <Button
        id="toolbar-styles-button"
        variant="ghost"
        size="sm"
        title="Styles"
        aria-label="Styles"
        aria-expanded={open}
        aria-haspopup="menu"
        aria-controls={open ? "toolbar-styles-menu" : undefined}
        onClick={() => setOpen((current) => !current)}
      >
        <ALargeSmall id="toolbar-styles-icon" className="size-4" aria-hidden="true" />
        {labels ? <span id="toolbar-styles-label">Styles</span> : null}
      </Button>
      {open ? (
        <div
          id="toolbar-styles-menu"
          role="menu"
          aria-label="Styles"
          className="absolute top-full right-0 z-20 mt-1 w-max min-w-40 rounded-md border border-zinc-700 bg-zinc-900 p-1 shadow-lg"
        >
          <div id="toolbar-styles-density" role="group" aria-labelledby="toolbar-styles-density-label" className="flex flex-col">
            <p id="toolbar-styles-density-label" className="px-2 pt-1 pb-0.5 text-xs font-medium text-zinc-500">
              Density
            </p>
            {DENSITY_OPTIONS.map((option) => {
              const selected = density === option.id;
              return (
                <button
                  id={`toolbar-styles-density-${option.id}`}
                  key={option.id}
                  type="button"
                  role="menuitemradio"
                  aria-checked={selected}
                  onClick={() => chooseDensity(option.id)}
                  className="flex h-7 items-center gap-2 rounded px-2 text-left text-sm whitespace-nowrap text-zinc-200 hover:bg-zinc-800"
                >
                  <Check
                    id={`toolbar-styles-density-${option.id}-check`}
                    className={cn("size-3.5 shrink-0", selected ? "text-sky-400" : "invisible")}
                    aria-hidden="true"
                  />
                  <span id={`toolbar-styles-density-${option.id}-label`}>{option.label}</span>
                </button>
              );
            })}
          </div>
          <div id="toolbar-styles-sep" className="my-1 h-px bg-zinc-800" role="separator" />
          <div id="toolbar-styles-font" role="group" aria-labelledby="toolbar-styles-font-label" className="flex flex-col">
            <p id="toolbar-styles-font-label" className="px-2 pt-1 pb-0.5 text-xs font-medium text-zinc-500">
              Font size
            </p>
            {FONT_SIZE_OPTIONS.map((option) => {
              const selected = zoom === option.zoom;
              return (
                <button
                  id={`toolbar-styles-font-${option.id}`}
                  key={option.id}
                  type="button"
                  role="menuitemradio"
                  aria-checked={selected}
                  onClick={() => chooseFontSize(option.zoom)}
                  className="flex h-7 items-center gap-2 rounded px-2 text-left text-sm whitespace-nowrap text-zinc-200 hover:bg-zinc-800"
                >
                  <Check
                    id={`toolbar-styles-font-${option.id}-check`}
                    className={cn("size-3.5 shrink-0", selected ? "text-sky-400" : "invisible")}
                    aria-hidden="true"
                  />
                  <span id={`toolbar-styles-font-${option.id}-label`}>{option.label}</span>
                </button>
              );
            })}
          </div>
        </div>
      ) : null}
    </div>
  );
}
