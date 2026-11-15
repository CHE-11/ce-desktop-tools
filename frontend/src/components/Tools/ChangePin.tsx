import { useState } from "react";
import { changeVaultPin } from "../../lib/expansion-client";
import { messageOf } from "../../lib/message";
import { Button } from "../Button";

function pinError(pin: string) {
  const length = Array.from(pin).length;
  if (length < 4 || length > 64 || /[\u0000-\u001f]/.test(pin)) {
    return "Use a PIN of 4 to 64 characters.";
  }
  return null;
}

export function ChangePin({ busy }: { busy: boolean }) {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [working, setWorking] = useState(false);

  const submit = async () => {
    const currentProblem = pinError(current);
    const nextProblem = pinError(next);
    if (currentProblem || nextProblem) {
      setError(currentProblem ?? nextProblem);
      setSaved(false);
      return;
    }
    if (next !== confirm) {
      setError("Those new PINs do not match.");
      setSaved(false);
      return;
    }
    if (current === next) {
      setError("Choose a different PIN.");
      setSaved(false);
      return;
    }

    setWorking(true);
    setError(null);
    setSaved(false);
    try {
      await changeVaultPin(current, next);
      setCurrent("");
      setNext("");
      setConfirm("");
      setSaved(true);
    } catch (caught) {
      setError(messageOf(caught));
    } finally {
      setWorking(false);
    }
  };

  return (
    <section id="change-pin" className="rounded-lg border border-zinc-800 bg-zinc-950/60 px-3 py-3">
      <h3 id="change-pin-heading" className="text-[11px] font-semibold tracking-wide text-zinc-400 uppercase">
        Vault PIN
      </h3>
      <p id="change-pin-copy" className="mt-1 mb-3 text-xs leading-5 text-zinc-500">
        Text expansion uses this PIN. Any other locked tool uses the same one. Enter the current PIN before choosing a new one. Letters, numbers, and symbols are allowed.
      </p>
      <form
        id="change-pin-form"
        className="grid max-w-sm gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <label id="change-pin-current-label" htmlFor="change-pin-current" className="text-xs font-medium text-zinc-400">
          Current PIN
        </label>
        <input
          id="change-pin-current"
          value={current}
          onChange={(event) => setCurrent(event.target.value.slice(0, 64))}
          type="password"
          autoComplete="off"
          spellCheck={false}
          disabled={busy || working}
          className="h-9 rounded-md border border-zinc-700 bg-zinc-950 px-3 text-sm text-zinc-100 outline-none focus:border-sky-500 disabled:opacity-50"
        />
        <label id="change-pin-next-label" htmlFor="change-pin-next" className="text-xs font-medium text-zinc-400">
          New PIN
        </label>
        <input
          id="change-pin-next"
          value={next}
          onChange={(event) => setNext(event.target.value.slice(0, 64))}
          type="password"
          autoComplete="off"
          spellCheck={false}
          disabled={busy || working}
          className="h-9 rounded-md border border-zinc-700 bg-zinc-950 px-3 text-sm text-zinc-100 outline-none focus:border-sky-500 disabled:opacity-50"
        />
        <label id="change-pin-confirm-label" htmlFor="change-pin-confirm" className="text-xs font-medium text-zinc-400">
          Confirm new PIN
        </label>
        <input
          id="change-pin-confirm"
          value={confirm}
          onChange={(event) => setConfirm(event.target.value.slice(0, 64))}
          type="password"
          autoComplete="off"
          spellCheck={false}
          disabled={busy || working}
          className="h-9 rounded-md border border-zinc-700 bg-zinc-950 px-3 text-sm text-zinc-100 outline-none focus:border-sky-500 disabled:opacity-50"
        />
        {saved ? (
          <p id="change-pin-saved" className="text-sm text-emerald-300">
            PIN changed.
          </p>
        ) : null}
        {error ? (
          <p id="change-pin-error" className="text-sm text-red-400">
            {error}
          </p>
        ) : null}
        <Button id="change-pin-submit" variant="primary" type="submit" disabled={busy || working} className="mt-1 justify-self-start">
          <span id="change-pin-submit-label">{working ? "Changing…" : "Change PIN"}</span>
        </Button>
      </form>
    </section>
  );
}
