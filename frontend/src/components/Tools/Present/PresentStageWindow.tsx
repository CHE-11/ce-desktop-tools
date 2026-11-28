import { useEffect } from "react";
import { usePresentState } from "./PresentPalette";
import { PresentStage } from "./PresentStage";

export function PresentStageWindow({ monitor }: { monitor: number }) {
  const [state] = usePresentState();

  useEffect(() => {
    document.title = `Present ${monitor + 1}`;
  }, [monitor]);

  const spotlightHere = state.spotlight !== null && state.spotlightMonitor === monitor;
  return (
    <PresentStage
      monitor={monitor}
      tool={state.tool}
      cursor={state.cursor}
      color={state.color}
      width={state.width}
      dim={state.dim}
      radius={state.radius}
      spread={state.spread}
      circle={state.circle}
      placing={state.placing}
      clearToken={state.clear}
      spotlight={spotlightHere ? state.spotlight : null}
      spotlightElsewhere={state.spotlight !== null && !spotlightHere}
    />
  );
}
