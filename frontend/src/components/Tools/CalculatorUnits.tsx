import { ArrowLeftRight, Calculator as CalculatorIcon, Copy, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { CalcRef } from "../../../../types/calculator";
import type { UnitsAnswer, UnitsCatalog } from "../../../../types/units";
import { calculatorEval } from "../../lib/calculator-client";
import { displayOf, useCalculatorTape, withInsertedRef } from "../../lib/calculator-tape";
import { cn } from "../../lib/cn";
import { messageOf } from "../../lib/message";
import { unitsCatalog, unitsQuery } from "../../lib/units-client";
import { leadingAmount, leftOf, PAIRS, slugOf } from "../../lib/units-helpers";
import { Button } from "../Button";

const QUERY_KEY = "ce-desktop-tools.units-query";

type Live = {
  headline: string;
  detail: string;
  message: string;
  answer: UnitsAnswer | null;
};

const EMPTY: Live = { headline: "", detail: "", message: "", answer: null };

function readQuery() {
  try {
    return localStorage.getItem(QUERY_KEY) ?? "";
  } catch {
    return "";
  }
}

function liveOfUnits(answer: UnitsAnswer): Live {
  const bare = answer.kind === "conversion" && !answer.to;
  return {
    headline: bare ? answer.title : answer.value,
    detail: bare ? "" : answer.title,
    message: "",
    answer,
  };
}

// The units parser does not know #n or ans, so the calculator answers those and the grid comes from its result.
async function interpret(text: string, refs: CalcRef[]): Promise<Live> {
  const direct = await unitsQuery(text);
  if (direct.kind !== "message") {
    return liveOfUnits(direct);
  }
  const calc = await calculatorEval(text, refs);
  if (!calc.ok) {
    return { ...EMPTY, message: direct.title || calc.message };
  }
  const display = displayOf(calc);
  const grid = calc.unit ? await unitsQuery(display) : null;
  return {
    headline: display,
    detail: calc.title && calc.title !== display ? calc.title : "",
    message: "",
    answer: grid && grid.kind !== "message" ? grid : null,
  };
}

export function CalculatorUnits() {
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState(readQuery);
  const [catalog, setCatalog] = useState<UnitsCatalog | null>(null);
  const [live, setLive] = useState<Live>(EMPTY);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const tape = useCalculatorTape();
  const shown = [...tape.lines].reverse();

  const typed = query.trim();
  const selected = typed.length === 0 ? (tape.lines.find((line) => line.id === selectedId && line.ok) ?? null) : null;
  const source = typed.length > 0 ? typed : selected ? displayOf(selected) : "";
  const result = live.answer;
  const canSwap = Boolean(result?.from && result.to);

  useEffect(() => {
    inputRef.current?.focus();
    let cancelled = false;
    void unitsCatalog()
      .then((next) => {
        if (!cancelled) {
          setCatalog(next);
        }
      })
      .catch((caught) => {
        if (!cancelled) {
          setError(messageOf(caught));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    try {
      localStorage.setItem(QUERY_KEY, query);
    } catch {
      // The last query is a convenience. The tool still works without it.
    }
  }, [query]);

  useEffect(() => {
    if (source.length === 0) {
      setLive(EMPTY);
      return;
    }
    let cancelled = false;
    void interpret(source, tape.refs)
      .then((next) => {
        if (!cancelled) {
          setLive(next);
          setError(null);
        }
      })
      .catch((caught) => {
        if (!cancelled) {
          setError(messageOf(caught));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [source, tape.refs]);

  function edit(next: string) {
    setQuery(next);
    setSelectedId(null);
    inputRef.current?.focus();
  }

  function insertRef(id: number) {
    const element = inputRef.current;
    const start = element?.selectionStart ?? query.length;
    const end = element?.selectionEnd ?? query.length;
    const { next, cursor } = withInsertedRef(query, id, start, end);
    setQuery(next);
    setSelectedId(null);
    window.requestAnimationFrame(() => {
      element?.focus();
      element?.setSelectionRange(cursor, cursor);
    });
  }

  async function commit() {
    const saved = await tape.commit(query);
    if (!saved) {
      return;
    }
    setQuery("");
    inputRef.current?.focus();
  }

  function applyPair(from: string, to: string) {
    const amount = leadingAmount(source) ?? "1";
    edit(`${amount} ${from} to ${to}`);
  }

  function chooseDimension(id: string) {
    if (result?.dimension === id && result.rows.length > 0) {
      return;
    }
    const unit = catalog?.units.find((item) => item.dimension === id);
    if (!unit) {
      return;
    }
    const amount = leadingAmount(source) ?? "1";
    edit(`${amount} ${unit.symbol}`);
  }

  function swap() {
    if (!result?.from || !result.to) {
      return;
    }
    const amount = result.rows.find((row) => row.symbol === result.from)?.value ?? leadingAmount(source) ?? "1";
    edit(`${amount} ${result.to} to ${result.from}`);
  }

  function retarget(symbol: string) {
    if (selected) {
      edit(`#${selected.id} to ${symbol}`);
      return;
    }
    const left = leftOf(query) ?? typed;
    edit(left.length > 0 ? `${left} to ${symbol}` : `1 ${symbol}`);
  }

  const shownError = tape.error ?? error;

  return (
    <section id="calc-units-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="calc-units-header" className="flex items-start justify-between gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="calc-units-heading" className="min-w-0">
          <h1 id="calc-units-title" className="flex items-center gap-2 text-base font-semibold text-zinc-100">
            <CalculatorIcon id="calc-units-title-icon" className="size-4 text-zinc-400" aria-hidden="true" />
            <span id="calc-units-title-label">Calculator &amp; Units</span>
          </h1>
          <p id="calc-units-intro" className="mt-1 text-xs leading-5 text-zinc-400">
            Type a calculation or a conversion. 5 km to mi, 72 f to c, 5mm + 10in, or #1 * 2. Enter adds it to the tape.
          </p>
        </div>
        <Button id="calc-units-clear" variant="outline" disabled={tape.lines.length === 0} onClick={() => tape.clear()}>
          <Trash2 id="calc-units-clear-icon" className="size-3.5" aria-hidden="true" />
          <span id="calc-units-clear-label">Clear tape</span>
        </Button>
      </header>
      {shownError ? (
        <p id="calc-units-error" className="border-b border-amber-900/80 bg-amber-950/40 px-4 py-2 text-sm text-amber-200">
          {shownError}
        </p>
      ) : null}
      <div id="calc-units-composer" className="border-b border-zinc-800 px-4 py-3">
        <label id="calc-units-input-label" htmlFor="calc-units-input" className="text-xs font-medium text-zinc-400">
          Calculation or conversion
        </label>
        <input
          id="calc-units-input"
          ref={inputRef}
          value={query}
          disabled={!tape.ready}
          placeholder="5 km to mi, 1/2 cup to ml, 5mm + 10in, #1 + 2in"
          spellCheck={false}
          autoCapitalize="off"
          onChange={(event) => {
            setQuery(event.target.value);
            setSelectedId(null);
          }}
          onKeyDown={(event) => {
            if (event.key !== "Enter") {
              return;
            }
            event.preventDefault();
            if (event.ctrlKey) {
              if (live.headline) {
                void tape.copy(live.headline, "calc-units-copy");
              }
              return;
            }
            void commit();
          }}
          className="mt-1 h-12 w-full rounded-md border border-zinc-700 bg-zinc-950 px-3 font-mono text-lg text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-sky-500 disabled:opacity-60"
        />
        <div id="calc-units-answer" className="mt-3 flex items-start justify-between gap-3">
          <div id="calc-units-answer-text" className="min-w-0">
            {live.message ? (
              <p id="calc-units-message" className="text-sm text-amber-300">
                {live.message}
              </p>
            ) : live.headline ? (
              <>
                <p id="calc-units-value" className="truncate font-mono text-3xl font-semibold text-zinc-50 tabular-nums">
                  {live.headline}
                </p>
                <p id="calc-units-detail" className={cn("mt-1 truncate text-sm", live.detail ? "text-zinc-400" : "text-zinc-500")}>
                  {live.detail || (selected ? `From #${selected.id}: ${selected.input}` : "Enter adds it to the tape.")}
                </p>
              </>
            ) : (
              <p id="calc-units-idle" className="text-sm text-zinc-500">
                Enter adds the line to the tape. Ctrl+Enter copies the answer. Click a unit to switch the target.
              </p>
            )}
          </div>
          <div id="calc-units-answer-actions" className="flex shrink-0 gap-1">
            <Button id="calc-units-swap" variant="outline" disabled={!canSwap} onClick={swap} title="Swap units">
              <ArrowLeftRight id="calc-units-swap-icon" className="size-3.5" aria-hidden="true" />
              <span id="calc-units-swap-label">Swap</span>
            </Button>
            <Button
              id="calc-units-copy"
              variant="primary"
              disabled={live.headline.length === 0}
              onClick={() => void tape.copy(live.headline, "calc-units-copy")}
            >
              <Copy id="calc-units-copy-icon" className="size-3.5" aria-hidden="true" />
              <span id="calc-units-copy-label">{tape.copiedId === "calc-units-copy" ? "Copied" : "Copy"}</span>
            </Button>
          </div>
        </div>
        <div id="calc-units-pairs" className="mt-3 flex flex-wrap gap-1.5">
          {PAIRS.map(([from, to]) => {
            const id = `calc-units-pair-${slugOf(from)}-${slugOf(to)}`;
            return (
              <button
                id={id}
                key={id}
                type="button"
                onClick={() => applyPair(from, to)}
                className="h-7 rounded-full border border-zinc-700 bg-zinc-900 px-2.5 text-xs text-zinc-300 hover:border-zinc-500 hover:text-zinc-100"
              >
                <span id={`${id}-label`}>
                  {from} to {to}
                </span>
              </button>
            );
          })}
        </div>
      </div>
      <div id="calc-units-body" className="flex min-h-0 flex-1 flex-col overflow-auto lg:flex-row lg:overflow-hidden">
        <div
          id="calc-units-tape-panel"
          className="shrink-0 border-b border-zinc-800 lg:min-h-0 lg:w-80 lg:overflow-auto lg:border-r lg:border-b-0"
        >
          {shown.length === 0 ? (
            <p id="calc-units-tape-empty" className="px-4 py-6 text-sm text-zinc-500">
              Enter commits a line. Click its number to drop #n into the next calculation.
            </p>
          ) : (
            <ul id="calc-units-tape" className="divide-y divide-zinc-800">
              {shown.map((line) => {
                const display = line.ok ? displayOf(line) : "";
                const copyId = `calc-units-tape-copy-${line.id}`;
                const active = selected?.id === line.id;
                return (
                  <li
                    id={`calc-units-line-${line.id}`}
                    key={line.id}
                    className={cn("flex items-stretch gap-2 px-4 py-2", active && "bg-sky-950/40")}
                  >
                    <button
                      id={`calc-units-ref-${line.id}`}
                      type="button"
                      title={`Insert #${line.id}`}
                      onClick={() => insertRef(line.id)}
                      className="w-12 shrink-0 self-center rounded-md font-mono text-sm font-semibold text-sky-300 hover:bg-zinc-800"
                    >
                      <span id={`calc-units-ref-${line.id}-label`}>#{line.id}</span>
                    </button>
                    <div id={`calc-units-line-body-${line.id}`} className="min-w-0 flex-1">
                      <button
                        id={`calc-units-line-input-${line.id}`}
                        type="button"
                        title={`Show units for #${line.id}`}
                        disabled={!line.ok}
                        onClick={() => {
                          setQuery("");
                          setSelectedId(line.id);
                        }}
                        className="block max-w-full truncate text-left font-mono text-xs text-zinc-500 hover:text-zinc-300 disabled:hover:text-zinc-500"
                      >
                        {line.input}
                      </button>
                      {line.ok ? (
                        <button
                          id={copyId}
                          type="button"
                          onClick={() => void tape.copy(display, copyId)}
                          className="mt-0.5 block max-w-full truncate text-left font-mono text-lg text-zinc-100 tabular-nums hover:text-sky-200"
                        >
                          <span id={`calc-units-line-value-${line.id}`}>{tape.copiedId === copyId ? "Copied" : display}</span>
                        </button>
                      ) : (
                        <p id={`calc-units-line-message-${line.id}`} className="mt-0.5 text-sm text-amber-200">
                          {line.message}
                        </p>
                      )}
                    </div>
                    <button
                      id={`calc-units-delete-${line.id}`}
                      type="button"
                      title={`Delete #${line.id}`}
                      onClick={() => void tape.remove(line.id)}
                      className="self-center rounded-md p-1.5 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
                    >
                      <Trash2 id={`calc-units-delete-${line.id}-icon`} className="size-3.5" aria-hidden="true" />
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
        <div id="calc-units-grid-panel" className="min-w-0 px-4 py-3 lg:min-h-0 lg:flex-1 lg:overflow-auto">
          <div id="calc-units-dimensions" className="flex flex-wrap gap-1.5">
            {catalog?.dimensions.map((dimension) => {
              const pressed = result?.dimension === dimension.id;
              return (
                <button
                  id={`calc-units-dimension-${dimension.id}`}
                  key={dimension.id}
                  type="button"
                  aria-pressed={pressed}
                  onClick={() => chooseDimension(dimension.id)}
                  className={cn(
                    "h-7 rounded-full px-2.5 text-xs font-medium",
                    pressed ? "bg-sky-600 text-white" : "bg-zinc-800 text-zinc-300 hover:bg-zinc-700",
                  )}
                >
                  <span id={`calc-units-dimension-${dimension.id}-label`}>{dimension.label}</span>
                </button>
              );
            })}
          </div>
          {result && result.rows.length > 0 ? (
            <ul id="calc-units-grid" className="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-2 xl:grid-cols-3">
              {result.rows.map((row) => {
                const slug = slugOf(row.symbol);
                const active = row.symbol === result.to;
                const copyId = `calc-units-copy-${slug}`;
                return (
                  <li
                    id={`calc-units-row-${slug}`}
                    key={row.symbol}
                    className={cn(
                      "flex items-stretch overflow-hidden rounded-md border",
                      active ? "border-sky-500 bg-sky-950/40" : "border-zinc-800 bg-zinc-900/60",
                    )}
                  >
                    <button
                      id={copyId}
                      type="button"
                      onClick={() => void tape.copy(row.value, copyId)}
                      className="flex min-w-0 flex-1 flex-col items-start px-3 py-2 text-left hover:bg-zinc-800/60"
                    >
                      <span id={`calc-units-value-${slug}`} className="font-mono text-base text-zinc-100 tabular-nums">
                        {tape.copiedId === copyId ? "Copied" : row.value}
                      </span>
                      <span id={`calc-units-label-${slug}`} className="text-xs text-zinc-500">
                        {row.label}
                      </span>
                    </button>
                    <button
                      id={`calc-units-target-${slug}`}
                      type="button"
                      title={`Convert to ${row.symbol}`}
                      onClick={() => retarget(row.symbol)}
                      className="border-l border-zinc-800 px-3 font-mono text-sm text-sky-300 hover:bg-zinc-800"
                    >
                      <span id={`calc-units-symbol-${slug}`}>{row.symbol}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          ) : (
            <p id="calc-units-grid-empty" className="mt-3 text-sm text-zinc-500">
              A value with a unit lists every equivalent here. Click a tape line to see its units.
            </p>
          )}
        </div>
      </div>
    </section>
  );
}
