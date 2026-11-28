import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import type { ClipCleaner as ClipCleanerSettings, ClipCleanerPreview, ClipRule } from "../../../../types/clipboard";
import { clipboardCleanerPreview, clipboardCleanerSave, clipboardCleanerState } from "../../lib/clipboard-client";
import { messageOf } from "../../lib/message";
import { useSettingsSection } from "../../lib/use-settings-section";
import { Button } from "../Button";
import { Switch } from "../Switch";

const FIELD = "h-8 min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2 font-mono text-sm text-zinc-100 outline-none focus:border-sky-500";

function settingsOf(settings: ClipCleanerSettings) {
  return settings;
}

function newRule(): ClipRule {
  return {
    id: crypto.randomUUID(),
    enabled: true,
    find: "",
    replace: "",
    caseInsensitive: false,
  };
}

function domId(id: string) {
  return id.replace(/[^A-Za-z0-9_-]/g, "");
}

export function ClipCleaner() {
  const { settings, error, setError, update } = useSettingsSection({
    load: clipboardCleanerState,
    save: clipboardCleanerSave,
    settingsOf,
  });
  const [sample, setSample] = useState("");
  const [preview, setPreview] = useState<ClipCleanerPreview | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);

  useEffect(() => {
    if (!settings) {
      return;
    }
    const text = sample;
    if (text.length === 0) {
      setPreview(null);
      setPreviewError(null);
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void clipboardCleanerPreview(text, { ...settings, enabled: true })
        .then((next) => {
          if (!cancelled) {
            setPreview(next);
            setPreviewError(null);
          }
        })
        .catch((caught) => {
          if (!cancelled) {
            setPreviewError(messageOf(caught));
          }
        });
    }, 200);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [sample, settings]);

  function changeRules(rules: ClipRule[], delay = 0) {
    update({ rules }, delay);
  }

  const rules = settings?.rules ?? [];
  const problem = error || previewError;

  return (
    <section id="clip-cleaner-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="clip-cleaner-header" className="flex flex-wrap items-center justify-between gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="clip-cleaner-heading" className="min-w-0">
          <h1 id="clip-cleaner-title" className="text-base font-semibold text-zinc-100">
            Clip Cleaner
          </h1>
          <p id="clip-cleaner-summary" className="mt-1 max-w-xl text-sm leading-5 text-zinc-400">
            Each new copy is rewritten before it is saved. Rules run from top to bottom. Use <span id="clip-cleaner-group-token" className="font-mono text-zinc-200">$1</span> for the first capture group and <span id="clip-cleaner-dollar-token" className="font-mono text-zinc-200">$$</span> for a dollar sign.
          </p>
        </div>
        <Switch
          id="clip-cleaner-enabled"
          label="Clean copies automatically"
          checked={settings?.enabled ?? false}
          disabled={!settings}
          onChange={(enabled) => update({ enabled })}
        />
      </header>
      {problem ? (
        <p id="clip-cleaner-error" className="border-b border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
          {problem}
        </p>
      ) : null}
      <div id="clip-cleaner-rules" className="min-h-0 flex-1 overflow-auto">
        <div id="clip-cleaner-columns" className="sticky top-0 z-10 flex items-center gap-2 border-b border-zinc-800 bg-zinc-900 px-3 py-2 text-xs font-medium text-zinc-400">
          <span id="clip-cleaner-col-enabled" className="w-14 shrink-0">
            Use
          </span>
          <span id="clip-cleaner-col-find" className="min-w-0 flex-1">
            Find
          </span>
          <span id="clip-cleaner-col-replace" className="min-w-0 flex-1">
            Replace
          </span>
          <span id="clip-cleaner-col-case" className="w-24 shrink-0">
            Ignore case
          </span>
          <span id="clip-cleaner-col-actions" className="w-24 shrink-0" />
        </div>
        {rules.length === 0 ? (
          <p id="clip-cleaner-empty" className="px-4 py-8 text-sm text-zinc-500">
            Add a rule to find and replace copied text.
          </p>
        ) : (
          rules.map((rule, index) => {
            const rowId = domId(rule.id) || `row-${index}`;
            return (
              <div id={`clip-cleaner-rule-${rowId}`} key={rule.id} className="flex items-center gap-2 border-b border-zinc-800 px-3 py-2">
                <div id={`clip-cleaner-rule-enabled-${rowId}`} className="flex w-14 shrink-0 justify-center">
                  <input
                    id={`clip-cleaner-rule-enabled-input-${rowId}`}
                    type="checkbox"
                    aria-label={`Use rule ${index + 1}`}
                    checked={rule.enabled}
                    onChange={(event) => {
                      const enabled = event.target.checked;
                      changeRules(rules.map((item) => (item.id === rule.id ? { ...item, enabled } : item)));
                    }}
                    className="size-4 accent-sky-500"
                  />
                </div>
                <input
                  id={`clip-cleaner-find-${rowId}`}
                  value={rule.find}
                  aria-label={`Find for rule ${index + 1}`}
                  placeholder="\\s+"
                  autoComplete="off"
                  spellCheck={false}
                  onChange={(event) => {
                    const find = event.target.value;
                    changeRules(
                      rules.map((item) => (item.id === rule.id ? { ...item, find } : item)),
                      300,
                    );
                  }}
                  className={FIELD}
                />
                <input
                  id={`clip-cleaner-replace-${rowId}`}
                  value={rule.replace}
                  aria-label={`Replace for rule ${index + 1}`}
                  placeholder=" "
                  autoComplete="off"
                  spellCheck={false}
                  onChange={(event) => {
                    const replace = event.target.value;
                    changeRules(
                      rules.map((item) => (item.id === rule.id ? { ...item, replace } : item)),
                      300,
                    );
                  }}
                  className={FIELD}
                />
                <label id={`clip-cleaner-case-${rowId}`} className="flex w-24 shrink-0 items-center justify-center gap-1.5 text-xs text-zinc-300">
                  <input
                    id={`clip-cleaner-case-input-${rowId}`}
                    type="checkbox"
                    checked={rule.caseInsensitive}
                    onChange={(event) => {
                      const caseInsensitive = event.target.checked;
                      changeRules(rules.map((item) => (item.id === rule.id ? { ...item, caseInsensitive } : item)));
                    }}
                    className="size-4 accent-sky-500"
                  />
                  <span id={`clip-cleaner-case-label-${rowId}`}>Aa</span>
                </label>
                <div id={`clip-cleaner-rule-actions-${rowId}`} className="flex w-24 shrink-0 items-center justify-end">
                  <button
                    id={`clip-cleaner-up-${rowId}`}
                    type="button"
                    title="Move up"
                    aria-label={`Move rule ${index + 1} up`}
                    disabled={index === 0}
                    onClick={() => {
                      const next = [...rules];
                      const [row] = next.splice(index, 1);
                      next.splice(index - 1, 0, row);
                      changeRules(next);
                    }}
                    className="inline-flex size-7 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 disabled:opacity-30"
                  >
                    <ArrowUp id={`clip-cleaner-up-icon-${rowId}`} className="size-3.5" aria-hidden="true" />
                  </button>
                  <button
                    id={`clip-cleaner-down-${rowId}`}
                    type="button"
                    title="Move down"
                    aria-label={`Move rule ${index + 1} down`}
                    disabled={index === rules.length - 1}
                    onClick={() => {
                      const next = [...rules];
                      const [row] = next.splice(index, 1);
                      next.splice(index + 1, 0, row);
                      changeRules(next);
                    }}
                    className="inline-flex size-7 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 disabled:opacity-30"
                  >
                    <ArrowDown id={`clip-cleaner-down-icon-${rowId}`} className="size-3.5" aria-hidden="true" />
                  </button>
                  <button
                    id={`clip-cleaner-delete-${rowId}`}
                    type="button"
                    title="Delete rule"
                    aria-label={`Delete rule ${index + 1}`}
                    onClick={() => changeRules(rules.filter((item) => item.id !== rule.id))}
                    className="inline-flex size-7 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-red-300"
                  >
                    <Trash2 id={`clip-cleaner-delete-icon-${rowId}`} className="size-3.5" aria-hidden="true" />
                  </button>
                </div>
              </div>
            );
          })
        )}
      </div>
      <div id="clip-cleaner-footer" className="flex items-center justify-between gap-3 border-t border-zinc-800 px-3 py-2">
        <Button id="clip-cleaner-add" variant="outline" size="sm" disabled={!settings || rules.length >= 40} onClick={() => changeRules([...rules, newRule()])}>
          <Plus id="clip-cleaner-add-icon" className="size-3.5" aria-hidden="true" />
          <span id="clip-cleaner-add-label">Add rule</span>
        </Button>
        <p id="clip-cleaner-count" className="text-xs text-zinc-500 tabular-nums">
          {rules.length} {rules.length === 1 ? "rule" : "rules"}
          {settings?.enabled ? "" : " · cleaning is off"}
        </p>
      </div>
      <div id="clip-cleaner-try" className="grid gap-2 border-t border-zinc-800 px-4 py-3">
        <label id="clip-cleaner-sample-label" htmlFor="clip-cleaner-sample" className="text-xs font-medium text-zinc-400">
          Try a copy
        </label>
        <textarea
          id="clip-cleaner-sample"
          value={sample}
          rows={2}
          placeholder="Paste text to see what the rules would do"
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => {
            setSample(event.target.value);
            setError(null);
          }}
          className="min-h-16 w-full resize-y rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 font-mono text-sm text-zinc-100 outline-none focus:border-sky-500"
        />
        <p id="clip-cleaner-preview-label" className="text-xs font-medium text-zinc-400">
          Result
        </p>
        <pre id="clip-cleaner-preview" className="max-h-28 min-h-10 overflow-auto whitespace-pre-wrap rounded-md border border-zinc-800 bg-zinc-950 px-2 py-1.5 font-mono text-sm text-zinc-200">
          {sample.length === 0 ? "" : (preview?.text ?? "")}
        </pre>
        {preview?.error ? (
          <p id="clip-cleaner-preview-error" className="text-xs text-amber-300">
            {preview.error}
          </p>
        ) : null}
      </div>
    </section>
  );
}
