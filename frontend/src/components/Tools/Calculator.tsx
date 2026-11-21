import { Calculator as CalculatorIcon, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { displayOf, useCalculatorTape, withInsertedRef } from "../../lib/calculator-tape";
import { cn } from "../../lib/cn";
import { Button } from "../Button";

export function Calculator() {
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const tape = useCalculatorTape();
  const shown = [...tape.lines].reverse();

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  function insertRef(id: number) {
    const element = inputRef.current;
    const start = element?.selectionStart ?? query.length;
    const end = element?.selectionEnd ?? query.length;
    const { next, cursor } = withInsertedRef(query, id, start, end);
    setQuery(next);
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
        <Button id="calculator-clear" variant="outline" disabled={tape.lines.length === 0} onClick={() => tape.clear()}>
          <Trash2 id="calculator-clear-icon" className="size-3.5" aria-hidden="true" />
          <span id="calculator-clear-label">Clear</span>
        </Button>
      </header>
      {tape.error ? (
        <p id="calculator-error" className="border-b border-amber-900/80 bg-amber-950/40 px-4 py-2 text-sm text-amber-200">
          {tape.error}
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
          disabled={!tape.ready}
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
                        onClick={() => void tape.copy(display, copyId)}
                        className="mt-0.5 block max-w-full truncate text-left font-mono text-lg text-zinc-100 tabular-nums hover:text-sky-200"
                      >
                        <span id={`calculator-value-${line.id}`}>{tape.copiedId === copyId ? "Copied" : display}</span>
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
                    onClick={() => void tape.remove(line.id)}
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
