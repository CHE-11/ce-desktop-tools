import { useEffect, useState } from "react";
import { messageOf } from "../../lib/message";
import { Button } from "../Button";

type VaultLockProps = {
  needsPin: boolean;
  busy: boolean;
  focusNonce: number;
  onCreate: (pin: string) => Promise<void>;
  onUnlock: (pin: string) => Promise<void>;
};

export function VaultLock({ needsPin, busy, focusNonce, onCreate, onUnlock }: VaultLockProps) {
  const [pin, setPin] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setPin("");
    setConfirm("");
    setError(null);
    document.getElementById("vault-pin")?.focus();
  }, [focusNonce, needsPin]);

  const submit = async () => {
    if (!/^\d{4,12}$/.test(pin)) {
      setError("Use a PIN of 4 to 12 digits.");
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

  return (
    <form
      id="vault-lock"
      className="mx-auto flex w-full max-w-sm flex-1 flex-col justify-center gap-3 px-4 py-8"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <h2 id="vault-lock-title" className="text-lg font-semibold text-zinc-100">
        Unlock text expansion vault
      </h2>
      <p id="vault-lock-copy" className="text-sm leading-5 text-zinc-400">
        {needsPin
          ? "Choose a PIN. It encrypts expansion.json in the settings folder. Expansion stays off until the vault is unlocked."
          : "Enter the PIN to unlock expansion. Snippets stay encrypted in expansion.json."}
      </p>
      <label id="vault-pin-label" htmlFor="vault-pin" className="text-xs font-medium text-zinc-400">
        PIN
      </label>
      <input
        id="vault-pin"
        value={pin}
        onChange={(event) => setPin(event.target.value.replace(/\D/g, "").slice(0, 12))}
        type="password"
        inputMode="numeric"
        autoComplete="off"
        disabled={busy}
        className="h-9 rounded-md border border-zinc-700 bg-zinc-950 px-3 text-sm tracking-widest text-zinc-100 outline-none focus:border-sky-500 disabled:opacity-50"
      />
      {needsPin ? (
        <>
          <label id="vault-pin-confirm-label" htmlFor="vault-pin-confirm" className="text-xs font-medium text-zinc-400">
            Confirm PIN
          </label>
          <input
            id="vault-pin-confirm"
            value={confirm}
            onChange={(event) => setConfirm(event.target.value.replace(/\D/g, "").slice(0, 12))}
            type="password"
            inputMode="numeric"
            autoComplete="off"
            disabled={busy}
            className="h-9 rounded-md border border-zinc-700 bg-zinc-950 px-3 text-sm tracking-widest text-zinc-100 outline-none focus:border-sky-500 disabled:opacity-50"
          />
        </>
      ) : null}
      {error ? (
        <p id="vault-lock-error" className="text-sm text-red-400">
          {error}
        </p>
      ) : null}
      <Button id="vault-lock-submit" variant="primary" type="submit" disabled={busy}>
        <span id="vault-lock-submit-label">{busy ? "Checking…" : needsPin ? "Create PIN" : "Unlock"}</span>
      </Button>
    </form>
  );
}
