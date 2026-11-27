import { useEffect, useRef, useState } from "react";
import type { ColorFormat, FormatToken } from "../../../../types/color-picker";
import { colorPickerRender } from "../../lib/color-picker-client";
import { Button } from "../Button";
import { Dialog } from "./Dialog";

const SAMPLE = "#ffe4b5";

type ColorFormatDialogProps = {
  open: boolean;
  /** The custom format being edited, or null to add one. */
  format: ColorFormat | null;
  takenNames: string[];
  tokens: FormatToken[];
  onClose: () => void;
  onSave: (format: ColorFormat) => void;
};

export function ColorFormatDialog({ open, format, takenNames, tokens, onClose, onSave }: ColorFormatDialogProps) {
  const [name, setName] = useState("");
  const [template, setTemplate] = useState("");
  const [preview, setPreview] = useState("");
  const templateRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setName(format?.name ?? "");
      setTemplate(format?.template ?? "rgba(%Re, %Gr, %Bl, 1)");
    }
  }, [open, format]);

  useEffect(() => {
    if (!open) {
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void colorPickerRender(template, SAMPLE)
        .then((value) => {
          if (!cancelled) {
            setPreview(value);
          }
        })
        .catch(() => undefined);
    }, 120);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [open, template]);

  const trimmed = name.trim();
  const clash = takenNames.some(
    (taken) => taken.toLowerCase() === trimmed.toLowerCase() && taken.toLowerCase() !== format?.name.toLowerCase(),
  );
  const problem = !trimmed
    ? "Give the format a name."
    : clash
      ? "Another format already has this name."
      : !template.trim()
        ? "Write a template."
        : null;

  function insert(token: string) {
    const field = templateRef.current;
    const start = field?.selectionStart ?? template.length;
    const end = field?.selectionEnd ?? template.length;
    const next = template.slice(0, start) + token + template.slice(end);
    setTemplate(next);
    window.requestAnimationFrame(() => {
      field?.focus();
      field?.setSelectionRange(start + token.length, start + token.length);
    });
  }

  function save() {
    if (problem) {
      return;
    }
    onSave({ name: trimmed, template, visible: format?.visible ?? true, builtIn: false });
  }

  return (
    <Dialog
      id="color-format-dialog"
      title={format ? "Edit color format" : "Add new format"}
      open={open}
      onClose={onClose}
      onEnter={save}
      headerActions={
        <>
          <Button id="color-format-dialog-cancel" variant="ghost" size="sm" onClick={onClose}>
            Cancel
          </Button>
          <Button id="color-format-dialog-save" variant="primary" size="sm" disabled={problem !== null} onClick={save}>
            Save
          </Button>
        </>
      }
    >
      <div id="color-format-dialog-form" className="flex flex-col gap-3">
        <label id="color-format-dialog-name-label" htmlFor="color-format-dialog-name" className="flex flex-col gap-1 text-xs text-zinc-400">
          Name
          <input
            id="color-format-dialog-name"
            value={name}
            onChange={(event) => setName(event.target.value)}
            className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100"
          />
        </label>
        <label
          id="color-format-dialog-template-label"
          htmlFor="color-format-dialog-template"
          className="flex flex-col gap-1 text-xs text-zinc-400"
        >
          Template
          <input
            id="color-format-dialog-template"
            ref={templateRef}
            value={template}
            onChange={(event) => setTemplate(event.target.value)}
            className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 font-mono text-sm text-zinc-100"
          />
        </label>
        <div id="color-format-dialog-preview" className="flex items-center gap-2 text-sm">
          <span id="color-format-dialog-preview-swatch" className="size-5 rounded border border-zinc-600" style={{ backgroundColor: SAMPLE }} />
          <span id="color-format-dialog-preview-label" className="text-zinc-400">
            Preview
          </span>
          <code id="color-format-dialog-preview-value" className="font-mono text-sky-400">
            {preview}
          </code>
        </div>
        {problem ? (
          <p id="color-format-dialog-problem" className="text-xs text-amber-300">
            {problem}
          </p>
        ) : null}
        <div id="color-format-dialog-tokens" className="flex flex-col gap-1.5">
          <p id="color-format-dialog-tokens-title" className="text-xs text-zinc-400">
            Click a token to insert it at the cursor.
          </p>
          <div id="color-format-dialog-token-list" className="grid grid-cols-2 gap-1 sm:grid-cols-3">
            {tokens.map((token) => (
              <button
                id={`color-format-dialog-token-${token.token.slice(1)}`}
                key={token.token}
                type="button"
                onClick={() => insert(token.token)}
                className="flex items-center gap-2 rounded-md px-2 py-1 text-left text-xs hover:bg-zinc-800"
              >
                <code id={`color-format-dialog-token-${token.token.slice(1)}-code`} className="w-12 shrink-0 font-mono text-sky-400">
                  {token.token}
                </code>
                <span id={`color-format-dialog-token-${token.token.slice(1)}-text`} className="truncate text-zinc-300">
                  {token.description}
                </span>
              </button>
            ))}
          </div>
        </div>
      </div>
    </Dialog>
  );
}
