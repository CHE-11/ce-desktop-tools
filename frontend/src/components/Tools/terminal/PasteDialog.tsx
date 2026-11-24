import { useEffect, useState } from "react";
import type { ShellKind } from "../../../../../types/terminal";
import { Button } from "../../Button";
import { hasSmartQuotes, normalizePaste, pasteLines, type PasteMode } from "./paste";

type Props = {
  text: string;
  shell: ShellKind;
  onCancel: () => void;
  onRun: (text: string, mode: PasteMode) => void;
};

export function PasteDialog({ text, shell, onCancel, onRun }: Props) {
  const [draft, setDraft] = useState(text);
  const [strip, setStrip] = useState(() => hasSmartQuotes(text));

  useEffect(() => {
    setDraft(text);
    setStrip(hasSmartQuotes(text));
  }, [text]);

  const prepared = normalizePaste(draft, strip);
  const count = pasteLines(prepared).length;
  const blockHint =
    shell === "cmd" ? "Joins the lines with & and runs them as one command." : "Wraps the lines in & { } so they run as one script block.";

  return (
    <div
      id="terminal-paste-overlay"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6"
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          onCancel();
        }
      }}
    >
      <div
        id="terminal-paste-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="terminal-paste-title"
        className="flex max-h-full w-full max-w-2xl flex-col gap-3 rounded-lg border border-zinc-700 bg-zinc-900 p-4 shadow-xl"
      >
        <h2 id="terminal-paste-title" className="text-sm font-semibold text-zinc-100">
          Paste {count} lines
        </h2>
        <p id="terminal-paste-copy" className="text-xs text-zinc-400">
          Check or edit the text before it runs.
        </p>
        <textarea
          id="terminal-paste-text"
          className="min-h-48 flex-1 resize-y rounded-md border border-zinc-700 bg-zinc-950 p-2 font-mono text-xs text-zinc-100"
          spellCheck={false}
          autoFocus
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
        />
        <label id="terminal-paste-strip-label" className="flex items-center gap-2 text-xs text-zinc-300">
          <input
            id="terminal-paste-strip"
            type="checkbox"
            checked={strip}
            onChange={(event) => setStrip(event.target.checked)}
          />
          Replace smart quotes, long dashes, and non-breaking spaces
        </label>
        <div id="terminal-paste-actions" className="flex flex-wrap items-center justify-end gap-2">
          <Button id="terminal-paste-cancel" variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button id="terminal-paste-block" variant="secondary" title={blockHint} disabled={count === 0} onClick={() => onRun(prepared, "block")}>
            Run as one block
          </Button>
          <Button id="terminal-paste-lines" variant="primary" disabled={count === 0} onClick={() => onRun(prepared, "lines")}>
            Run line by line
          </Button>
        </div>
      </div>
    </div>
  );
}
