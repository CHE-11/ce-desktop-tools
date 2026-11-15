import { listen } from "@tauri-apps/api/event";
import { Settings } from "lucide-react";
import { useEffect, useState } from "react";
import type { PantherbarAction, PantherbarShow } from "../../../../types/pantherbar";
import { pantherbarFit, pantherbarRun } from "../../lib/pantherbar-client";
import { messageOf } from "../../lib/message";
import { PantherbarIcon } from "./PantherbarIcon";

export function PantherbarBar() {
  const [actions, setActions] = useState<PantherbarAction[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    document.title = "Pantherbar";
    const subscriptions = [
      listen<PantherbarShow>("pantherbar-show", (event) => {
        setActions(event.payload.actions);
        setMessage(null);
        setOpen(true);
      }),
      listen("pantherbar-hide", () => {
        setOpen(false);
        setMessage(null);
      }),
    ];
    return () => {
      void Promise.all(subscriptions).then((stops) => {
        for (const stop of stops) {
          stop();
        }
      });
    };
  }, []);

  useEffect(() => {
    if (!open) {
      return;
    }
    const shell = document.getElementById("pantherbar-shell");
    if (!shell || shell.offsetWidth < 1 || shell.offsetHeight < 1) {
      return;
    }
    void pantherbarFit(shell.offsetWidth, shell.offsetHeight);
  }, [open, actions, message]);

  async function run(id: string) {
    try {
      setMessage(await pantherbarRun(id));
    } catch (caught) {
      setMessage(messageOf(caught));
    }
  }

  return (
    <div id="pantherbar-shell" className={open ? "inline-flex flex-col" : "hidden"}>
      <div id="pantherbar-bar" className="flex items-center gap-0.5 rounded-lg border border-zinc-700 bg-zinc-900 p-1 shadow-lg">
        {actions.map((action) => (
          <button
            id={`pantherbar-action-${action.id}`}
            key={action.id}
            type="button"
            title={action.name}
            onClick={() => void run(action.id)}
            className="flex size-8 items-center justify-center rounded-md text-zinc-100 hover:bg-zinc-800"
          >
            <PantherbarIcon id={`pantherbar-action-${action.id}-icon`} name={action.icon} className="size-4" />
          </button>
        ))}
        <button
          id="pantherbar-manage"
          type="button"
          title="Pantherbar settings"
          onClick={() => void run("manage")}
          className="flex size-8 items-center justify-center rounded-md text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
        >
          <Settings id="pantherbar-manage-icon" className="size-4" aria-hidden="true" />
        </button>
      </div>
      {message ? (
        <p id="pantherbar-result" className="mt-1 rounded-md border border-zinc-700 bg-zinc-900 px-2 py-1 text-xs text-zinc-200 shadow-lg">
          {message}
        </p>
      ) : null}
    </div>
  );
}
