import { useMemo, useState } from "react";
import { copyText } from "../../lib/ascii-client";
import { Button } from "../Button";
import { Dialog } from "./Dialog";

type Variable = {
  id: string;
  tokens: string[];
  meaning: string;
  example: (when: Date) => string;
};

const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS = ["", "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

const DATES: Variable[] = [
  { id: "yyyy", tokens: ["{yyyy}", "{YYYY}"], meaning: "Year", example: (when) => String(when.getFullYear()) },
  { id: "yy", tokens: ["{yy}", "{YY}"], meaning: "Year, two digits", example: (when) => pad(when.getFullYear() % 100) },
  { id: "mmmm", tokens: ["{MMMM}"], meaning: "Month name", example: (when) => MONTHS[when.getMonth() + 1] ?? "" },
  { id: "mmm", tokens: ["{MMM}"], meaning: "Month, three letters", example: (when) => (MONTHS[when.getMonth() + 1] ?? "").slice(0, 3) },
  { id: "month", tokens: ["{MM}"], meaning: "Month, 01–12", example: (when) => pad(when.getMonth() + 1) },
  { id: "month-short", tokens: ["{M}"], meaning: "Month, 1–12", example: (when) => String(when.getMonth() + 1) },
  { id: "dddd", tokens: ["{dddd}", "{DDDD}"], meaning: "Weekday", example: (when) => WEEKDAYS[when.getDay()] ?? "" },
  { id: "ddd", tokens: ["{ddd}", "{DDD}"], meaning: "Weekday, three letters", example: (when) => (WEEKDAYS[when.getDay()] ?? "").slice(0, 3) },
  { id: "day", tokens: ["{dd}", "{DD}"], meaning: "Day, 01–31", example: (when) => pad(when.getDate()) },
  { id: "day-short", tokens: ["{d}", "{D}"], meaning: "Day, 1–31", example: (when) => String(when.getDate()) },
  { id: "hour24", tokens: ["{HH}"], meaning: "Hour, 00–23", example: (when) => pad(when.getHours()) },
  { id: "hour24-short", tokens: ["{H}"], meaning: "Hour, 0–23", example: (when) => String(when.getHours()) },
  { id: "hour12", tokens: ["{hh}"], meaning: "Hour, 01–12", example: (when) => pad(hour12(when.getHours())) },
  { id: "hour12-short", tokens: ["{h}"], meaning: "Hour, 1–12", example: (when) => String(hour12(when.getHours())) },
  { id: "minute", tokens: ["{mm}"], meaning: "Minute, 00–59", example: (when) => pad(when.getMinutes()) },
  { id: "minute-short", tokens: ["{m}"], meaning: "Minute, without a leading zero", example: (when) => String(when.getMinutes()) },
  { id: "second", tokens: ["{ss}"], meaning: "Second, 00–59", example: (when) => pad(when.getSeconds()) },
  { id: "second-short", tokens: ["{s}"], meaning: "Second, without a leading zero", example: (when) => String(when.getSeconds()) },
  { id: "ampm", tokens: ["{tt}"], meaning: "AM or PM", example: (when) => (when.getHours() < 12 ? "AM" : "PM") },
  { id: "ampm-short", tokens: ["{t}"], meaning: "A or P", example: (when) => (when.getHours() < 12 ? "A" : "P") },
  {
    id: "longdate",
    tokens: ["{longdate}"],
    meaning: "Windows long date",
    example: (when) => `${WEEKDAYS[when.getDay()]}, ${MONTHS[when.getMonth() + 1]} ${when.getDate()}, ${when.getFullYear()}`,
  },
];

const OTHER: Variable[] = [
  { id: "clipboard", tokens: ["{clipboard}"], meaning: "Text on the Windows clipboard", example: () => "clipboard text" },
  { id: "cursor", tokens: ["{cursor}"], meaning: "Where the caret stays after expanding", example: () => "caret" },
  { id: "enter", tokens: ["{enter}", "{newline}"], meaning: "New line", example: () => "new line" },
  { id: "tab", tokens: ["{tab}"], meaning: "Tab", example: () => "tab" },
  { id: "user", tokens: ["{user}", "{username}"], meaning: "Windows user name", example: () => "your user name" },
  { id: "computer", tokens: ["{computer}", "{hostname}"], meaning: "Computer name", example: () => "this PC" },
  { id: "uuid", tokens: ["{uuid}", "{guid}"], meaning: "A new unique id each time it expands", example: () => "new id" },
  { id: "env", tokens: ["{env:NAME}"], meaning: "Value of the environment variable NAME", example: () => "value of NAME" },
];

export function ExpansionOptionsDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [copied, setCopied] = useState<string | null>(null);
  const now = useMemo(() => new Date(), [open]);

  return (
    <Dialog id="expansion-options" title="Expansion variables" open={open} onClose={onClose} panelClassName="max-w-xl">
      <p id="expansion-options-intro" className="text-sm leading-5 text-zinc-400">
        Put a token in the replacement text. It is filled in when the trigger expands. {`{MM}`} is the month and {`{mm}`} is the minute. You can combine date tokens with punctuation, as in {`{MM}/{DD}/{YYYY}`}.
      </p>
      <h3 id="expansion-options-dates-title" className="mt-4 text-xs font-medium tracking-wide text-zinc-500 uppercase">
        Dates
      </h3>
      <VariableTable id="expansion-options-dates" rows={DATES} now={now} exampleLabel="Now" onCopy={setCopied} />
      <h3 id="expansion-options-other-title" className="mt-4 text-xs font-medium tracking-wide text-zinc-500 uppercase">
        Other
      </h3>
      <VariableTable id="expansion-options-other" rows={OTHER} now={now} exampleLabel="Example" onCopy={setCopied} />
      <div id="expansion-options-footer" className="mt-3 flex items-center justify-between gap-3">
        <p id="expansion-options-copied" className="text-xs text-zinc-500">
          {copied ? `Copied ${copied}` : "Click a token to copy it."}
        </p>
        <Button id="expansion-options-close" variant="ghost" size="sm" onClick={onClose}>
          <span id="expansion-options-close-label">Close</span>
        </Button>
      </div>
    </Dialog>
  );
}

function VariableTable({
  id,
  rows,
  now,
  exampleLabel,
  onCopy,
}: {
  id: string;
  rows: Variable[];
  now: Date;
  exampleLabel: string;
  onCopy: (token: string) => void;
}) {
  return (
    <table id={`${id}-table`} className="mt-2 w-full border-collapse text-left text-sm">
      <thead id={`${id}-head`}>
        <tr id={`${id}-columns`} className="border-b border-zinc-800 text-xs text-zinc-500">
          <th id={`${id}-col-token`} className="py-2 pr-3 font-medium">
            Token
          </th>
          <th id={`${id}-col-meaning`} className="py-2 pr-3 font-medium">
            Meaning
          </th>
          <th id={`${id}-col-example`} className="py-2 font-medium">
            {exampleLabel}
          </th>
        </tr>
      </thead>
      <tbody id={`${id}-body`}>
        {rows.map((variable) => (
          <tr id={`${id}-row-${variable.id}`} key={variable.id} className="border-b border-zinc-800">
            <td id={`${id}-tokens-${variable.id}`} className="py-1.5 pr-3">
              <div id={`${id}-token-row-${variable.id}`} className="flex flex-wrap gap-1">
                {variable.tokens.map((token) => {
                  const tokenId = token.replace(/[^A-Za-z0-9]/g, "");
                  return (
                    <button
                      id={`${id}-copy-${variable.id}-${tokenId}`}
                      key={token}
                      type="button"
                      title={`Copy ${token}`}
                      onClick={() => {
                        void copyText(token)
                          .then(() => onCopy(token))
                          .catch(() => onCopy(""));
                      }}
                      className="rounded px-1 font-mono text-xs text-sky-300 hover:bg-zinc-800"
                    >
                      {token}
                    </button>
                  );
                })}
              </div>
            </td>
            <td id={`${id}-meaning-${variable.id}`} className="py-1.5 pr-3 text-zinc-300">
              {variable.meaning}
            </td>
            <td id={`${id}-example-${variable.id}`} className="py-1.5 font-mono text-xs text-zinc-400">
              {variable.example(now)}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function pad(value: number) {
  return value.toString().padStart(2, "0");
}

function hour12(hour: number) {
  const value = hour % 12;
  return value === 0 ? 12 : value;
}
