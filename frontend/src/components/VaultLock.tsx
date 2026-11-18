import { Lock } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { messageOf } from "../lib/message";
import { cn } from "../lib/cn";
import { Button } from "./Button";

type VaultLockProps = {
  locked: boolean;
  needsPin: boolean;
  busy: boolean;
  focusNonce: number;
  compact?: boolean;
  pending?: boolean;
  onCreate: (pin: string) => Promise<void>;
  onUnlock: (pin: string) => Promise<void>;
  onLock: () => void | Promise<void>;
};

export function VaultLock({ locked, needsPin, busy, focusNonce, compact = false, onCreate, onUnlock, onLock }: VaultLockProps) {
  const [pin, setPin] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);
  const skipFocus = useRef(compact);
  const showPin = locked || needsPin;

  useEffect(() => {
    setPin("");
    setConfirm("");
    setError(null);
    if (skipFocus.current || !showPin) {
      skipFocus.current = false;
      return;
    }
    document.getElementById("vault-pin")?.focus();
  }, [focusNonce, needsPin, showPin]);

  const submit = async () => {
    if (pin.length < 4 || pin.length > 64 || /[\u0000-\u001f]/.test(pin)) {
      setError("Use a PIN of 4 to 64 characters.");
      return;
    }
    if (needsPin && pin !== confirm) {
      setError("Those PINs do not match.");
      return;
    }
    setError(null);
    try {
      if (needsPin) {
        await onCreate(pin);
      } else {
        await onUnlock(pin);
      }
      setPin("");
      setConfirm("");
    } catch (caught) {
      setError(messageOf(caught));
    }
  };

  const copy = compact
    ? needsPin
      ? "Choose a PIN to lock hidden snippets. Snippets that aren't hidden stay available while this is locked."
      : "Hidden snippets stay locked. The others still show and expand."
    : needsPin
      ? "Choose a PIN. Letters, numbers, and symbols are allowed. This stays locked until the vault is unlocked. Any other locked tool uses this same PIN."
      : "Enter the PIN to unlock. Any other locked tool uses this same PIN.";

  if (!showPin) {
    return (
      <div
        id="vault-lock"
        className={cn(
          compact ? "flex items-center" : "mx-auto flex w-full max-w-sm flex-1 flex-col justify-center gap-3 px-4 py-8",
        )}
      >
        {compact ? null : (
          <>
            <h2 id="vault-lock-title" className="text-lg font-semibold text-zinc-100">
              Vault unlocked
            </h2>
            <p id="vault-lock-copy" className="text-sm leading-5 text-zinc-400">
              Lock the vault to hide protected snippets and clipboard history. Any other unlocked tool uses this same lock.
            </p>
          </>
        )}
        <Button id="vault-lock-button" variant="outline" size={compact ? "sm" : "default"} disabled={busy} title="Lock the vault" onClick={() => void onLock()}>
          <Lock id="vault-lock-button-icon" className="size-4" aria-hidden="true" />
          <span id="vault-lock-button-label">{busy ? "Locking…" : "Lock"}</span>
        </Button>
      </div>
    );
  }

  return (
    <form
      id="vault-lock"
      className={cn(
        compact
          ? "flex flex-col gap-2"
          : "mx-auto flex w-full max-w-sm flex-1 flex-col justify-center gap-3 px-4 py-8",
      )}
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      {compact ? null : (
        <>
          <h2 id="vault-lock-title" className="text-lg font-semibold text-zinc-100">
            Unlock text expansion vault
          </h2>
          <p id="vault-lock-copy" className="text-sm leading-5 text-zinc-400">
            {copy}
          </p>
        </>
      )}
      {compact ? (
        <p id="vault-lock-copy" className="sr-only">
          {copy}
        </p>
      ) : null}
      <div id="vault-lock-fields" className={compact ? "flex flex-wrap items-end gap-2" : "flex flex-col gap-3"}>
        <div id="vault-pin-field" className={compact ? "min-w-40 flex-1" : "flex flex-col gap-3"}>
          <input
            id="vault-pin"
            value={pin}
            onChange={(event) => setPin(event.target.value.slice(0, 64))}
            type="password"
            autoComplete="off"
            spellCheck={false}
            disabled={busy}
            className="h-7 w-full rounded-md border border-zinc-700 bg-zinc-950 px-3 text-sm tracking-widest text-zinc-100 outline-none focus:border-sky-500 disabled:opacity-50"
          />
        </div>
        {needsPin ? (
          <div id="vault-pin-confirm-field" className={compact ? "min-w-40 flex-1" : "flex flex-col gap-3"}>
            <label id="vault-pin-confirm-label" htmlFor="vault-pin-confirm" className="text-xs font-medium text-zinc-400">
              Confirm PIN
            </label>
            <input
              id="vault-pin-confirm"
              value={confirm}
              onChange={(event) => setConfirm(event.target.value.slice(0, 64))}
              type="password"
              autoComplete="off"
              spellCheck={false}
              disabled={busy}
              className="mt-1 h-9 w-full rounded-md border border-zinc-700 bg-zinc-950 px-3 text-sm tracking-widest text-zinc-100 outline-none focus:border-sky-500 disabled:opacity-50"
            />
          </div>
        ) : null}
        {compact ? (
          <Button id="vault-lock-submit" className="h-7" variant="primary" type="submit" disabled={busy}>
            <span id="vault-lock-submit-label">{busy ? "Checking…" : needsPin ? "Create PIN" : "Unlock"}</span>
          </Button>
        ) : null}
      </div>
      {error ? (
        <p id="vault-lock-error" className="text-sm text-red-400">
          {error}
        </p>
      ) : null}
      {compact ? null : (
        <Button id="vault-lock-submit" variant="primary" type="submit" disabled={busy}>
          <span id="vault-lock-submit-label">{busy ? "Checking…" : needsPin ? "Create PIN" : "Unlock"}</span>
        </Button>
      )}
    </form>
  );
}
