import { ChevronRight, Globe, RefreshCw, Search } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AppCall, AppCallConnection, AppCallsSnapshot } from "../../../../types/app-calls";
import { appCalls } from "../../lib/app-calls-client";
import { openUrl } from "../../lib/app-client";
import { cn } from "../../lib/cn";
import { messageOf } from "../../lib/message";
import { formatBytes, processKill } from "../../lib/processes-client";
import { Button } from "../Button";

const REFRESH_MS = 2000;
const CONFIRM_MS = 3000;
const WEB_PORTS = new Set([80, 443, 8080, 8443]);
const INPUT = "h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100";

function processQuestion(name: string) {
  return `What is the windows 11 process: ${name}?`;
}

function cpuLabel(percent: number | null) {
  if (percent == null) {
    return "— CPU";
  }
  return `${percent.toFixed(1)}% CPU`;
}

export function AppCalls() {
  const [snapshot, setSnapshot] = useState<AppCallsSnapshot | null>(null);
  const [filter, setFilter] = useState("");
  const [live, setLive] = useState(true);
  const [showLocal, setShowLocal] = useState(true);
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setSnapshot(await appCalls());
      setError(null);
    } catch (caught) {
      setError(messageOf(caught));
    }
  }, []);

  useEffect(() => {
    void refresh();
    if (!live) {
      return;
    }
    const timer = window.setInterval(() => {
      if (document.visibilityState === "visible") {
        void refresh();
      }
    }, REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [live, refresh]);

  const needle = filter.trim().toLowerCase();
  const foregroundMatched = useMemo(() => matching(snapshot, "foreground", needle), [snapshot, needle]);
  const backgroundMatched = useMemo(() => matching(snapshot, "background", needle), [snapshot, needle]);
  const foreground = useMemo(() => visibleApps(foregroundMatched, showLocal), [foregroundMatched, showLocal]);
  const background = useMemo(() => visibleApps(backgroundMatched, showLocal), [backgroundMatched, showLocal]);

  const killProcess = useCallback(
    async (pid: number) => {
      try {
        await processKill(pid, false);
        setError(null);
        await refresh();
      } catch (caught) {
        setError(messageOf(caught));
      }
    },
    [refresh],
  );

  const searchProcess = useCallback(async (name: string) => {
    const query = processQuestion(name);
    try {
      await openUrl(`https://www.duckduckgo.com/search?q=${encodeURIComponent(query)}`);
      setError(null);
    } catch (caught) {
      setError(messageOf(caught));
    }
  }, []);

  const toggleApp = useCallback((key: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }, []);
  return (
    <section id="app-calls-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="app-calls-header" className="flex flex-col gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="app-calls-heading" className="flex items-center gap-2">
          <Globe id="app-calls-title-icon" className="size-4 text-zinc-400" aria-hidden="true" />
          <h1 id="app-calls-title" className="text-base font-semibold text-zinc-100">
            Network Explorer
          </h1>
        </div>
        <p id="app-calls-summary" className="text-sm text-zinc-400">
          Live outbound TCP connections for every process. This is the address and port, not the request URL.
        </p>
        <div id="app-calls-controls" className="flex flex-wrap items-center gap-2">
          <label id="app-calls-filter-label" htmlFor="app-calls-filter" className="sr-only">
            Filter processes and connections
          </label>
          <input
            id="app-calls-filter"
            value={filter}
            placeholder="Filter by process, title, PID, or address"
            onChange={(event) => setFilter(event.target.value)}
            className={cn(INPUT, "w-80")}
          />
          <span id="app-calls-count" className="text-xs text-zinc-500">
            {snapshot
              ? `${foreground.length + background.length} processes`
              : error
                ? "Couldn't load"
                : "Loading"}
          </span>
          <label id="app-calls-local-label" className="ml-auto flex items-center gap-1.5 text-sm text-zinc-300">
            <input
              id="app-calls-local"
              type="checkbox"
              checked={showLocal}
              onChange={(event) => setShowLocal(event.target.checked)}
            />
            Show local connections
          </label>
          <label id="app-calls-live-label" className="flex items-center gap-1.5 text-sm text-zinc-300">
            <input
              id="app-calls-live"
              type="checkbox"
              checked={live}
              onChange={(event) => setLive(event.target.checked)}
            />
            Refresh every 2 seconds
          </label>
          <Button id="app-calls-refresh" variant="outline" size="sm" onClick={() => void refresh()}>
            <RefreshCw id="app-calls-refresh-icon" className="size-3.5" aria-hidden="true" />
            Refresh
          </Button>
        </div>
      </header>
      {error ? (
        <p id="app-calls-message" className="border-b border-red-900 bg-red-950/40 px-4 py-2 text-sm text-red-300">
          {error}
        </p>
      ) : null}
      <div id="app-calls-body" className="flex min-h-0 flex-1 flex-col gap-6 overflow-auto px-4 py-3">
        <CallSection
          id="foreground"
          title="Foreground"
          apps={foreground}
          expanded={expanded}
          onToggle={toggleApp}
          onKill={killProcess}
          onSearch={searchProcess}
          empty={sectionEmpty(needle, showLocal, foregroundMatched.length, foreground.length, "The foreground process has no outbound connection.")}
        />
        <CallSection
          id="background"
          title="Background"
          apps={background}
          expanded={expanded}
          onToggle={toggleApp}
          onKill={killProcess}
          onSearch={searchProcess}
          empty={sectionEmpty(needle, showLocal, backgroundMatched.length, background.length, "No other process has an outbound connection.")}
        />
      </div>
    </section>
  );
}

function sectionEmpty(needle: string, showLocal: boolean, matched: number, visible: number, idle: string) {
  if (visible > 0) {
    return idle;
  }
  if (!showLocal && matched > 0) {
    return "Local connections are hidden.";
  }
  if (needle && matched === 0) {
    return "Nothing in this list matches the filter.";
  }
  return idle;
}

function visibleApps(apps: AppCall[], showLocal: boolean) {
  if (showLocal) {
    return apps;
  }
  return apps.flatMap((app) => {
    const connections = app.connections.filter((connection) => !connection.local);
    return connections.length > 0 ? [{ ...app, connections }] : [];
  });
}

function matching(snapshot: AppCallsSnapshot | null, placement: AppCall["placement"], needle: string) {
  const apps = snapshot?.apps.filter((app) => app.placement === placement) ?? [];
  if (!needle) {
    return apps;
  }
  return apps.flatMap((app) => {
    if (appMatches(app, needle)) {
      return [app];
    }
    const connections = app.connections.filter((connection) => connectionMatches(connection, needle));
    return connections.length > 0 ? [{ ...app, connections }] : [];
  });
}

function appMatches(app: AppCall, needle: string) {
  return (
    app.name.toLowerCase().includes(needle) ||
    app.path.toLowerCase().includes(needle) ||
    app.windowTitle.toLowerCase().includes(needle) ||
    String(app.pid) === needle
  );
}

function remoteLabel(connection: AppCallConnection) {
  const address = connection.remoteAddress.includes(":")
    ? `[${connection.remoteAddress}]`
    : connection.remoteAddress;
  return `${address}:${connection.remotePort}`;
}

function connectionMatches(connection: AppCallConnection, needle: string) {
  return (
    connection.remoteAddress.toLowerCase().includes(needle) ||
    remoteLabel(connection).toLowerCase().includes(needle) ||
    connection.localAddress.toLowerCase().includes(needle) ||
    connection.protocol.toLowerCase().includes(needle) ||
    connection.state.toLowerCase().includes(needle) ||
    String(connection.remotePort) === needle
  );
}

function useConfirm() {
  const [armed, setArmed] = useState<string | null>(null);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  return {
    armed,
    confirm(key: string, action: () => void) {
      if (armed === key) {
        window.clearTimeout(timer.current);
        setArmed(null);
        action();
        return;
      }
      setArmed(key);
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => setArmed(null), CONFIRM_MS);
    },
  };
}

function CallSection({
  id,
  title,
  apps,
  expanded,
  onToggle,
  onKill,
  onSearch,
  empty,
}: {
  id: string;
  title: string;
  apps: AppCall[];
  expanded: Set<string>;
  onToggle: (key: string) => void;
  onKill: (pid: number) => void;
  onSearch: (name: string) => void;
  empty: string;
}) {
  return (
    <section id={`app-calls-${id}`} className="flex flex-col gap-2">
      <h2 id={`app-calls-${id}-title`} className="text-sm font-medium text-zinc-200">
        {title}
      </h2>
      {apps.length === 0 ? (
        <p id={`app-calls-${id}-empty`} className="text-sm text-zinc-500">
          {empty}
        </p>
      ) : (
        <ul id={`app-calls-${id}-list`} className="flex flex-col gap-2">
          {apps.map((app) => {
            const key = `${id}-${app.pid}`;
            return (
              <AppRow
                key={key}
                section={id}
                app={app}
                open={expanded.has(key)}
                onToggle={() => onToggle(key)}
                onKill={() => onKill(app.pid)}
                onSearch={() => onSearch(app.name)}
              />
            );
          })}
        </ul>
      )}
    </section>
  );
}

function AppRow({
  section,
  app,
  open,
  onToggle,
  onKill,
  onSearch,
}: {
  section: string;
  app: AppCall;
  open: boolean;
  onToggle: () => void;
  onKill: () => void;
  onSearch: () => void;
}) {
  const row = `${section}-${app.pid}`;
  const question = processQuestion(app.name);
  const { armed, confirm } = useConfirm();
  const killKey = `kill-${row}`;
  return (
    <li id={`app-calls-app-${row}`} className="rounded-md border border-zinc-800 bg-zinc-950/40">
      <div id={`app-calls-app-bar-${row}`} className="flex items-center gap-2 pr-2">
        <button
          id={`app-calls-app-head-${row}`}
          type="button"
          aria-expanded={open}
          aria-controls={`app-calls-connections-${row}`}
          onClick={onToggle}
          className="flex min-w-0 flex-1 flex-wrap items-center gap-x-3 gap-y-1 px-3 py-2 text-left hover:bg-zinc-900"
        >
          <ChevronRight
            id={`app-calls-app-chevron-${row}`}
            className={cn("size-3.5 shrink-0 text-zinc-500 transition-transform", open && "rotate-90")}
            aria-hidden="true"
          />
          <span id={`app-calls-app-name-${row}`} className="text-sm font-medium text-zinc-100">
            {app.name}
          </span>
          <span id={`app-calls-app-pid-${row}`} className="text-xs text-zinc-500">
            PID {app.pid}
          </span>
          <span id={`app-calls-app-cpu-${row}`} className="text-xs tabular-nums text-zinc-300">
            {cpuLabel(app.cpuPercent)}
          </span>
          <span id={`app-calls-app-memory-${row}`} className="text-xs tabular-nums text-zinc-300">
            {formatBytes(app.memoryBytes)}
          </span>
          {app.windowTitle ? (
            <span id={`app-calls-app-title-${row}`} className="min-w-0 flex-1 truncate text-xs text-zinc-400">
              {app.windowTitle}
            </span>
          ) : (
            <span id={`app-calls-app-title-${row}`} className="min-w-0 flex-1" />
          )}
          <span id={`app-calls-app-count-${row}`} className="text-xs text-zinc-500">
            {app.connections.length} {app.connections.length === 1 ? "connection" : "connections"}
          </span>
        </button>
        <Button id={`app-calls-search-${row}`} variant="ghost" size="xs" title={question} onClick={onSearch}>
          <Search id={`app-calls-search-icon-${row}`} className="size-3.5" aria-hidden="true" />
          Search
        </Button>
        <Button
          id={`app-calls-kill-${row}`}
          variant={armed === killKey ? "destructive" : "outline"}
          size="xs"
          onClick={() => confirm(killKey, onKill)}
        >
          {armed === killKey ? "Confirm" : "Kill"}
        </Button>
      </div>
      {open ? (
        <ul id={`app-calls-connections-${row}`} className="border-t border-zinc-800">
          {app.connections.map((connection, index) => (
            <ConnectionRow
              key={`${connection.localAddress}-${connection.remoteAddress}-${connection.remotePort}-${index}`}
              row={row}
              index={index}
              connection={connection}
            />
          ))}
        </ul>
      ) : null}
    </li>
  );
}

function ConnectionRow({
  row,
  index,
  connection,
}: {
  row: string;
  index: number;
  connection: AppCallConnection;
}) {
  const id = `${row}-${index}`;
  const web = WEB_PORTS.has(connection.remotePort);
  return (
    <li id={`app-calls-connection-${id}`} className="flex flex-wrap items-center gap-x-3 gap-y-1 px-3 py-1.5 text-sm">
      <span id={`app-calls-protocol-${id}`} className="w-14 text-xs text-zinc-500">
        {connection.protocol}
      </span>
      <span id={`app-calls-remote-${id}`} className="min-w-0 font-mono text-xs text-zinc-200">
        {remoteLabel(connection)}
      </span>
      <span id={`app-calls-state-${id}`} className="text-xs text-zinc-400">
        {connection.state}
      </span>
      {web ? (
        <span id={`app-calls-web-${id}`} className="rounded bg-sky-950 px-1.5 py-0.5 text-[10px] font-medium text-sky-300">
          Web
        </span>
      ) : null}
      {connection.local ? (
        <span id={`app-calls-local-${id}`} className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] font-medium text-zinc-300">
          Local
        </span>
      ) : null}
      <span id={`app-calls-local-address-${id}`} className="ml-auto truncate font-mono text-xs text-zinc-600">
        {connection.localAddress}
      </span>
    </li>
  );
}
