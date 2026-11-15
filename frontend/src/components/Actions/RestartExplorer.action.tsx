import { useCallback, useState } from "react";
import { restartExplorer } from "../../lib/explorer-client";
import { messageOf } from "../../lib/message";
import { Button } from "../Button";

export function RestartExplorer() {
  const [restarting, setRestarting] = useState(false);
  const [confirmRestart, setConfirmRestart] = useState(false);
  const [restartError, setRestartError] = useState<string | null>(null);

  const restart = useCallback(async () => {
    setRestarting(true);
    setRestartError(null);
    try {
      await restartExplorer();
      setConfirmRestart(false);
    } catch (caught) {
      setRestartError(messageOf(caught));
    } finally {
      setRestarting(false);
    }
  }, []);

  return (
    <>
      {restartError ? (
        <p id="app-action-explorer-error" className="text-xs leading-4 text-red-300">
          {restartError}
        </p>
      ) : null}
      {confirmRestart ? (
        <>
          <Button id="app-action-explorer-now" variant="destructive" className="w-full" disabled={restarting} onClick={() => void restart()}>
            <span id="app-action-explorer-now-label">{restarting ? "Restarting…" : "Restart now"}</span>
          </Button>
          <Button id="app-action-explorer-cancel" variant="outline" className="w-full" disabled={restarting} onClick={() => setConfirmRestart(false)}>
            <span id="app-action-explorer-cancel-label">Cancel</span>
          </Button>
        </>
      ) : (
        <Button id="app-action-explorer" variant="outline" className="w-full" onClick={() => setConfirmRestart(true)}>
          <span id="app-action-explorer-label">Restart Explorer</span>
        </Button>
      )}
    </>
  );
}
