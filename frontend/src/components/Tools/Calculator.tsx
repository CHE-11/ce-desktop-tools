import { Calculator as CalculatorIcon, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { CalcRef, CalcResult } from "../../../../types/calculator";
import { copyText } from "../../lib/ascii-client";
import { calculatorEval } from "../../lib/calculator-client";
import { cn } from "../../lib/cn";
import { messageOf } from "../../lib/message";
import { Button } from "../Button";

const TAPE_KEY = "ce-desktop-tools.calculator-tape";

type StoredLine = {
  id: number;
  input: string;
};

type TapeLine = StoredLine & CalcResult;

type StoredTape = {
  nextId: number;
  lines: StoredLine[];
};

function readTape(): StoredTape {
  try {
    const raw = localStorage.getItem(TAPE_KEY);
    if (!raw) {
      return { nextId: 1, lines: [] };
    }
    const parsed = JSON.parse(raw) as Partial<StoredTape>;
    const lines = Array.isArray(parsed.lines)
      ? parsed.lines.filter((line): line is StoredLine => {
          return !!line && typeof line.id === "number" && typeof line.input === "string";
        })
      : [];
    const nextId = typeof parsed.nextId === "number" ? parsed.nextId : 1;
    return { nextId: Math.max(nextId, ...lines.map((line) => line.id + 1), 1), lines };
  } catch {
    return { nextId: 1, lines: [] };
  }
}

function refsOf(lines: TapeLine[]): CalcRef[] {
  return lines
    .filter((line) => line.ok && line.base != null)
    .map((line) => ({
      id: line.id,
      base: line.base ?? 0,
      dimension: line.dimension,
      unit: line.unit,
    }));
}

function displayOf(line: Pick<CalcResult, "value" | "unit">) {
  return line.unit ? `${line.value} ${line.unit}` : line.value;
}

async function recompute(stored: StoredLine[]) {
  const lines: TapeLine[] = [];
  for (const line of stored) {
    const result = await calculatorEval(line.input, refsOf(lines));
    lines.push({ ...line, ...result });
  }
  return lines;
}

export function Calculator() {
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [lines, setLines] = useState<TapeLine[]>([]);
  const [nextId, setNextId] = useState(1);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);

  useEffect(() => {
    inputRef.current?.focus();
    let cancelled = false;
    const stored = readTape();
    void recompute(stored.lines)
      .then((next) => {
        if (cancelled) {
          return;
        }
        setLines(next);
        setNextId(Math.max(stored.nextId, ...next.map((line) => line.id + 1), 1));
        setReady(true);
      })
      .catch((caught) => {
        if (!cancelled) {
          setError(messageOf(caught));
          setReady(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!ready) {
      return;
    }
    const payload: StoredTape = {
      nextId,
      lines: lines.map(({ id, input }) => ({ id, input })),
    };
    try {
      localStorage.setItem(TAPE_KEY, JSON.stringify(payload));
    } catch {
      // The tape still works for this session when storage is unavailable.
    }
  }, [ready, nextId, lines]);

  async function copy(text: string, id: string) {
    try {
      await copyText(text);
      setCopiedId(id);
      window.setTimeout(() => setCopiedId((current) => (current === id ? null : current)), 1200);
      setError(null);
    } catch (caught) {
      setError(messageOf(caught));
    }
  }

  async function commit() {
    const expression = query.trim();
    if (!expression || !ready) {
      return;
    }
    try {
      const result = await calculatorEval(expression, refsOf(lines));
      if (!result.ok) {
        setError(result.message);
        return;
      }
      setLines((current) => [...current, { id: nextId, input: expression, ...result }]);
      setNextId((current) => current + 1);
      setQuery("");
      setError(null);
      inputRef.current?.focus();
    } catch (caught) {
      setError(messageOf(caught));
    }
  }

  async function remove(id: number) {
    const kept = lines.filter((line) => line.id !== id).map(({ id: lineId, input }) => ({ id: lineId, input }));
    try {
      setLines(await recompute(kept));
      setError(null);
    } catch (caught) {
      setError(messageOf(caught));
    }
  }

  function insertRef(id: number) {
    const token = `#${id}`;
    const element = inputRef.current;
    const start = element?.selectionStart ?? query.length;
    const end = element?.selectionEnd ?? query.length;
    const pad = start > 0 && query[start - 1] !== " " ? " " : "";
    const next = `${query.slice(0, start)}${pad}${token}${query.slice(end)}`;
    setQuery(next);
    const cursor = start + pad.length + token.length;
    window.requestAnimationFrame(() => {
      element?.focus();
      element?.setSelectionRange(cursor, cursor);
    });
  }

  const shown = [...lines].reverse();

  return (
    <section id="calculator-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="calculator-header" className="flex items-start justify-between gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="calculator-heading" className="min-w-0">
          <h1 id="calculator-title" className="flex items-center gap-2 text-base font-semibold text-zinc-100">
            <CalculatorIcon id="calculator-title-icon" className="size-4 text-zinc-400" aria-hidden="true" />
            <span id="calculator-title-label">Calculator</span>
          </h1>
          <p id="calculator-intro" className="mt-1 text-xs leading-5 text-zinc-400">
            Each result keeps a number. Use #1 or ans in a later line. 1 + .5, 5mm + 10in, and #1 + 2 all work.
          </p>
        </div>
        <Button
          id="calculator-clear"
          variant="outline"
          disabled={lines.length === 0}
          onClick={() => {
            setLines([]);
            setNextId(1);
          }}
        >
          <Trash2 id="calculator-clear-icon" className="size-3.5" aria-hidden="true" />
          <span id="calculator-clear-label">Clear</span>
        </Button>
      </header>
      {error ? (
        <p id="calculator-error" className="border-b border-amber-900/80 bg-amber-950/40 px-4 py-2 text-sm text-amber-200">
          {error}
        </p>
      ) : null}
      <div id="calculator-composer" className="border-b border-zinc-800 px-4 py-3">
        <label id="calculator-input-label" htmlFor="calculator-input" className="text-xs font-medium text-zinc-400">
          Calculation
        </label>
        <input
          id="calculator-input"
          ref={inputRef}
          value={query}
          disabled={!ready}
          placeholder="1 + .5, 5mm + 10in, #1 + 2in"
          spellCheck={false}
          autoCapitalize="off"
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              void commit();
            }
          }}
          className="mt-1 h-12 w-full rounded-md border border-zinc-700 bg-zinc-950 px-3 font-mono text-lg text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-sky-500 disabled:opacity-60"
        />
      </div>
      <div id="calculator-scroll" className="min-h-0 flex-1 overflow-auto">
        {shown.length === 0 ? (
          <p id="calculator-empty" className="px-4 py-6 text-sm text-zinc-500">
            Enter commits a line. Click its number to drop #n into the next calculation.
          </p>
        ) : (
          <ul id="calculator-tape" className="divide-y divide-zinc-800">
            {shown.map((line) => {
              const display = line.ok ? displayOf(line) : "";
              const copyId = `calculator-copy-${line.id}`;
              return (
                <li id={`calculator-line-${line.id}`} key={line.id} className="flex items-stretch gap-2 px-4 py-2">
                  <button
                    id={`calculator-ref-${line.id}`}
                    type="button"
                    title={`Insert #${line.id}`}
                    onClick={() => insertRef(line.id)}
                    className="w-12 shrink-0 self-center rounded-md font-mono text-sm font-semibold text-sky-300 hover:bg-zinc-800"
                  >
                    <span id={`calculator-ref-${line.id}-label`}>#{line.id}</span>
                  </button>
                  <div id={`calculator-body-${line.id}`} className="min-w-0 flex-1">
                    <p id={`calculator-input-${line.id}`} className="truncate font-mono text-xs text-zinc-500">
                      {line.input}
                    </p>
                    {line.ok ? (
                      <button
                        id={copyId}
                        type="button"
                        onClick={() => void copy(display, copyId)}
                        className="mt-0.5 block max-w-full truncate text-left font-mono text-lg text-zinc-100 tabular-nums hover:text-sky-200"
                      >
                        <span id={`calculator-value-${line.id}`}>{copiedId === copyId ? "Copied" : display}</span>
                      </button>
                    ) : (
                      <p id={`calculator-message-${line.id}`} className="mt-0.5 text-sm text-amber-200">
                        {line.message}
                      </p>
                    )}
                  </div>
                  <button
                    id={`calculator-delete-${line.id}`}
                    type="button"
                    title={`Delete #${line.id}`}
                    onClick={() => void remove(line.id)}
                    className={cn("self-center rounded-md p-1.5 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200")}
                  >
                    <Trash2 id={`calculator-delete-${line.id}-icon`} className="size-3.5" aria-hidden="true" />
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </section>
  );
}
