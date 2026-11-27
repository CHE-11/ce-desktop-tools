import { useCallback, useEffect, useState } from "react";
import { megaclipRunning, runMegaClip, stopMegaClip } from "../../../lib/megaclip-client";
import { messageOf } from "../../../lib/message";
import { Button } from "../../Button";
import { Pause, Play } from "lucide-react";

export function RunMegaClip() {
  const [running, setRunning] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const next = await megaclipRunning();
    setRunning(next);
    return next;
  }, []);

  useEffect(() => {
    let current = true;
    const look = () => {
      void megaclipRunning()
        .then((next) => {
          if (current) {
            setRunning(next);
          }
        })
        .catch(() => undefined);
    };
    look();
    const id = window.setInterval(look, 1500);
    return () => {
      current = false;
      window.clearInterval(id);
    };
  }, []);

  const toggle = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const active = running ?? (await refresh());
      if (active) {
        await stopMegaClip();
      } else {
        await runMegaClip();
      }
      await refresh();
    } catch (caught) {
      setError(messageOf(caught));
      void refresh().catch(() => undefined);
    } finally {
      setBusy(false);
    }
  }, [refresh, running]);

  const label = busy ? (running ? "Stopping…" : "Starting…") : running ? "Stop MegaClip" : "Run MegaClip";

  return (
    <>
      {error ? (
        <p id="app-action-megaclip-error" className="text-xs leading-4 text-red-300">
          {error}
        </p>
      ) : null}
      <Button
        id="app-action-megaclip"
        variant="outline"
        className="w-full"
        disabled={busy || running === null}
        title={running ? "Stop MegaClip.ahk" : "Start MegaClip.ahk"}
        onClick={() => void toggle()}
      >
        {running ? <Pause size={16} /> : <Play size={16} /> }
        <span id="app-action-megaclip-label">{running === null ? "MegaClip" : label}</span>
      </Button>
    </>
  );
}
