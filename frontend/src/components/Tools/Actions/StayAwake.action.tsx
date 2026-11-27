import { listen } from "@tauri-apps/api/event";
import { Coffee } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { messageOf } from "../../../lib/message";
import { setStayAwake, stayAwakeState } from "../../../lib/stay-awake-client";
import { Button } from "../../Button";

export function StayAwake() {
  const [on, setOn] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void stayAwakeState()
      .then(setOn)
      .catch(() => undefined);
    const subscription = listen<boolean>("stay-awake", (event) => setOn(event.payload));
    return () => {
      void subscription.then((stop) => stop());
    };
  }, []);

  const toggle = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setOn(await setStayAwake(!on));
    } catch (caught) {
      setError(messageOf(caught));
    } finally {
      setBusy(false);
    }
  }, [on]);

  return (
    <>
      {error ? (
        <p id="app-action-stay-awake-error" className="text-xs leading-4 text-red-300">
          {error}
        </p>
      ) : null}
      <Button
        id="app-action-stay-awake"
        variant={on ? "primary" : "outline"}
        className="w-full"
        aria-pressed={on}
        disabled={busy}
        title={on ? "The computer and display stay awake until you turn this off." : "Keep the computer and display awake."}
        onClick={() => void toggle()}
      >
        <Coffee id="app-action-stay-awake-icon" className="size-4 shrink-0" aria-hidden="true" />
        <span id="app-action-stay-awake-label">{on ? "Staying awake" : "Stay awake"}</span>
      </Button>
    </>
  );
}
