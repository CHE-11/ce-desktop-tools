import { Pipette } from "lucide-react";
import { useCallback, useState } from "react";
import { colorPickerPick } from "../../../lib/color-picker-client";
import { messageOf } from "../../../lib/message";
import { Button } from "../../Button";

export function ColorPickerAction() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const pick = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await colorPickerPick();
    } catch (caught) {
      setError(messageOf(caught));
    } finally {
      setBusy(false);
    }
  }, []);

  return (
    <>
      {error ? (
        <p id="app-action-color-picker-error" className="text-xs leading-4 text-red-300">
          {error}
        </p>
      ) : null}
      <Button
        id="app-action-color-picker"
        variant="outline"
        className="w-full"
        disabled={busy}
        title="Pick a color from the screen"
        onClick={() => void pick()}
      >
        <Pipette id="app-action-color-picker-icon" className="size-4 shrink-0" aria-hidden="true" />
        <span id="app-action-color-picker-label">{busy ? "Picking…" : "Color Picker"}</span>
      </Button>
    </>
  );
}
