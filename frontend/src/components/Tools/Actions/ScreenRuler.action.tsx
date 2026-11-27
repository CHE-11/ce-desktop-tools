import { RulerDimensionLine } from "lucide-react";
import { useCallback, useState } from "react";
import { messageOf } from "../../../lib/message";
import { screenRulerOpen } from "../../../lib/screen-ruler-client";
import { Button } from "../../Button";

export function ScreenRulerAction() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const open = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await screenRulerOpen();
    } catch (caught) {
      setError(messageOf(caught));
    } finally {
      setBusy(false);
    }
  }, []);

  return (
    <>
      {error ? (
        <p id="app-action-screen-ruler-error" className="text-xs leading-4 text-red-300">
          {error}
        </p>
      ) : null}
      <Button
        id="app-action-screen-ruler"
        variant="outline"
        className="w-full"
        disabled={busy}
        title="Open the screen ruler"
        onClick={() => void open()}
      >
        <RulerDimensionLine id="app-action-screen-ruler-icon" className="size-4 shrink-0" aria-hidden="true" />
        <span id="app-action-screen-ruler-label">{busy ? "Opening…" : "Screen Ruler"}</span>
      </Button>
    </>
  );
}
