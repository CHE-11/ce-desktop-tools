import { VaultLock } from "../VaultLock";
import { Dialog } from "./Dialog";

type VaultLockDialogProps = {
  open: boolean;
  onClose: () => void;
  locked: boolean;
  needsPin: boolean;
  busy: boolean;
  focusNonce: number;
  onCreatePin: (pin: string) => Promise<void>;
  onUnlock: (pin: string) => Promise<void>;
  onLock: () => void | Promise<void>;
};

export function VaultLockDialog({ open, onClose, locked, needsPin, busy, focusNonce, onCreatePin, onUnlock, onLock }: VaultLockDialogProps) {
  return (
    <Dialog id="vault-lock-dialog" title="Unlock the vault" open={open} onClose={onClose} panelClassName="max-w-lg">
      <p id="vault-lock-dialog-description" className="text-sm text-zinc-400 mb-2">
        Enter the password to unlock the vault.
      </p>  
      <VaultLock
        locked={locked}
        needsPin={needsPin}
        busy={busy}
        focusNonce={focusNonce}
        compact
        pending={locked && !needsPin}
        onCreate={onCreatePin}
        onUnlock={onUnlock}
        onLock={onLock}
      />
    </Dialog>
  );
}
