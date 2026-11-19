import { ArrowLeftRight, Copy } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { UnitsAnswer, UnitsCatalog } from "../../../../types/units";
import { copyText } from "../../lib/ascii-client";
import { cn } from "../../lib/cn";
import { messageOf } from "../../lib/message";
import { unitsCatalog, unitsQuery } from "../../lib/units-client";
import { Button } from "../Button";

const QUERY_KEY = "ce-desktop-tools.units-query";

const PAIRS = [
  ["km", "mi"],
  ["m", "ft"],
  ["in", "cm"],
  ["kg", "lb"],
  ["°F", "°C"],
  ["L", "gal"],
  ["cup", "ml"],
  ["mph", "km/h"],
  ["GB", "GiB"],
] as const;

const SEPARATORS = [" to ", " -> ", " => ", " = ", " as "];

function readQuery() {
  try {
    return localStorage.getItem(QUERY_KEY) ?? "";
  } catch {
    return "";
  }
}

function slugOf(symbol: string) {
  const slug = symbol
    .toLowerCase()
    .replaceAll("²", "2")
    .replaceAll("³", "3")
    .replaceAll("°", "deg")
    .replaceAll("µ", "u")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
  return slug || "unit";
}

function leadingAmount(text: string) {
  const trimmed = text.trim();
  const mixed = trimmed.match(/^([+-]?\d[\d,]*(?:\.\d+)?)\s+(\d[\d,]*)\s*\/\s*(\d[\d,]*)/);
  if (mixed) {
    return `${mixed[1]} ${mixed[2]}/${mixed[3]}`;
  }
  const fraction = trimmed.match(/^([+-]?\d[\d,]*(?:\.\d+)?)\s*\/\s*(\d[\d,]*)/);
  if (fraction) {
    return `${fraction[1]}/${fraction[2]}`;
  }
  const number = trimmed.match(/^([+-]?(?:\d[\d,]*(?:\.\d+)?|\.\d+))/);
  if (!number || number[1] === "+" || number[1] === "-" || number[1] === ".") {
    return null;
  }
  return number[1];
}

function leftOf(text: string) {
  const lower = text.toLowerCase();
  for (const separator of SEPARATORS) {
    const position = lower.indexOf(separator);
    if (position >= 0) {
      return text.slice(0, position).trim();
    }
  }
  const position = lower.lastIndexOf(" in ");
  if (position >= 0) {
    return text.slice(0, position).trim();
  }
  return null;
}

export function UnitsConverter() {
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState(readQuery);
  const [catalog, setCatalog] = useState<UnitsCatalog | null>(null);
  const [result, setResult] = useState<UnitsAnswer | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);

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
      // The last query is a convenience. The converter still works without it.
    }
  }, [query]);

  useEffect(() => {
    const text = query.trim();
    if (text.length === 0) {
      setResult(null);
      return;
    }
    let cancelled = false;
    void unitsQuery(text)
      .then((next) => {
        if (!cancelled) {
          setResult(next);
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
  }, [query]);

  const bare = result?.kind === "conversion" && !result.to;
  const headline = !result || result.kind === "message" ? "" : bare ? result.title : result.value;
  const detail = !result || result.kind === "message" || bare ? "" : result.title;
  const canSwap = Boolean(result?.from && result.to);

  async function copy(text: string, id: string) {
    if (text.length === 0) {
      return;
    }
    try {
      await copyText(text);
      setCopiedId(id);
      window.setTimeout(() => setCopiedId((current) => (current === id ? null : current)), 1200);
      setError(null);
    } catch (caught) {
      setError(messageOf(caught));
    }
  }

  function applyPair(from: string, to: string) {
    const amount = leadingAmount(query) ?? "1";
    setQuery(`${amount} ${from} to ${to}`);
    inputRef.current?.focus();
  }

  function chooseDimension(id: string) {
    if (result?.dimension === id && result.rows.length > 0) {
      return;
    }
    const unit = catalog?.units.find((item) => item.dimension === id);
    if (!unit) {
      return;
    }
    const amount = leadingAmount(query) ?? "1";
    setQuery(`${amount} ${unit.symbol}`);
    inputRef.current?.focus();
  }

  function swap() {
    if (!result?.from || !result.to) {
      return;
    }
    const amount = result.rows.find((row) => row.symbol === result.from)?.value ?? leadingAmount(query) ?? "1";
    setQuery(`${amount} ${result.to} to ${result.from}`);
    inputRef.current?.focus();
  }

  function retarget(symbol: string) {
    const left = leftOf(query) ?? query.trim();
    setQuery(left.length > 0 ? `${left} to ${symbol}` : `1 ${symbol}`);
    inputRef.current?.focus();
  }

  return (
    <section id="units-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="units-header" className="border-b border-zinc-800 px-4 py-3">
        <h1 id="units-title" className="text-base font-semibold text-zinc-100">
          Units
        </h1>
        <p id="units-intro" className="mt-1 text-xs leading-5 text-zinc-400">
          Type a conversion or a calculation. 5 km to mi, 72 f to c, 1/2 cup to ml, 5 ft 6 in, or (2+3)*4.
        </p>
      </header>
      {error ? (
        <p id="units-error" className="border-b border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
          {error}
        </p>
      ) : null}
      <div id="units-composer" className="border-b border-zinc-800 px-4 py-3">
        <label id="units-input-label" htmlFor="units-input" className="text-xs font-medium text-zinc-400">
          Conversion or calculation
        </label>
        <input
          id="units-input"
          ref={inputRef}
          value={query}
          placeholder="5 km to mi, 72 f to c, 1/2 cup to ml, (2+3)*4"
          spellCheck={false}
          autoCapitalize="off"
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && headline) {
              event.preventDefault();
              void copy(headline, "units-copy");
            }
          }}
          className="mt-1 h-12 w-full rounded-md border border-zinc-700 bg-zinc-950 px-3 font-mono text-lg text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-sky-500"
        />
        <div id="units-answer" className="mt-3 flex items-start justify-between gap-3">
          <div id="units-answer-text" className="min-w-0">
            {result?.kind === "message" && result.title ? (
              <p id="units-message" className="text-sm text-amber-300">
                {result.title}
              </p>
            ) : headline ? (
              <>
                <p id="units-value" className="truncate font-mono text-3xl font-semibold text-zinc-50 tabular-nums">
                  {headline}
                </p>
                {detail ? (
                  <p id="units-detail" className="mt-1 truncate text-sm text-zinc-400">
                    {detail}
                  </p>
                ) : (
                  <p id="units-detail" className="mt-1 text-sm text-zinc-500">
                    Every equivalent unit is below.
                  </p>
                )}
              </>
            ) : (
              <p id="units-idle" className="text-sm text-zinc-500">
                Enter copies the answer. Click a unit to switch the target.
              </p>
            )}
          </div>
          <div id="units-answer-actions" className="flex shrink-0 gap-1">
            <Button id="units-swap" variant="outline" disabled={!canSwap} onClick={swap} title="Swap units">
              <ArrowLeftRight id="units-swap-icon" className="size-3.5" aria-hidden="true" />
              <span id="units-swap-label">Swap</span>
            </Button>
            <Button id="units-copy" variant="primary" disabled={headline.length === 0} onClick={() => void copy(headline, "units-copy")}>
              <Copy id="units-copy-icon" className="size-3.5" aria-hidden="true" />
              <span id="units-copy-label">{copiedId === "units-copy" ? "Copied" : "Copy"}</span>
            </Button>
          </div>
        </div>
        <div id="units-pairs" className="mt-3 flex flex-wrap gap-1.5">
          {PAIRS.map(([from, to]) => {
            const id = `units-pair-${slugOf(from)}-${slugOf(to)}`;
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
      <div id="units-scroll" className="min-h-0 flex-1 overflow-auto px-4 py-3">
        <div id="units-dimensions" className="flex flex-wrap gap-1.5">
          {catalog?.dimensions.map((dimension) => {
            const selected = result?.dimension === dimension.id;
            return (
              <button
                id={`units-dimension-${dimension.id}`}
                key={dimension.id}
                type="button"
                aria-pressed={selected}
                onClick={() => chooseDimension(dimension.id)}
                className={cn(
                  "h-7 rounded-full px-2.5 text-xs font-medium",
                  selected ? "bg-sky-600 text-white" : "bg-zinc-800 text-zinc-300 hover:bg-zinc-700",
                )}
              >
                <span id={`units-dimension-${dimension.id}-label`}>{dimension.label}</span>
              </button>
            );
          })}
        </div>
        {result && result.rows.length > 0 ? (
          <ul id="units-grid" className="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-2 xl:grid-cols-3">
            {result.rows.map((row) => {
              const slug = slugOf(row.symbol);
              const active = row.symbol === result.to;
              const copyId = `units-copy-${slug}`;
              return (
                <li
                  id={`units-row-${slug}`}
                  key={row.symbol}
                  className={cn(
                    "flex items-stretch overflow-hidden rounded-md border",
                    active ? "border-sky-500 bg-sky-950/40" : "border-zinc-800 bg-zinc-900/60",
                  )}
                >
                  <button
                    id={copyId}
                    type="button"
                    onClick={() => void copy(row.value, copyId)}
                    className="flex min-w-0 flex-1 flex-col items-start px-3 py-2 text-left hover:bg-zinc-800/60"
                  >
                    <span id={`units-value-${slug}`} className="font-mono text-base text-zinc-100 tabular-nums">
                      {copiedId === copyId ? "Copied" : row.value}
                    </span>
                    <span id={`units-label-${slug}`} className="text-xs text-zinc-500">
                      {row.label}
                    </span>
                  </button>
                  <button
                    id={`units-target-${slug}`}
                    type="button"
                    title={`Convert to ${row.symbol}`}
                    onClick={() => retarget(row.symbol)}
                    className="border-l border-zinc-800 px-3 font-mono text-sm text-sky-300 hover:bg-zinc-800"
                  >
                    <span id={`units-symbol-${slug}`}>{row.symbol}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        ) : null}
      </div>
    </section>
  );
}
