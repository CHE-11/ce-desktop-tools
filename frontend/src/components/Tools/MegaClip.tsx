import { listen } from "@tauri-apps/api/event";
import { Copy } from "lucide-react";
import { useEffect, useState } from "react";
import type { MegaClipState } from "../../../../types/megaclip";
import { megaclipClear, megaclipCopy, megaclipSetIndex, megaclipState } from "../../lib/megaclip-client";
import { cn } from "../../lib/cn";
import { messageOf } from "../../lib/message";
import { Button } from "../Button";

const EMPTY: MegaClipState = { clips: [], index: 0 };

export function MegaClip() {
  const [stack, setStack] = useState<MegaClipState>(EMPTY);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void megaclipState()
      .then((next) => {
        if (!cancelled) {
          setStack(next);
        }
      })
      .catch((caught) => {
        if (!cancelled) {
          setError(messageOf(caught));
        }
      });
    const subscription = listen<MegaClipState>("megaclip-changed", (event) => setStack(event.payload));
    return () => {
      cancelled = true;
      void subscription.then((stop) => stop());
    };
  }, []);

  const position = stack.clips.length === 0 ? "Empty" : `${stack.index + 1} of ${stack.clips.length}`;

  return (
    <section id="megaclip-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="megaclip-header" className="flex items-start justify-between gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="megaclip-heading" className="min-w-0">
          <h1 id="megaclip-title" className="text-base font-semibold text-zinc-100">
            MegaClip
          </h1>
          <p id="megaclip-position" className="mt-1 text-xs text-zinc-400">
            Next paste: {position}
          </p>
        </div>
        <Button
          id="megaclip-clear"
          variant="outline"
          disabled={stack.clips.length === 0}
          onClick={() => void megaclipClear().then(setStack).catch((caught) => setError(messageOf(caught)))}
        >
          <span id="megaclip-clear-label">Clear</span>
        </Button>
      </header>
      <ul id="megaclip-hotkeys" className="grid gap-1 border-b border-zinc-800 px-4 py-3 text-xs text-zinc-400 sm:grid-cols-2">
        <li id="megaclip-hotkey-copy">Alt+Ctrl+C copies the selection onto the stack</li>
        <li id="megaclip-hotkey-paste">Alt+Ctrl+V pastes the current snippet and moves on</li>
        <li id="megaclip-hotkey-next">Alt+CapsLock shows the next snippet</li>
        <li id="megaclip-hotkey-clear">CapsLock+C clears the stack without typing C</li>
      </ul>
      {error ? (
        <p id="megaclip-error" className="border-b border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
          {error}
        </p>
      ) : null}
      <div id="megaclip-scroll" className="min-h-0 flex-1 overflow-auto">
        {stack.clips.length === 0 ? (
          <p id="megaclip-empty" className="px-4 py-8 text-sm text-zinc-500">
            Copy something with Alt+Ctrl+C. The stack stays in memory until you clear it.
          </p>
        ) : (
          <ol id="megaclip-list" className="px-2 py-2">
            {stack.clips.map((clip, index) => {
              const current = index === stack.index;
              const preview = clip.replace(/\s+/g, " ");
              return (
                <li id={`megaclip-item-${index}`} key={`${index}-${preview.slice(0, 12)}`} className="mb-1">
                  <div
                    id={`megaclip-row-${index}`}
                    title={clip}
                    className={cn(
                      "flex items-center gap-2 rounded-md px-2 py-1.5",
                      current ? "bg-zinc-800" : "hover:bg-zinc-900",
                    )}
                  >
                    <span id={`megaclip-number-${index}`} className="w-8 shrink-0 text-xs text-zinc-500 tabular-nums">
                      {index + 1}
                    </span>
                    <span id={`megaclip-text-${index}`} className="min-w-0 flex-1 truncate text-sm text-zinc-200">
                      {preview}
                    </span>
                    <div id={`megaclip-actions-${index}`} className="flex shrink-0 items-center">
                      <button
                        id={`megaclip-copy-${index}`}
                        type="button"
                        title="Copy to clipboard"
                        aria-label="Copy to clipboard"
                        onClick={() => void megaclipCopy(index).catch((caught) => setError(messageOf(caught)))}
                        className="inline-flex size-7 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
                      >
                        <Copy id={`megaclip-copy-icon-${index}`} className="size-3.5" aria-hidden="true" />
                      </button>
                      <button
                        id={`megaclip-next-${index}`}
                        type="button"
                        title="Paste this next"
                        aria-pressed={current}
                        onClick={() => void megaclipSetIndex(index).then(setStack).catch((caught) => setError(messageOf(caught)))}
                        className={cn(
                          "inline-flex h-7 items-center rounded px-2 text-xs font-medium",
                          current ? "text-sky-300" : "text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100",
                        )}
                      >
                        <span id={`megaclip-next-label-${index}`}>Next</span>
                      </button>
                    </div>
                  </div>
                </li>
              );
            })}
          </ol>
        )}
      </div>
    </section>
  );
}
