import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { Terminal, type ITheme } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { ChevronDown, ChevronUp, X } from "lucide-react";
import { useEffect, useImperativeHandle, useRef, useState, type Ref } from "react";
import type { TerminalSettings } from "../../../../../types/app";
import type { TerminalSession } from "../../../../../types/terminal";
import { openUrl } from "../../../lib/app-client";
import { copyText } from "../../../lib/ascii-client";
import { cn } from "../../../lib/cn";
import { attachTerminal, terminalClipboardText, terminalResize, terminalWrite } from "../../../lib/terminal-client";
import { Button } from "../../Button";

export type TerminalViewHandle = {
  focus: () => void;
  clear: () => void;
  copyAll: () => Promise<boolean>;
  copyLastOutput: () => Promise<boolean>;
  openSearch: () => void;
};

type Props = {
  ref?: Ref<TerminalViewHandle>;
  session: TerminalSession;
  active: boolean;
  settings: TerminalSettings;
  onPaste: (text: string) => void;
  onExit: (code: number | null) => void;
  onZoom: (delta: number) => void;
  onCopied: (message: string) => void;
};

const PROMPT = /^(PS [^\n]*>|[A-Za-z]:\\[^>]*>)/;

function cssVar(name: string, fallback: string) {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

function themeOf(): ITheme {
  return {
    background: cssVar("--app-zinc-950", "#09090b"),
    foreground: cssVar("--app-zinc-100", "#f4f4f5"),
    cursor: cssVar("--app-sky-400", "#38bdf8"),
    cursorAccent: cssVar("--app-zinc-950", "#09090b"),
    selectionBackground: "#0284c766",
    black: "#18181b",
    red: cssVar("--app-red-400", "#f87171"),
    green: cssVar("--app-emerald-400", "#34d399"),
    yellow: cssVar("--app-amber-300", "#fcd34d"),
    blue: cssVar("--app-sky-500", "#0ea5e9"),
    magenta: cssVar("--app-violet-300", "#c4b5fd"),
    cyan: "#22d3ee",
    white: cssVar("--app-zinc-300", "#d4d4d8"),
    brightBlack: cssVar("--app-zinc-500", "#71717a"),
    brightRed: "#fca5a5",
    brightGreen: "#6ee7b7",
    brightYellow: "#fde68a",
    brightBlue: cssVar("--app-sky-400", "#38bdf8"),
    brightMagenta: "#ddd6fe",
    brightCyan: "#67e8f9",
    brightWhite: "#ffffff",
  };
}

/** Trailing spaces on each line are padding from the grid, not text. */
function tidy(text: string) {
  return text
    .split(/\r?\n/)
    .map((line) => line.replace(/\s+$/, ""))
    .join("\r\n")
    .replace(/(\r\n)+$/, "");
}

/** Buffer rows as logical lines, with soft-wrapped rows joined back together. */
function bufferLines(term: Terminal) {
  const buffer = term.buffer.active;
  const lines: string[] = [];
  for (let index = 0; index < buffer.length; index += 1) {
    const row = buffer.getLine(index);
    if (!row) {
      continue;
    }
    const text = row.translateToString(true);
    if (row.isWrapped && lines.length > 0) {
      lines[lines.length - 1] += text;
    } else {
      lines.push(text);
    }
  }
  while (lines.length > 0 && lines[lines.length - 1].trim() === "") {
    lines.pop();
  }
  return lines;
}

/**
 * Arrow keys that move the shell's cursor to the clicked cell. Only clicks on the
 * line being typed count, and full-screen programs that read the mouse are left alone.
 */
function cursorMoves(term: Terminal, host: HTMLElement, clientX: number, clientY: number) {
  const buffer = term.buffer.active;
  if (term.hasSelection() || buffer.type !== "normal" || term.modes.mouseTrackingMode !== "none") {
    return null;
  }
  const screen = host.querySelector(".xterm-screen")?.getBoundingClientRect();
  if (!screen || screen.width === 0 || screen.height === 0) {
    return null;
  }
  const col = Math.floor(((clientX - screen.left) / screen.width) * term.cols);
  const row = Math.floor(((clientY - screen.top) / screen.height) * term.rows);
  if (col < 0 || row < 0 || col >= term.cols || row >= term.rows) {
    return null;
  }
  const clickedRow = buffer.viewportY + row;
  const cursorRow = buffer.baseY + buffer.cursorY;
  const { first, last } = inputRows(term);
  if (clickedRow < first || clickedRow > last) {
    return null;
  }
  return arrows((clickedRow - cursorRow) * term.cols + (col - buffer.cursorX));
}

function arrows(delta: number) {
  if (delta === 0) {
    return "";
  }
  return (delta > 0 ? "\x1b[C" : "\x1b[D").repeat(Math.abs(delta));
}

/** Buffer rows of the line being typed, including rows it soft-wrapped onto. */
function inputRows(term: Terminal) {
  const buffer = term.buffer.active;
  const cursorRow = buffer.baseY + buffer.cursorY;
  let first = cursorRow;
  while (first > 0 && buffer.getLine(first)?.isWrapped) {
    first -= 1;
  }
  let last = cursorRow;
  while (buffer.getLine(last + 1)?.isWrapped) {
    last += 1;
  }
  return { first, last };
}

/**
 * Keys that delete the selected text from the line being typed: move to the end of
 * the selection, then backspace over it. Backspaces that reach the prompt do nothing,
 * so a selection that starts in the prompt only removes the typed part.
 */
function deleteSelectionKeys(term: Terminal) {
  const buffer = term.buffer.active;
  const range = term.getSelectionPosition();
  if (!range || buffer.type !== "normal" || term.modes.mouseTrackingMode !== "none") {
    return null;
  }
  const { first, last } = inputRows(term);
  const startRow = range.start.y;
  const endRow = range.end.y;
  if (startRow < first || endRow > last) {
    return null;
  }
  const offset = (row: number, col: number) => (row - first) * term.cols + col;
  const lastText = buffer.getLine(last)?.translateToString(true).length ?? 0;
  const lineEnd = offset(last, lastText);
  const start = Math.min(offset(startRow, range.start.x), lineEnd);
  const end = Math.min(offset(endRow, range.end.x), lineEnd);
  if (end <= start) {
    return null;
  }
  const cursor = offset(buffer.baseY + buffer.cursorY, buffer.cursorX);
  return arrows(end - cursor) + "\x7f".repeat(end - start);
}

export function TerminalView({ ref, session, active, settings, onPaste, onExit, onZoom, onCopied }: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const searchRef = useRef<SearchAddon | null>(null);
  const settingsRef = useRef(settings);
  const callbacks = useRef({ onPaste, onExit, onZoom, onCopied });
  const [searchOpen, setSearchOpen] = useState(false);
  const [query, setQuery] = useState("");
  const searchInput = useRef<HTMLInputElement>(null);

  settingsRef.current = settings;
  callbacks.current = { onPaste, onExit, onZoom, onCopied };

  async function copy(text: string, message: string) {
    if (!text) {
      return false;
    }
    await copyText(text);
    callbacks.current.onCopied(message);
    return true;
  }

  function copySelection() {
    const term = termRef.current;
    if (!term?.hasSelection()) {
      return false;
    }
    void copy(tidy(term.getSelection()), "Copied selection.");
    term.clearSelection();
    return true;
  }

  async function pasteFromClipboard() {
    const text = await terminalClipboardText();
    if (text) {
      callbacks.current.onPaste(text);
    }
  }

  function openSearch() {
    setSearchOpen(true);
    const selection = termRef.current?.getSelection();
    if (selection && !selection.includes("\n")) {
      setQuery(selection);
    }
    window.setTimeout(() => searchInput.current?.select(), 0);
  }

  useEffect(() => {
    const host = hostRef.current;
    if (!host) {
      return;
    }
    const term = new Terminal({
      fontFamily: '"Cascadia Mono", "Cascadia Code", Consolas, "Courier New", monospace',
      fontSize: settingsRef.current.fontSize,
      cursorBlink: true,
      cursorStyle: "bar",
      cursorWidth: 2,
      cursorInactiveStyle: "outline",
      scrollback: 10000,
      allowProposedApi: true,
      theme: themeOf(),
      rightClickSelectsWord: false,
    });
    const fit = new FitAddon();
    const search = new SearchAddon();
    term.loadAddon(fit);
    term.loadAddon(search);
    term.loadAddon(new WebLinksAddon((_event, uri) => void openUrl(uri)));
    term.open(host);
    termRef.current = term;
    fitRef.current = fit;
    searchRef.current = search;

    term.attachCustomKeyEventHandler((event) => {
      if (event.type !== "keydown") {
        return true;
      }
      const key = event.key.toLowerCase();
      const typed = event.key.length === 1 && !event.ctrlKey && !event.altKey && !event.metaKey;
      if (term.hasSelection() && (event.key === "Backspace" || event.key === "Delete" || typed)) {
        const keys = deleteSelectionKeys(term);
        if (keys) {
          event.preventDefault();
          term.clearSelection();
          void terminalWrite(session.id, typed ? keys + event.key : keys).catch(() => undefined);
          return false;
        }
      }
      if (event.ctrlKey && !event.altKey) {
        if (key === "c" && (event.shiftKey || term.hasSelection())) {
          copySelection();
          return false;
        }
        if (key === "v") {
          // Leave the browser's paste to the capture listener below so the text goes through review.
          return false;
        }
        if (key === "f") {
          event.preventDefault();
          openSearch();
          return false;
        }
        if (key === "=" || key === "+") {
          event.preventDefault();
          callbacks.current.onZoom(1);
          return false;
        }
        if (key === "-") {
          event.preventDefault();
          callbacks.current.onZoom(-1);
          return false;
        }
        if (key === "0") {
          event.preventDefault();
          callbacks.current.onZoom(0);
          return false;
        }
      }
      return true;
    });

    const onPasteEvent = (event: ClipboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      const text = event.clipboardData?.getData("text/plain") ?? "";
      if (text) {
        callbacks.current.onPaste(text);
      }
    };
    const onContextMenu = (event: MouseEvent) => {
      event.preventDefault();
      if (!copySelection()) {
        void pasteFromClipboard();
      }
    };
    const onWheel = (event: WheelEvent) => {
      if (event.ctrlKey) {
        event.preventDefault();
        callbacks.current.onZoom(event.deltaY < 0 ? 1 : -1);
      }
    };
    let pressed: { x: number; y: number } | null = null;
    const onMouseDown = (event: MouseEvent) => {
      pressed = event.button === 0 && !event.shiftKey && !event.altKey && !event.ctrlKey ? { x: event.clientX, y: event.clientY } : null;
    };
    const onMouseUp = (event: MouseEvent) => {
      const start = pressed;
      pressed = null;
      if (!start || Math.abs(event.clientX - start.x) > 3 || Math.abs(event.clientY - start.y) > 3) {
        return;
      }
      const moves = cursorMoves(term, host, event.clientX, event.clientY);
      if (moves) {
        void terminalWrite(session.id, moves).catch(() => undefined);
      }
    };
    host.addEventListener("mousedown", onMouseDown, true);
    host.addEventListener("mouseup", onMouseUp, true);
    host.addEventListener("paste", onPasteEvent, true);
    host.addEventListener("contextmenu", onContextMenu);
    host.addEventListener("wheel", onWheel, { passive: false });

    const data = term.onData((input) => void terminalWrite(session.id, input).catch(() => undefined));
    const resize = term.onResize(({ cols, rows }) => void terminalResize(session.id, cols, rows).catch(() => undefined));
    const selection = term.onSelectionChange(() => {
      if (settingsRef.current.copyOnSelect && term.hasSelection()) {
        void copy(tidy(term.getSelection()), "Copied selection.");
      }
    });
    const detach = attachTerminal(
      session.id,
      (output) => term.write(output),
      (code) => {
        term.write(`\r\n\x1b[90m[Process exited${code === null ? "" : ` with code ${code}`}]\x1b[0m\r\n`);
        callbacks.current.onExit(code);
      },
    );
    const observer = new ResizeObserver(() => {
      if (host.offsetWidth > 0 && host.offsetHeight > 0) {
        fit.fit();
      }
    });
    observer.observe(host);

    return () => {
      observer.disconnect();
      detach();
      data.dispose();
      resize.dispose();
      selection.dispose();
      host.removeEventListener("mousedown", onMouseDown, true);
      host.removeEventListener("mouseup", onMouseUp, true);
      host.removeEventListener("paste", onPasteEvent, true);
      host.removeEventListener("contextmenu", onContextMenu);
      host.removeEventListener("wheel", onWheel);
      term.dispose();
      termRef.current = null;
    };
    // The terminal is created once per session.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session.id]);

  useEffect(() => {
    const term = termRef.current;
    if (!term) {
      return;
    }
    term.options.fontSize = settings.fontSize;
    if (hostRef.current?.offsetWidth) {
      fitRef.current?.fit();
    }
  }, [settings.fontSize]);

  useEffect(() => {
    if (!active) {
      return;
    }
    const frame = requestAnimationFrame(() => {
      const term = termRef.current;
      if (!term) {
        return;
      }
      term.options.theme = themeOf();
      fitRef.current?.fit();
      term.focus();
    });
    return () => cancelAnimationFrame(frame);
  }, [active]);

  useImperativeHandle(ref, () => ({
    focus: () => termRef.current?.focus(),
    clear: () => termRef.current?.clear(),
    openSearch,
    copyAll: async () => {
      const term = termRef.current;
      return term ? copy(tidy(bufferLines(term).join("\n")), "Copied the whole terminal.") : false;
    },
    copyLastOutput: async () => {
      const term = termRef.current;
      if (!term) {
        return false;
      }
      const lines = bufferLines(term);
      const prompts = lines.flatMap((line, index) => (PROMPT.test(line) ? [index] : []));
      if (prompts.length === 0) {
        return false;
      }
      const lastPrompt = prompts[prompts.length - 1];
      const endsAtPrompt = lines.length - 1 === lastPrompt && lines[lastPrompt].replace(PROMPT, "").trim() === "";
      const start = endsAtPrompt ? prompts[prompts.length - 2] : lastPrompt;
      const end = endsAtPrompt ? lastPrompt : lines.length;
      if (start === undefined) {
        return false;
      }
      const output = tidy(lines.slice(start + 1, end).join("\n"));
      return copy(output, "Copied the last command's output.");
    },
  }));

  function find(next: boolean) {
    if (!query) {
      return;
    }
    const options = { decorations: { matchOverviewRuler: "#0284c7", activeMatchColorOverviewRuler: "#38bdf8" } };
    if (next) {
      searchRef.current?.findNext(query, options);
    } else {
      searchRef.current?.findPrevious(query, options);
    }
  }

  function closeSearch() {
    setSearchOpen(false);
    searchRef.current?.clearDecorations();
    termRef.current?.focus();
  }

  return (
    <div id={`terminal-view-${session.id}`} className={cn("relative min-h-0 flex-1", active ? "flex" : "hidden")}>
      <div id={`terminal-host-${session.id}`} ref={hostRef} className="min-h-0 flex-1 bg-zinc-950 p-2" />
      {searchOpen ? (
        <div
          id={`terminal-search-${session.id}`}
          className="absolute right-4 top-2 flex items-center gap-1 rounded-md border border-zinc-700 bg-zinc-900 p-1 shadow-lg"
        >
          <input
            id={`terminal-search-${session.id}-input`}
            ref={searchInput}
            className="h-7 w-48 rounded border border-zinc-700 bg-zinc-950 px-2 text-xs text-zinc-100"
            placeholder="Find"
            value={query}
            autoFocus
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                find(!event.shiftKey);
              } else if (event.key === "Escape") {
                closeSearch();
              }
            }}
          />
          <Button id={`terminal-search-${session.id}-prev`} variant="ghost" size="xs" aria-label="Previous match" onClick={() => find(false)}>
            <ChevronUp id={`terminal-search-${session.id}-prev-icon`} className="size-3.5" aria-hidden="true" />
          </Button>
          <Button id={`terminal-search-${session.id}-next`} variant="ghost" size="xs" aria-label="Next match" onClick={() => find(true)}>
            <ChevronDown id={`terminal-search-${session.id}-next-icon`} className="size-3.5" aria-hidden="true" />
          </Button>
          <Button id={`terminal-search-${session.id}-close`} variant="ghost" size="xs" aria-label="Close search" onClick={closeSearch}>
            <X id={`terminal-search-${session.id}-close-icon`} className="size-3.5" aria-hidden="true" />
          </Button>
        </div>
      ) : null}
    </div>
  );
}
