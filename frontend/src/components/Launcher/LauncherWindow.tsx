import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  AppWindow,
  Calculator,
  Info,
  LayoutGrid,
  Ruler,
  Search,
  Skull,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { LauncherResult } from "../../../../types/launcher";
import { copyText } from "../../lib/ascii-client";
import { cn } from "../../lib/cn";
import { launcherHide, launcherQuery, launcherRun } from "../../lib/launcher-client";
import { messageOf } from "../../lib/message";

const ICONS: Record<string, LucideIcon> = {
  calculator: Calculator,
  ruler: Ruler,
  "app-window": AppWindow,
  "layout-grid": LayoutGrid,
  zap: Zap,
  skull: Skull,
  info: Info,
};

const KIND_LABELS: Record<LauncherResult["kind"], string> = {
  calc: "Calculator",
  unit: "Convert",
  app: "App",
  tool: "Tool",
  action: "Action",
  process: "Process",
  info: "",
};

const QUERY_DELAY_MS = 40;

function domId(value: string) {
  return value.replace(/[^a-zA-Z0-9_-]/g, "-").slice(0, 80);
}

function answerOf(result: LauncherResult | undefined) {
  if (!result || (result.kind !== "calc" && result.kind !== "unit")) {
    return null;
  }
  return result.id.slice(result.id.indexOf(":") + 1);
}

export function LauncherWindow() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<LauncherResult[]>([]);
  const [active, setActive] = useState(0);
  const [message, setMessage] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const sequence = useRef(0);
  const queryRef = useRef(query);
  queryRef.current = query;

  const search = useCallback(async (text: string) => {
    const mine = ++sequence.current;
    try {
      const next = await launcherQuery(text);
      if (mine !== sequence.current) {
        return;
      }
      setResults(next);
      setActive(0);
    } catch (caught) {
      if (mine === sequence.current) {
        setMessage(messageOf(caught));
      }
    }
  }, []);

  useEffect(() => {
    document.title = "Launcher";
    void search("");
    const subscription = listen("launcher-show", () => {
      setQuery("");
      setMessage(null);
      void search("");
      requestAnimationFrame(() => inputRef.current?.focus());
    });
    return () => {
      void subscription.then((stop) => stop());
    };
  }, [search]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (focused) {
          inputRef.current?.focus();
        } else {
          void launcherHide();
        }
      })
      .then((stop) => {
        unlisten = stop;
      });
    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => void search(query), QUERY_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [query, search]);

  useEffect(() => {
    itemRefs.current[active]?.scrollIntoView({ block: "nearest" });
  }, [active, results]);

  async function run(result: LauncherResult | undefined) {
    if (!result || result.kind === "info") {
      return;
    }
    try {
      const reply = await launcherRun(result.id);
      if (reply) {
        setMessage(reply);
        void search(queryRef.current);
      } else {
        setMessage(null);
      }
    } catch (caught) {
      setMessage(messageOf(caught));
    }
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActive((index) => (results.length === 0 ? 0 : (index + 1) % results.length));
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setActive((index) => (results.length === 0 ? 0 : (index - 1 + results.length) % results.length));
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      void run(results[active]);
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      if (query) {
        setQuery("");
        setMessage(null);
      } else {
        void launcherHide();
      }
      return;
    }
    if (event.key.toLowerCase() === "c" && (event.ctrlKey || event.metaKey)) {
      const input = event.currentTarget;
      const answer = answerOf(results[active]);
      if (answer !== null && input.selectionStart === input.selectionEnd) {
        event.preventDefault();
        void copyText(answer)
          .then(() => setMessage(`Copied ${answer}`))
          .catch((caught) => setMessage(messageOf(caught)));
      }
    }
  }

  return (
    <div id="launcher-shell" className="flex h-full w-full p-2">
      <div
        id="launcher-panel"
        className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border border-zinc-700 bg-zinc-900 shadow-2xl"
      >
        <div id="launcher-search" className="flex items-center gap-3 border-b border-zinc-800 px-4 py-3">
          <Search id="launcher-search-icon" className="size-5 shrink-0 text-zinc-500" aria-hidden="true" />
          <label id="launcher-input-label" htmlFor="launcher-input" className="sr-only">
            Search apps, tools, and actions
          </label>
          <input
            id="launcher-input"
            ref={inputRef}
            autoFocus
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setMessage(null);
            }}
            onKeyDown={onKeyDown}
            placeholder="Search apps, calculate, convert units, kill a process"
            role="combobox"
            aria-expanded="true"
            aria-controls="launcher-results"
            aria-activedescendant={results[active] ? `launcher-result-${active}` : undefined}
            className="h-9 min-w-0 flex-1 bg-transparent text-lg text-zinc-100 outline-none placeholder:text-zinc-500"
          />
        </div>
        {message ? (
          <p id="launcher-message" className="border-b border-zinc-800 bg-zinc-950/60 px-4 py-2 text-sm text-zinc-200">
            {message}
          </p>
        ) : null}
        <div id="launcher-results" role="listbox" aria-label="Results" className="min-h-0 flex-1 overflow-y-auto p-1.5">
          {results.length === 0 ? (
            <p id="launcher-empty" className="px-3 py-8 text-center text-sm text-zinc-500">
              No matches
            </p>
          ) : (
            results.map((result, index) => {
              const Icon = ICONS[result.icon] ?? AppWindow;
              const selected = index === active;
              const key = `${index}-${domId(result.id)}`;
              return (
                <button
                  id={`launcher-result-${index}`}
                  key={key}
                  ref={(node) => {
                    itemRefs.current[index] = node;
                  }}
                  type="button"
                  role="option"
                  aria-selected={selected}
                  tabIndex={-1}
                  onMouseMove={() => setActive(index)}
                  onClick={() => void run(result)}
                  className={cn(
                    "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left",
                    selected ? "row-selected" : "hover:bg-zinc-800",
                    result.kind === "info" && "cursor-default",
                  )}
                >
                  <Icon
                    id={`launcher-result-${index}-icon`}
                    className={cn(
                      "size-5 shrink-0",
                      result.kind === "calc" || result.kind === "unit" ? "text-sky-400" : "text-zinc-400",
                      result.kind === "process" && "text-red-400",
                    )}
                    aria-hidden="true"
                  />
                  <span id={`launcher-result-${index}-text`} className="flex min-w-0 flex-1 flex-col">
                    <span
                      id={`launcher-result-${index}-title`}
                      className={cn(
                        "truncate text-zinc-100",
                        result.kind === "calc" || result.kind === "unit" ? "text-lg font-semibold" : "text-sm",
                      )}
                    >
                      {result.title}
                    </span>
                    {result.subtitle ? (
                      <span id={`launcher-result-${index}-subtitle`} className="truncate text-xs text-zinc-500">
                        {result.subtitle}
                      </span>
                    ) : null}
                  </span>
                  {KIND_LABELS[result.kind] ? (
                    <span id={`launcher-result-${index}-kind`} className="shrink-0 text-[11px] tracking-wide text-zinc-500 uppercase">
                      {KIND_LABELS[result.kind]}
                    </span>
                  ) : null}
                </button>
              );
            })
          )}
        </div>
        <footer id="launcher-footer" className="flex items-center gap-4 border-t border-zinc-800 px-4 py-2 text-xs text-zinc-500">
          <span id="launcher-footer-enter">Enter runs</span>
          <span id="launcher-footer-copy">Ctrl+C copies an answer</span>
          <span id="launcher-footer-escape">Esc closes</span>
          <span id="launcher-footer-examples" className="ml-auto truncate">
            Try 2^10, 5 km to mi, kill notepad, port 3000
          </span>
        </footer>
      </div>
    </div>
  );
}
