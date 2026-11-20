import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CalcRef, CalcResult } from "../../../types/calculator";
import { copyText } from "./ascii-client";
import { calculatorEval } from "./calculator-client";
import { messageOf } from "./message";

const TAPE_KEY = "ce-desktop-tools.calculator-tape";

export type StoredLine = {
  id: number;
  input: string;
};

export type TapeLine = StoredLine & CalcResult;

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

export function refsOf(lines: TapeLine[]): CalcRef[] {
  return lines
    .filter((line) => line.ok && line.base != null)
    .map((line) => ({
      id: line.id,
      base: line.base ?? 0,
      dimension: line.dimension,
      unit: line.unit,
    }));
}

export function displayOf(line: Pick<CalcResult, "value" | "unit">) {
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

function sameLines(current: TapeLine[], stored: StoredLine[]) {
  return (
    current.length === stored.length &&
    current.every((line, index) => line.id === stored[index]?.id && line.input === stored[index]?.input)
  );
}

// A lone number or word stays a search. `e` finds Edge, and `2` does not become a calculation.
// A calculation has a reference, parentheses, an operator next to a digit, a base literal, or a unit target.
export function looksLikeCalculation(text: string) {
  const trimmed = text.trim().replace(/^=+/, "").trim();
  if (!trimmed || /^(?:kill|port)(?:\s|$)/i.test(trimmed)) {
    return false;
  }
  if (trimmed.includes("#") || /(^|[^a-z])ans([^a-z]|$)/i.test(trimmed)) {
    return true;
  }
  if (/^0[xbo][0-9a-f]+$/i.test(trimmed)) {
    return true;
  }
  if (/[()]/.test(trimmed)) {
    return true;
  }
  if (!/\d/.test(trimmed)) {
    return false;
  }
  if (/[+\-*/%^!]/.test(trimmed)) {
    return true;
  }
  return /(?:^|\s)(?:to|in|as)\s+\S+\s*$/i.test(trimmed);
}

export function withInsertedRef(query: string, id: number, start: number, end: number) {
  const token = `#${id}`;
  const pad = start > 0 && query[start - 1] !== " " ? " " : "";
  const next = `${query.slice(0, start)}${pad}${token}${query.slice(end)}`;
  return { next, cursor: start + pad.length + token.length };
}

export function useCalculatorTape() {
  const [lines, setLines] = useState<TapeLine[]>([]);
  const [nextId, setNextId] = useState(1);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const linesRef = useRef(lines);
  const nextIdRef = useRef(nextId);
  const readyRef = useRef(ready);
  const writable = useRef(false);
  const epoch = useRef(0);
  const reloadGate = useRef<Promise<void> | null>(null);
  const copyTimer = useRef<number | null>(null);
  linesRef.current = lines;
  nextIdRef.current = nextId;
  readyRef.current = ready;

  const reload = useCallback(() => {
    const previous = reloadGate.current ?? Promise.resolve();
    const task = previous.then(async () => {
      const token = ++epoch.current;
      const stored = readTape();
      if (sameLines(linesRef.current, stored.lines) && readyRef.current) {
        if (token !== epoch.current) {
          return;
        }
        setNextId(stored.nextId);
        nextIdRef.current = stored.nextId;
        return;
      }
      try {
        const next = await recompute(stored.lines);
        if (token !== epoch.current) {
          return;
        }
        linesRef.current = next;
        const id = Math.max(stored.nextId, ...next.map((line) => line.id + 1), 1);
        nextIdRef.current = id;
        writable.current = true;
        setLines(next);
        setNextId(id);
        setReady(true);
      } catch (caught) {
        if (token !== epoch.current) {
          return;
        }
        setError(messageOf(caught));
        setReady(true);
      }
    });
    const settled = task.finally(() => {
      if (reloadGate.current === settled) {
        reloadGate.current = null;
      }
    });
    reloadGate.current = settled;
    return task;
  }, []);

  useEffect(() => {
    void reload();
    const onStorage = (event: StorageEvent) => {
      if (event.key !== TAPE_KEY && event.key !== null) {
        return;
      }
      void reload();
    };
    window.addEventListener("storage", onStorage);
    return () => {
      window.removeEventListener("storage", onStorage);
      if (copyTimer.current !== null) {
        window.clearTimeout(copyTimer.current);
      }
    };
  }, [reload]);

  useEffect(() => {
    if (!ready || !writable.current) {
      return;
    }
    const payload = JSON.stringify({
      nextId,
      lines: lines.map(({ id, input }) => ({ id, input })),
    });
    try {
      if (localStorage.getItem(TAPE_KEY) === payload) {
        return;
      }
      localStorage.setItem(TAPE_KEY, payload);
    } catch {
      // The tape still works for this session when storage is unavailable.
    }
  }, [ready, nextId, lines]);

  const refs = useMemo(() => refsOf(lines), [lines]);

  const copy = useCallback(async (text: string, id: string) => {
    try {
      await copyText(text);
      setCopiedId(id);
      if (copyTimer.current !== null) {
        window.clearTimeout(copyTimer.current);
      }
      copyTimer.current = window.setTimeout(() => {
        setCopiedId((current) => (current === id ? null : current));
      }, 1200);
      setError(null);
      return true;
    } catch (caught) {
      setError(messageOf(caught));
      return false;
    }
  }, []);

  const commit = useCallback(async (expression: string) => {
    const trimmed = expression.trim();
    if (!trimmed) {
      return false;
    }
    const pending = reloadGate.current;
    if (pending) {
      await pending;
    }
    if (!readyRef.current) {
      return false;
    }
    try {
      let result = await calculatorEval(trimmed, refsOf(linesRef.current));
      const raced = reloadGate.current;
      if (raced) {
        await raced;
        result = await calculatorEval(trimmed, refsOf(linesRef.current));
      }
      if (!result.ok) {
        setError(result.message);
        return false;
      }
      epoch.current += 1;
      const id = nextIdRef.current;
      const line = { id, input: trimmed, ...result };
      const next = [...linesRef.current, line];
      linesRef.current = next;
      nextIdRef.current = id + 1;
      if (writable.current || readTape().lines.length === 0) {
        writable.current = true;
      }
      setLines(next);
      setNextId(id + 1);
      setError(null);
      return true;
    } catch (caught) {
      setError(messageOf(caught));
      return false;
    }
  }, []);

  const remove = useCallback(async (id: number) => {
    const pending = reloadGate.current;
    if (pending) {
      await pending;
    }
    const kept = linesRef.current.filter((line) => line.id !== id).map(({ id: lineId, input }) => ({ id: lineId, input }));
    try {
      const next = await recompute(kept);
      epoch.current += 1;
      linesRef.current = next;
      writable.current = true;
      setLines(next);
      setError(null);
    } catch (caught) {
      setError(messageOf(caught));
    }
  }, []);

  const clear = useCallback(() => {
    epoch.current += 1;
    linesRef.current = [];
    nextIdRef.current = 1;
    writable.current = true;
    setLines([]);
    setNextId(1);
    setError(null);
  }, []);

  return { lines, refs, ready, error, setError, copiedId, reload, commit, remove, clear, copy };
}
