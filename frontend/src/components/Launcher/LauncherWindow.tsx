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
  Trash2,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent } from "react";
import type { CalcResult } from "../../../../types/calculator";
import type { LauncherResult } from "../../../../types/launcher";
import { calculatorEval } from "../../lib/calculator-client";
import {
  displayOf,
  looksLikeCalculation,
  useCalculatorTape,
  withInsertedRef,
  type TapeLine,
} from "../../lib/calculator-tape";
import { cn } from "../../lib/cn";
import { launcherHide, launcherQuery, launcherRun } from "../../lib/launcher-client";
import { messageOf } from "../../lib/message";
import { Button } from "../Button";

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

type LivePreview = {
  expression: string;
  result: CalcResult;
};

type LauncherRow =
  | { kind: "preview"; expression: string; result: CalcResult | null }
  | { kind: "tape"; line: TapeLine }
  | { kind: "result"; result: LauncherResult };

function domId(value: string) {
  return value.replace(/[^a-zA-Z0-9_-]/g, "-").slice(0, 80);
}

function answerOf(result: LauncherResult | undefined) {
  if (!result || (result.kind !== "calc" && result.kind !== "unit")) {
    return null;
  }
  return result.id.slice(result.id.indexOf(":") + 1);
}

function answerOfRow(row: LauncherRow | undefined) {
  if (!row) {
    return null;
  }
  if (row.kind === "preview") {
    return row.result?.ok ? displayOf(row.result) : null;
  }
  if (row.kind === "tape") {
    return row.line.ok ? displayOf(row.line) : null;
  }
  return answerOf(row.result);
}

function copyTargetId(row: LauncherRow, index: number) {
  if (row.kind === "preview") {
    return "launcher-preview-copy";
  }
  if (row.kind === "tape") {
    return `launcher-tape-copy-${row.line.id}`;
  }
  return `launcher-result-${index}-copy`;
}

function keepInput(event: MouseEvent) {
  event.preventDefault();
}

export function LauncherWindow() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<LauncherResult[]>([]);
  const [active, setActive] = useState(0);
  const [message, setMessage] = useState<string | null>(null);
  const [preview, setPreview] = useState<LivePreview | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const itemRefs = useRef<Array<HTMLDivElement | null>>([]);
  const sequence = useRef(0);
  const previewSeq = useRef(0);
  const queryRef = useRef(query);
  const {
    lines,
    refs,
    ready,
    error: tapeError,
    setError: setTapeError,
    copiedId,
    reload,
    commit,
    remove,
    clear,
    copy,
  } = useCalculatorTape();
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
      setTapeError(null);
      void search("");
      void reload();
      requestAnimationFrame(() => inputRef.current?.focus());
    });
    return () => {
      void subscription.then((stop) => stop());
    };
  }, [search, reload, setTapeError]);

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
    const expression = query.trim();
    if (!looksLikeCalculation(expression)) {
      setPreview(null);
      return;
    }
    if (!ready) {
      return;
    }
    const mine = ++previewSeq.current;
    const timer = window.setTimeout(() => {
      void calculatorEval(expression, refs)
        .then((result) => {
          if (mine === previewSeq.current) {
            setPreview({ expression, result });
          }
        })
        .catch((caught) => {
          if (mine !== previewSeq.current) {
            return;
          }
          const text = messageOf(caught);
          setPreview({
            expression,
            result: {
              ok: false,
              title: text,
              value: "",
              unit: null,
              dimension: null,
              base: null,
              message: text,
            },
          });
        });
    }, QUERY_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [query, ready, refs]);

  const calculating = looksLikeCalculation(query);
  const expression = query.trim();
  const live = preview && preview.expression === expression ? preview.result : null;
  const newest = useMemo(() => [...lines].reverse(), [lines]);
  const rows = useMemo(() => {
    const next: LauncherRow[] = [];
    if (calculating) {
      next.push({ kind: "preview", expression, result: live });
      if (live && !live.ok) {
        for (const result of results) {
          if (result.kind === "unit") {
            next.push({ kind: "result", result });
          }
        }
      }
      for (const line of newest) {
        next.push({ kind: "tape", line });
      }
      return next;
    }
    if (expression === "") {
      for (const line of newest) {
        next.push({ kind: "tape", line });
      }
    }
    for (const result of results) {
      next.push({ kind: "result", result });
    }
    return next;
  }, [calculating, expression, live, newest, results]);
  const rowsRef = useRef(rows);
  rowsRef.current = rows;
  const selected = rows.length === 0 ? 0 : Math.min(active, rows.length - 1);

  useEffect(() => {
    itemRefs.current[selected]?.scrollIntoView({ block: "nearest" });
  }, [selected, rows]);

  function insertTapeRef(id: number) {
    const element = inputRef.current;
    const current = queryRef.current;
    const start = element?.selectionStart ?? current.length;
    const end = element?.selectionEnd ?? current.length;
    const { next, cursor } = withInsertedRef(current, id, start, end);
    setQuery(next);
    setMessage(null);
    setTapeError(null);
    window.requestAnimationFrame(() => {
      element?.focus();
      element?.setSelectionRange(cursor, cursor);
    });
  }

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

  async function copyAnswer(row: LauncherRow, index: number) {
    const answer = answerOfRow(row);
    if (answer === null) {
      return;
    }
    const copied = await copy(answer, copyTargetId(row, index));
    if (copied) {
      setMessage(`Copied ${answer}`);
    }
  }

  async function activate(row: LauncherRow | undefined) {
    if (!row) {
      return;
    }
    if (row.kind === "preview") {
      if (row.result && !row.result.ok) {
        return;
      }
      const saved = await commit(queryRef.current);
      if (saved) {
        setQuery("");
        setMessage(null);
        inputRef.current?.focus();
      }
      return;
    }
    if (row.kind === "tape") {
      insertTapeRef(row.line.id);
      return;
    }
    await run(row.result);
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    const currentRows = rowsRef.current;
    const count = currentRows.length;
    const index = count === 0 ? 0 : Math.min(active, count - 1);
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActive(count === 0 ? 0 : (index + 1) % count);
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setActive(count === 0 ? 0 : (index - 1 + count) % count);
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      void activate(currentRows[index]);
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      if (query) {
        setQuery("");
        setMessage(null);
        setTapeError(null);
      } else {
        void launcherHide();
      }
      return;
    }
    if (event.key.toLowerCase() === "c" && (event.ctrlKey || event.metaKey)) {
      const input = event.currentTarget;
      const answer = answerOfRow(currentRows[index]);
      if (answer !== null && input.selectionStart === input.selectionEnd) {
        event.preventDefault();
        void copyAnswer(currentRows[index], index);
      }
    }
  }

  const banner = message ?? (tapeError && tapeError !== live?.message ? tapeError : null);
  const showTape = calculating || (expression === "" && lines.length > 0);

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
            spellCheck={false}
            autoCapitalize="off"
            onChange={(event) => {
              setQuery(event.target.value);
              setMessage(null);
              setTapeError(null);
            }}
            onKeyDown={onKeyDown}
            placeholder="Search apps, calculate, convert units, kill a process"
            role="combobox"
            aria-expanded="true"
            aria-controls="launcher-results"
            aria-activedescendant={rows[selected] ? `launcher-result-${selected}` : undefined}
            className="h-9 min-w-0 flex-1 bg-transparent text-lg text-zinc-100 outline-none placeholder:text-zinc-500"
          />
        </div>
        {banner ? (
          <p id="launcher-message" className="border-b border-zinc-800 bg-zinc-950/60 px-4 py-2 text-sm text-zinc-200">
            {banner}
          </p>
        ) : null}
        <div id="launcher-results" role="listbox" aria-label="Results" className="min-h-0 flex-1 overflow-y-auto p-1.5">
          {expression === "" && lines.length > 0 ? (
            <p id="launcher-recent-label" className="px-3 pt-1.5 pb-1 text-[11px] tracking-wide text-zinc-500 uppercase">
              Recent calculations
            </p>
          ) : null}
          {rows.length === 0 ? (
            <p id="launcher-empty" className="px-3 py-8 text-center text-sm text-zinc-500">
              No matches
            </p>
          ) : (
            rows.map((row, index) => {
              const isSelected = index === selected;
              return (
                <div
                  id={`launcher-result-${index}`}
                  key={rowKey(row, index)}
                  ref={(node) => {
                    itemRefs.current[index] = node;
                  }}
                  role="option"
                  aria-selected={isSelected}
                  onMouseMove={() => setActive(index)}
                  onMouseDown={row.kind === "result" ? undefined : keepInput}
                  onClick={() => void activate(row)}
                  className={cn(
                    "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left",
                    isSelected ? "row-selected" : "hover:bg-zinc-800",
                    (row.kind === "result" && row.result.kind === "info") ||
                      (row.kind === "preview" && row.result && !row.result.ok)
                      ? "cursor-default"
                      : "cursor-pointer",
                  )}
                >
                  {row.kind === "preview" ? (
                    <PreviewBody row={row} copiedId={copiedId} onCopy={() => void copyAnswer(row, index)} />
                  ) : null}
                  {row.kind === "tape" ? (
                    <TapeBody
                      line={row.line}
                      copiedId={copiedId}
                      onInsert={() => insertTapeRef(row.line.id)}
                      onCopy={() => void copyAnswer(row, index)}
                      onDelete={() => void remove(row.line.id)}
                    />
                  ) : null}
                  {row.kind === "result" ? <ResultBody result={row.result} index={index} /> : null}
                </div>
              );
            })
          )}
        </div>
        <footer id="launcher-footer" className="flex items-center gap-4 border-t border-zinc-800 px-4 py-2 text-xs text-zinc-500">
          {calculating ? (
            <span id="launcher-footer-enter">Enter saves the line</span>
          ) : (
            <span id="launcher-footer-enter">Enter runs</span>
          )}
          {showTape ? <span id="launcher-footer-ref">Enter on #n inserts it</span> : null}
          <span id="launcher-footer-copy">{calculating ? "Ctrl+C copies" : "Ctrl+C copies an answer"}</span>
          <span id="launcher-footer-escape">Esc closes</span>
          {showTape && lines.length > 0 ? (
            <Button id="launcher-tape-clear" variant="ghost" size="xs" onMouseDown={keepInput} onClick={() => clear()}>
              <span id="launcher-tape-clear-label">Clear</span>
            </Button>
          ) : null}
          <span id="launcher-footer-examples" className="ml-auto truncate">
            {calculating ? "#1 or ans reuses a saved line" : "Try 2^10, #1 + 2, 5 km to mi, kill notepad"}
          </span>
        </footer>
      </div>
    </div>
  );
}

function rowKey(row: LauncherRow, index: number) {
  if (row.kind === "preview") {
    return "preview";
  }
  if (row.kind === "tape") {
    return `tape-${row.line.id}`;
  }
  return `${index}-${domId(row.result.id)}`;
}

function PreviewBody({
  row,
  copiedId,
  onCopy,
}: {
  row: Extract<LauncherRow, { kind: "preview" }>;
  copiedId: string | null;
  onCopy: () => void;
}) {
  const display = row.result?.ok ? displayOf(row.result) : null;
  const copyId = "launcher-preview-copy";
  return (
    <>
      <Calculator id="launcher-preview-icon" className="size-5 shrink-0 text-sky-400" aria-hidden="true" />
      <span id="launcher-preview-text" className="flex min-w-0 flex-1 flex-col">
        <span id="launcher-preview-expression" className="truncate font-mono text-xs text-zinc-500">
          {row.expression}
        </span>
        {display ? (
          <button
            id={copyId}
            type="button"
            onMouseDown={keepInput}
            onClick={(event) => {
              event.stopPropagation();
              onCopy();
            }}
            className="truncate text-left text-lg font-semibold text-zinc-100 tabular-nums hover:text-sky-200"
          >
            <span id="launcher-preview-value">{copiedId === copyId ? "Copied" : display}</span>
          </button>
        ) : (
          <span
            id="launcher-preview-value"
            className={cn("truncate font-semibold", row.result && !row.result.ok ? "text-sm text-amber-200" : "text-lg text-zinc-500")}
          >
            {row.result && !row.result.ok ? row.result.message : "…"}
          </span>
        )}
      </span>
      <span id="launcher-preview-kind" className="shrink-0 text-[11px] tracking-wide text-zinc-500 uppercase">
        Calculator
      </span>
    </>
  );
}

function TapeBody({
  line,
  copiedId,
  onInsert,
  onCopy,
  onDelete,
}: {
  line: TapeLine;
  copiedId: string | null;
  onInsert: () => void;
  onCopy: () => void;
  onDelete: () => void;
}) {
  const display = line.ok ? displayOf(line) : "";
  const copyId = `launcher-tape-copy-${line.id}`;
  return (
    <>
      <button
        id={`launcher-tape-ref-${line.id}`}
        type="button"
        title={`Insert #${line.id}`}
        onMouseDown={keepInput}
        onClick={(event) => {
          event.stopPropagation();
          onInsert();
        }}
        className="w-10 shrink-0 rounded-md font-mono text-sm font-semibold text-sky-300 hover:bg-zinc-800"
      >
        <span id={`launcher-tape-ref-${line.id}-label`}>#{line.id}</span>
      </button>
      <span id={`launcher-tape-text-${line.id}`} className="flex min-w-0 flex-1 flex-col">
        <span id={`launcher-tape-input-${line.id}`} className="truncate font-mono text-xs text-zinc-500">
          {line.input}
        </span>
        {line.ok ? (
          <button
            id={copyId}
            type="button"
            onMouseDown={keepInput}
            onClick={(event) => {
              event.stopPropagation();
              onCopy();
            }}
            className="truncate text-left font-mono text-sm text-zinc-100 tabular-nums hover:text-sky-200"
          >
            <span id={`launcher-tape-value-${line.id}`}>{copiedId === copyId ? "Copied" : display}</span>
          </button>
        ) : (
          <span id={`launcher-tape-message-${line.id}`} className="truncate text-sm text-amber-200">
            {line.message}
          </span>
        )}
      </span>
      <button
        id={`launcher-tape-delete-${line.id}`}
        type="button"
        title={`Delete #${line.id}`}
        onMouseDown={keepInput}
        onClick={(event) => {
          event.stopPropagation();
          onDelete();
        }}
        className="shrink-0 rounded-md p-1.5 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
      >
        <Trash2 id={`launcher-tape-delete-${line.id}-icon`} className="size-3.5" aria-hidden="true" />
      </button>
    </>
  );
}

function ResultBody({ result, index }: { result: LauncherResult; index: number }) {
  const Icon = ICONS[result.icon] ?? AppWindow;
  return (
    <>
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
    </>
  );
}
