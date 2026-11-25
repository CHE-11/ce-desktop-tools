import { AppWindow, MonitorDown } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { DesktopApp } from "../../../../types/desktop-apps";
import { cn } from "../../lib/cn";
import { desktopAppAdd, desktopApps } from "../../lib/desktop-apps-client";
import { messageOf } from "../../lib/message";
import { Button } from "../Button";

function rowId(appId: string) {
  return appId.replace(/[^a-zA-Z0-9_-]/g, "-").slice(0, 80);
}

function detailLine(app: DesktopApp) {
  const kind = app.kind === "store" ? "Store" : "Desktop";
  return [app.publisher, app.version, kind].filter((part) => part && part.trim().length > 0).join(" · ");
}

export function AddToDesktop() {
  const [apps, setApps] = useState<DesktopApp[]>([]);
  const [filter, setFilter] = useState("");
  const [loaded, setLoaded] = useState(false);
  const [pending, setPending] = useState<string | null>(null);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);

  const load = useCallback(async () => {
    try {
      setApps(await desktopApps());
      setLoaded(true);
    } catch (caught) {
      setMessage({ text: messageOf(caught), error: true });
      setLoaded(true);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const rows = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    if (!needle) {
      return apps;
    }
    return apps.filter((app) =>
      [app.name, app.publisher, app.version, app.location, app.kind, app.appId]
        .filter((part) => part && part.length > 0)
        .join(" ")
        .toLowerCase()
        .includes(needle),
    );
  }, [apps, filter]);

  const add = useCallback(async (app: DesktopApp) => {
    if (app.onDesktop || pending) {
      return;
    }
    setPending(app.appId);
    setMessage(null);
    try {
      const path = await desktopAppAdd(app.name, app.appId);
      setApps((current) =>
        current.map((entry) => (entry.appId === app.appId ? { ...entry, onDesktop: true } : entry)),
      );
      setMessage({ text: `Shortcut saved to ${path}`, error: false });
    } catch (caught) {
      setMessage({ text: messageOf(caught), error: true });
    } finally {
      setPending(null);
    }
  }, [pending]);

  return (
    <section id="add-to-desktop" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="add-to-desktop-header" className="flex flex-col gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="add-to-desktop-heading" className="flex items-center gap-2">
          <MonitorDown id="add-to-desktop-title-icon" className="size-4 text-zinc-400" aria-hidden="true" />
          <h1 id="add-to-desktop-title" className="text-base font-semibold text-zinc-100">
            Add to Desktop
          </h1>
        </div>
        <p id="add-to-desktop-summary" className="text-sm text-zinc-400">
          Start menu and Store apps, with the publisher, version, and icon. Add puts a shortcut on the desktop.
        </p>
        <input
          id="add-to-desktop-search"
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
          aria-label="Search apps"
          placeholder="Search name or publisher"
          className="h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-3 text-sm text-zinc-100 outline-none focus:border-sky-500"
        />
      </header>
      {message ? (
        <p
          id="add-to-desktop-message"
          className={cn(
            "border-b px-4 py-2 text-sm",
            message.error ? "border-red-900 bg-red-950/40 text-red-300" : "border-zinc-800 text-emerald-400",
          )}
        >
          {message.text}
        </p>
      ) : null}
      <div id="add-to-desktop-scroll" className="min-h-0 flex-1 overflow-auto">
        {!loaded ? (
          <p id="add-to-desktop-loading" className="px-4 py-3 text-sm text-zinc-400">
            Reading Start menu apps…
          </p>
        ) : rows.length === 0 ? (
          <p id="add-to-desktop-empty" className="px-4 py-3 text-sm text-zinc-400">
            {apps.length === 0 ? "No Start menu apps were found." : "No apps match."}
          </p>
        ) : (
          <ul id="add-to-desktop-list" className="divide-y divide-zinc-800">
            {rows.map((app, index) => {
              const id = `${index}-${rowId(app.appId)}`;
              const busy = pending === app.appId;
              return (
                <li
                  id={`add-to-desktop-row-${id}`}
                  key={app.appId}
                  className="flex items-center gap-3 px-4 py-2.5"
                >
                  <span
                    id={`add-to-desktop-icon-wrap-${id}`}
                    className="flex size-8 shrink-0 items-center justify-center overflow-hidden rounded-md bg-zinc-900"
                  >
                    {app.icon ? (
                      <img
                        id={`add-to-desktop-icon-${id}`}
                        src={app.icon}
                        alt=""
                        className="size-8 object-contain"
                      />
                    ) : (
                      <AppWindow
                        id={`add-to-desktop-icon-${id}`}
                        className="size-4 text-zinc-500"
                        aria-hidden="true"
                      />
                    )}
                  </span>
                  <span id={`add-to-desktop-copy-${id}`} className="min-w-0 flex-1">
                    <span
                      id={`add-to-desktop-name-${id}`}
                      title={app.appId}
                      className="block truncate text-sm text-zinc-100"
                    >
                      {app.name}
                    </span>
                    <span id={`add-to-desktop-meta-${id}`} className="block truncate text-xs text-zinc-400">
                      {detailLine(app)}
                    </span>
                    {app.location ? (
                      <span
                        id={`add-to-desktop-location-${id}`}
                        title={app.location}
                        className="block truncate text-xs text-zinc-500"
                      >
                        {app.location}
                      </span>
                    ) : null}
                  </span>
                  <Button
                    id={`add-to-desktop-add-${id}`}
                    className="shrink-0"
                    size="sm"
                    variant={app.onDesktop ? "ghost" : "secondary"}
                    disabled={app.onDesktop || busy || pending !== null}
                    onClick={() => void add(app)}
                  >
                    {app.onDesktop ? "On desktop" : busy ? "Adding…" : "Add"}
                  </Button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </section>
  );
}
