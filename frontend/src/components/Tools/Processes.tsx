import { open } from "@tauri-apps/plugin-dialog";
import { ChevronDown, ChevronRight, RefreshCw } from "lucide-react";
import { Fragment, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { FileLocker, PortOwner, ProcessInfo } from "../../../../types/processes";
import { revealPath } from "../../lib/app-client";
import { cn } from "../../lib/cn";
import { messageOf, pickedFolderPath } from "../../lib/message";
import { fileLockers, formatBytes, portOwners, processIcons, processKill, processList } from "../../lib/processes-client";
import { Button } from "../Button";

type Tab = "processes" | "port" | "locks";
type SortKey = "name" | "pid" | "memory";

const TABS: { id: Tab; label: string }[] = [
  { id: "processes", label: "Processes" },
  { id: "port", label: "Port lookup" },
  { id: "locks", label: "File locks" },
];

const REFRESH_MS = 3000;
const CONFIRM_MS = 3000;
const INPUT = "h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100";
const CELL = "truncate py-1.5 pr-3";

/** Kill buttons ask once more before they close anything. */
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

export function Processes() {
  const [tab, setTab] = useState<Tab>("processes");
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);

  const report = useCallback((text: string, error = false) => setMessage({ text, error }), []);

  return (
    <section id="processes-tool" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header id="processes-header" className="flex flex-col gap-3 border-b border-zinc-800 px-4 py-3">
        <div id="processes-heading">
          <h1 id="processes-title" className="text-base font-semibold text-zinc-100">
            Processes
          </h1>
          <p id="processes-summary" className="mt-1 text-sm text-zinc-400">
            Close a stuck program, find what is using a port, or find what is holding a file open. Kill buttons ask
            you to click twice.
          </p>
        </div>
        <nav id="processes-tabs" role="tablist" aria-label="Process tools" className="flex gap-1">
          {TABS.map((entry) => (
            <button
              id={`processes-tab-${entry.id}`}
              key={entry.id}
              type="button"
              role="tab"
              aria-selected={tab === entry.id}
              onClick={() => {
                setTab(entry.id);
                setMessage(null);
              }}
              className={cn(
                "rounded-md px-3 py-1.5 text-sm",
                tab === entry.id ? "bg-zinc-800 font-medium text-zinc-100" : "text-zinc-400 hover:bg-zinc-800",
              )}
            >
              {entry.label}
            </button>
          ))}
        </nav>
      </header>
      {message ? (
        <p
          id="processes-message"
          className={cn(
            "border-b px-4 py-2 text-sm",
            message.error ? "border-red-900 bg-red-950/40 text-red-300" : "border-zinc-800 text-emerald-400",
          )}
        >
          {message.text}
        </p>
      ) : null}
      <div id="processes-body" className="flex min-h-0 flex-1 flex-col overflow-hidden px-4 py-3">
        {tab === "processes" ? <ProcessTable report={report} /> : null}
        {tab === "port" ? <PortLookup report={report} /> : null}
        {tab === "locks" ? <FileLocks report={report} /> : null}
      </div>
    </section>
  );
}

type PanelProps = {
  report: (text: string, error?: boolean) => void;
};

type ProcessGroup = {
  key: string;
  name: string;
  path: string;
  memoryBytes: number;
  processes: ProcessInfo[];
};

function groupDomId(key: string) {
  let hash = 2166136261;
  for (let index = 0; index < key.length; index += 1) {
    hash ^= key.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(36);
}

function compareProcesses(sort: SortKey, left: ProcessInfo, right: ProcessInfo) {
  if (sort === "name") {
    return left.name.localeCompare(right.name, undefined, { sensitivity: "base" }) || left.pid - right.pid;
  }
  if (sort === "pid") {
    return left.pid - right.pid;
  }
  return right.memoryBytes - left.memoryBytes || left.pid - right.pid;
}

function ProcessGlyph({ id, src }: { id: string; src: string | undefined }) {
  if (!src) {
    return <span id={id} className="size-4 shrink-0 rounded-sm bg-zinc-800" aria-hidden="true" />;
  }
  return <img id={id} src={src} alt="" className="size-4 shrink-0" />;
}

function ProcessTable({ report }: PanelProps) {
  const [processes, setProcesses] = useState<ProcessInfo[]>([]);
  const [icons, setIcons] = useState<Map<string, string>>(() => new Map());
  const [filter, setFilter] = useState("");
  const [sort, setSort] = useState<SortKey>("memory");
  const [grouped, setGrouped] = useState(true);
  const [openGroups, setOpenGroups] = useState<Set<string>>(() => new Set());
  const [live, setLive] = useState(true);
  const [loaded, setLoaded] = useState(false);
  const askedIcons = useRef(new Set<string>());
  const { armed, confirm } = useConfirm();

  const refresh = useCallback(async () => {
    try {
      setProcesses(await processList());
      setLoaded(true);
    } catch (caught) {
      report(messageOf(caught), true);
    }
  }, [report]);

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

  useEffect(() => {
    const missing = [
      ...new Set(
        processes
          .map((process) => process.path)
          .filter((path) => path && !askedIcons.current.has(path.toLowerCase())),
      ),
    ];
    if (missing.length === 0) {
      return;
    }
    for (const path of missing) {
      askedIcons.current.add(path.toLowerCase());
    }
    void processIcons(missing)
      .then((found) => {
        if (found.length === 0) {
          return;
        }
        setIcons((current) => {
          const next = new Map(current);
          for (const entry of found) {
            if (entry.icon) {
              next.set(entry.path.toLowerCase(), entry.icon);
            }
          }
          return next;
        });
      })
      .catch(() => {
        for (const path of missing) {
          askedIcons.current.delete(path.toLowerCase());
        }
      });
  }, [processes]);

  const groups = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    const matching = needle
      ? processes.filter(
          (process) =>
            process.name.toLowerCase().includes(needle) ||
            String(process.pid) === needle ||
            process.path.toLowerCase().includes(needle),
        )
      : processes;
    const buckets = new Map<string, ProcessInfo[]>();
    for (const process of matching) {
      const key = grouped && process.path ? process.path.toLowerCase() : `pid:${process.pid}`;
      const members = buckets.get(key);
      if (members) {
        members.push(process);
      } else {
        buckets.set(key, [process]);
      }
    }
    const built: ProcessGroup[] = [...buckets.entries()].map(([key, members]) => {
      const processesInGroup = [...members].sort((left, right) => compareProcesses(sort, left, right));
      const lead = processesInGroup.reduce((best, process) =>
        process.memoryBytes > best.memoryBytes ? process : best,
      );
      return {
        key,
        name: lead.name,
        path: lead.path,
        memoryBytes: processesInGroup.reduce((sum, process) => sum + process.memoryBytes, 0),
        processes: processesInGroup,
      };
    });
    built.sort((left, right) => {
      if (sort === "name") {
        return left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
      }
      if (sort === "pid") {
        return left.processes[0].pid - right.processes[0].pid;
      }
      return right.memoryBytes - left.memoryBytes;
    });
    return built;
  }, [processes, filter, sort, grouped]);

  const matchedCount = groups.reduce((sum, group) => sum + group.processes.length, 0);

  function toggleGroup(key: string) {
    setOpenGroups((current) => {
      const next = new Set(current);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }

  async function kill(process: ProcessInfo, tree: boolean) {
    try {
      report(`Closed ${await processKill(process.pid, tree)}.`);
      void refresh();
    } catch (caught) {
      report(messageOf(caught), true);
    }
  }

  function header(key: SortKey, label: string, className: string) {
    return (
      <th id={`processes-head-${key}`} className={cn("py-1.5 pr-3 font-medium", className)}>
        <button
          id={`processes-sort-${key}`}
          type="button"
          onClick={() => setSort(key)}
          className={cn("hover:text-zinc-200", sort === key && "text-zinc-200")}
        >
          {label}
          {sort === key ? " ▾" : ""}
        </button>
      </th>
    );
  }

  return (
    <div id="processes-list" className="flex min-h-0 flex-1 flex-col gap-3">
      <div id="processes-list-controls" className="flex flex-wrap items-center gap-2">
        <label id="processes-filter-label" htmlFor="processes-filter" className="sr-only">
          Filter processes
        </label>
        <input
          id="processes-filter"
          value={filter}
          placeholder="Filter by name, PID, or path"
          onChange={(event) => setFilter(event.target.value)}
          className={cn(INPUT, "w-72")}
        />
        <span id="processes-count" className="text-xs text-zinc-500">
          {loaded
            ? grouped
              ? `${groups.length} groups · ${matchedCount} of ${processes.length}`
              : `${matchedCount} of ${processes.length}`
            : "Loading"}
        </span>
        <label id="processes-group-label" className="ml-auto flex items-center gap-1.5 text-sm text-zinc-300">
          <input
            id="processes-group"
            type="checkbox"
            checked={grouped}
            onChange={(event) => setGrouped(event.target.checked)}
          />
          Group by path
        </label>
        <label id="processes-live-label" className="flex items-center gap-1.5 text-sm text-zinc-300">
          <input id="processes-live" type="checkbox" checked={live} onChange={(event) => setLive(event.target.checked)} />
          Refresh every 3 seconds
        </label>
        <Button id="processes-refresh" variant="outline" size="sm" onClick={() => void refresh()}>
          <RefreshCw id="processes-refresh-icon" className="size-3.5" aria-hidden="true" />
          Refresh
        </Button>
      </div>
      <div id="processes-table-wrap" className="min-h-0 flex-1 overflow-auto rounded-md border border-zinc-800">
        <table id="processes-table" className="w-full table-fixed text-left text-sm">
          <thead id="processes-table-head" className="sticky top-0 bg-zinc-900 text-xs text-zinc-500">
            <tr id="processes-table-head-row">
              {header("name", "Name", "w-64 pl-3")}
              {header("pid", "PID", "w-20")}
              {header("memory", "Memory", "w-24")}
              <th id="processes-head-path" className="py-1.5 pr-3 font-medium">
                Path
              </th>
              <th id="processes-head-actions" className="w-64 py-1.5 pr-3 font-medium">
                <span id="processes-head-actions-label" className="sr-only">
                  Actions
                </span>
              </th>
            </tr>
          </thead>
          <tbody id="processes-table-body">
            {groups.map((group) => {
              const many = grouped && group.processes.length > 1;
              const open = many && openGroups.has(group.key);
              const base = many ? `processes-group-${groupDomId(group.key)}` : `processes-row-${group.processes[0].pid}`;
              const icon = icons.get(group.path.toLowerCase());
              return (
                <Fragment key={group.key}>
                  <tr id={base} className="border-t border-zinc-800 hover:bg-zinc-900">
                    <td id={`${base}-name`} className={cn(CELL, "pl-3 text-zinc-100")}>
                      <span id={`${base}-name-row`} className="flex min-w-0 items-center gap-1.5">
                        {many ? (
                          <button
                            id={`${base}-toggle`}
                            type="button"
                            aria-expanded={open}
                            aria-label={open ? `Hide processes for ${group.name}` : `Show processes for ${group.name}`}
                            onClick={() => toggleGroup(group.key)}
                            className="rounded p-0.5 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
                          >
                            {open ? (
                              <ChevronDown id={`${base}-toggle-icon`} className="size-3.5" aria-hidden="true" />
                            ) : (
                              <ChevronRight id={`${base}-toggle-icon`} className="size-3.5" aria-hidden="true" />
                            )}
                          </button>
                        ) : grouped ? (
                          <span id={`${base}-toggle-spacer`} className="size-3.5 shrink-0" aria-hidden="true" />
                        ) : null}
                        <ProcessGlyph id={`${base}-icon`} src={icon} />
                        {many ? (
                          <button
                            id={`${base}-name-button`}
                            type="button"
                            title={group.name}
                            onClick={() => toggleGroup(group.key)}
                            className="min-w-0 truncate text-left hover:text-zinc-50"
                          >
                            {group.name}
                          </button>
                        ) : (
                          <span id={`${base}-label`} className="min-w-0 truncate" title={group.name}>
                            {group.name}
                          </span>
                        )}
                        {many ? (
                          <span
                            id={`${base}-count`}
                            className="shrink-0 rounded bg-zinc-800 px-1.5 text-xs text-zinc-400"
                            title={`${group.processes.length} processes`}
                          >
                            {group.processes.length}
                          </span>
                        ) : null}
                      </span>
                    </td>
                    <td id={`${base}-pid`} className={cn(CELL, "font-mono text-xs text-zinc-400")}>
                      {many ? "" : group.processes[0].pid}
                    </td>
                    <td id={`${base}-memory`} className={cn(CELL, "text-zinc-300 tabular-nums")}>
                      {group.memoryBytes > 0 ? formatBytes(group.memoryBytes) : ""}
                    </td>
                    <td id={`${base}-path`} className={cn(CELL, "text-xs text-zinc-500")} title={group.path}>
                      {group.path}
                    </td>
                    <td id={`${base}-actions`} className="py-1 pr-3">
                      <div id={`${base}-actions-row`} className="flex justify-end gap-1">
                        <Button
                          id={`${base}-reveal`}
                          variant="ghost"
                          size="xs"
                          disabled={!group.path}
                          onClick={() => void revealPath(group.path).catch((caught) => report(messageOf(caught), true))}
                        >
                          Show file
                        </Button>
                        {many ? null : (
                          <>
                            <Button
                              id={`${base}-kill-tree`}
                              variant={armed === `${group.processes[0].pid}-tree` ? "destructive" : "ghost"}
                              size="xs"
                              onClick={() =>
                                confirm(`${group.processes[0].pid}-tree`, () => void kill(group.processes[0], true))
                              }
                            >
                              {armed === `${group.processes[0].pid}-tree` ? "Confirm" : "Kill tree"}
                            </Button>
                            <Button
                              id={`${base}-kill`}
                              variant={armed === `${group.processes[0].pid}` ? "destructive" : "outline"}
                              size="xs"
                              onClick={() => confirm(`${group.processes[0].pid}`, () => void kill(group.processes[0], false))}
                            >
                              {armed === `${group.processes[0].pid}` ? "Confirm" : "Kill"}
                            </Button>
                          </>
                        )}
                      </div>
                    </td>
                  </tr>
                  {open
                    ? group.processes.map((process) => {
                        const child = `processes-row-${process.pid}`;
                        return (
                          <tr id={child} key={process.pid} className="border-t border-zinc-800/80 bg-zinc-950 hover:bg-zinc-900">
                            <td id={`${child}-name`} className={cn(CELL, "pl-3 text-zinc-300")}>
                              <span id={`${child}-name-row`} className="flex items-center gap-1.5 pl-5">
                                <ProcessGlyph id={`${child}-icon`} src={icons.get(process.path.toLowerCase())} />
                                <span id={`${child}-label`} className="truncate" title={process.name}>
                                  {process.name}
                                </span>
                              </span>
                            </td>
                            <td id={`${child}-pid`} className={cn(CELL, "font-mono text-xs text-zinc-400")}>
                              {process.pid}
                            </td>
                            <td id={`${child}-memory`} className={cn(CELL, "text-zinc-300 tabular-nums")}>
                              {process.memoryBytes > 0 ? formatBytes(process.memoryBytes) : ""}
                            </td>
                            <td id={`${child}-path`} className={cn(CELL, "text-xs text-zinc-500")} title={process.path}>
                              {process.path}
                            </td>
                            <td id={`${child}-actions`} className="py-1 pr-3">
                              <div id={`${child}-actions-row`} className="flex justify-end gap-1">
                                <Button
                                  id={`${child}-reveal`}
                                  variant="ghost"
                                  size="xs"
                                  disabled={!process.path}
                                  onClick={() =>
                                    void revealPath(process.path).catch((caught) => report(messageOf(caught), true))
                                  }
                                >
                                  Show file
                                </Button>
                                <Button
                                  id={`${child}-kill-tree`}
                                  variant={armed === `${process.pid}-tree` ? "destructive" : "ghost"}
                                  size="xs"
                                  onClick={() => confirm(`${process.pid}-tree`, () => void kill(process, true))}
                                >
                                  {armed === `${process.pid}-tree` ? "Confirm" : "Kill tree"}
                                </Button>
                                <Button
                                  id={`${child}-kill`}
                                  variant={armed === `${process.pid}` ? "destructive" : "outline"}
                                  size="xs"
                                  onClick={() => confirm(`${process.pid}`, () => void kill(process, false))}
                                >
                                  {armed === `${process.pid}` ? "Confirm" : "Kill"}
                                </Button>
                              </div>
                            </td>
                          </tr>
                        );
                      })
                    : null}
                </Fragment>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function PortLookup({ report }: PanelProps) {
  const [port, setPort] = useState("");
  const [searched, setSearched] = useState("");
  const [owners, setOwners] = useState<PortOwner[] | null>(null);
  const [busy, setBusy] = useState(false);
  const { armed, confirm } = useConfirm();

  async function lookUp(text = searched || port) {
    setBusy(true);
    try {
      setOwners(await portOwners(text));
      setSearched(text);
    } catch (caught) {
      setOwners(null);
      report(messageOf(caught), true);
    } finally {
      setBusy(false);
    }
  }

  const wildcard = searched.includes("*");

  async function kill(owner: PortOwner) {
    try {
      report(`Closed ${await processKill(owner.pid, false)}.`);
      void lookUp();
    } catch (caught) {
      report(messageOf(caught), true);
    }
  }

  return (
    <div id="processes-port" className="flex min-h-0 flex-1 flex-col gap-3">
      <form
        id="processes-port-form"
        className="flex items-center gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          void lookUp(port);
        }}
      >
        <label id="processes-port-label" htmlFor="processes-port-input" className="text-sm text-zinc-300">
          Port
        </label>
        <input
          id="processes-port-input"
          value={port}
          placeholder="3000 or 30**"
          onChange={(event) => setPort(event.target.value.replace(/[^\d*]/g, "").slice(0, 5))}
          className={cn(INPUT, "w-32 font-mono")}
        />
        <Button id="processes-port-submit" type="submit" variant="primary" size="sm" disabled={busy || !port}>
          Look up
        </Button>
        {owners && owners.length > 0 ? (
          <span id="processes-port-count" className="text-xs text-zinc-500">
            {owners.length} connection{owners.length === 1 ? "" : "s"}
          </span>
        ) : null}
      </form>
      {owners === null ? (
        <p id="processes-port-hint" className="text-sm text-zinc-500">
          Shows every TCP and UDP connection on a port, with the program that owns it. Use * for any one digit:
          30** finds ports 3000 to 3099, and 8*80 finds 8080, 8180, and so on.
        </p>
      ) : owners.length === 0 ? (
        <p id="processes-port-empty" className="text-sm text-zinc-400">
          Nothing is using {wildcard ? `a port matching ${searched}` : `port ${searched}`}.
        </p>
      ) : (
        <div id="processes-port-table-wrap" className="min-h-0 flex-1 overflow-auto rounded-md border border-zinc-800">
          <table id="processes-port-table" className="w-full table-fixed text-left text-sm">
            <thead id="processes-port-head" className="sticky top-0 bg-zinc-900 text-xs text-zinc-500">
              <tr id="processes-port-head-row">
                <th id="processes-port-head-process" className="w-56 py-1.5 pr-3 pl-3 font-medium">Process</th>
                <th id="processes-port-head-port" className="w-16 py-1.5 pr-3 font-medium">Port</th>
                <th id="processes-port-head-protocol" className="w-20 py-1.5 pr-3 font-medium">Protocol</th>
                <th id="processes-port-head-local" className="py-1.5 pr-3 font-medium">Local</th>
                <th id="processes-port-head-remote" className="py-1.5 pr-3 font-medium">Remote</th>
                <th id="processes-port-head-state" className="w-28 py-1.5 pr-3 font-medium">State</th>
                <th id="processes-port-head-actions" className="w-24 py-1.5 pr-3 font-medium">
                  <span id="processes-port-head-actions-label" className="sr-only">
                    Actions
                  </span>
                </th>
              </tr>
            </thead>
            <tbody id="processes-port-body">
              {owners.map((owner, index) => {
                const base = `processes-port-row-${index}`;
                const key = `port-${owner.pid}`;
                return (
                  <tr id={base} key={`${owner.protocol}-${owner.localAddress}-${owner.remoteAddress}-${owner.pid}`} className="border-t border-zinc-800">
                    <td id={`${base}-process`} className={cn(CELL, "pl-3 text-zinc-100")}>
                      {owner.name} <span id={`${base}-pid`} className="font-mono text-xs text-zinc-500">{owner.pid}</span>
                    </td>
                    <td id={`${base}-port`} className={cn(CELL, "font-mono text-xs text-zinc-200")}>
                      {owner.localPort}
                    </td>
                    <td id={`${base}-protocol`} className={cn(CELL, "text-zinc-400")}>
                      {owner.protocol}
                    </td>
                    <td id={`${base}-local`} className={cn(CELL, "font-mono text-xs text-zinc-300")}>
                      {owner.localAddress}
                    </td>
                    <td id={`${base}-remote`} className={cn(CELL, "font-mono text-xs text-zinc-500")}>
                      {owner.remoteAddress}
                    </td>
                    <td id={`${base}-state`} className={cn(CELL, owner.state === "LISTENING" ? "text-sky-400" : "text-zinc-400")}>
                      {owner.state}
                    </td>
                    <td id={`${base}-actions`} className="py-1 pr-3 text-right">
                      <Button
                        id={`${base}-kill`}
                        variant={armed === key ? "destructive" : "outline"}
                        size="xs"
                        onClick={() => confirm(key, () => void kill(owner))}
                      >
                        {armed === key ? "Confirm" : "Kill"}
                      </Button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function FileLocks({ report }: PanelProps) {
  const [path, setPath] = useState("");
  const [lockers, setLockers] = useState<FileLocker[] | null>(null);
  const [busy, setBusy] = useState(false);
  const { armed, confirm } = useConfirm();

  async function check(target = path) {
    if (!target.trim()) {
      report("Choose a file first.", true);
      return;
    }
    setBusy(true);
    try {
      setLockers(await fileLockers(target));
    } catch (caught) {
      report(messageOf(caught), true);
    } finally {
      setBusy(false);
    }
  }

  async function choose() {
    const picked = await open({ multiple: false, directory: false, title: "Choose the file that will not delete or move" });
    const next = pickedFolderPath(picked);
    if (next) {
      setPath(next);
      void check(next);
    }
  }

  async function kill(locker: FileLocker) {
    try {
      report(`Closed ${await processKill(locker.pid, false)}.`);
      void check();
    } catch (caught) {
      report(messageOf(caught), true);
    }
  }

  return (
    <div id="processes-locks" className="flex min-h-0 flex-1 flex-col gap-3">
      <form
        id="processes-locks-form"
        className="flex items-center gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          void check();
        }}
      >
        <label id="processes-locks-label" htmlFor="processes-locks-input" className="sr-only">
          File path
        </label>
        <input
          id="processes-locks-input"
          value={path}
          placeholder="C:\path\to\file.docx"
          onChange={(event) => setPath(event.target.value)}
          className={cn(INPUT, "min-w-0 flex-1")}
        />
        <Button id="processes-locks-browse" variant="outline" size="sm" onClick={() => void choose()}>
          Choose file
        </Button>
        <Button id="processes-locks-submit" type="submit" variant="primary" size="sm" disabled={busy || !path.trim()}>
          Check
        </Button>
      </form>
      {lockers === null ? (
        <p id="processes-locks-hint" className="text-sm text-zinc-500">
          When Windows says a file is in use, this shows which programs have it open.
        </p>
      ) : lockers.length === 0 ? (
        <p id="processes-locks-empty" className="text-sm text-zinc-400">
          No program has this file open.
        </p>
      ) : (
        <ul id="processes-locks-list" className="flex flex-col divide-y divide-zinc-800 rounded-md border border-zinc-800">
          {lockers.map((locker) => {
            const base = `processes-locks-row-${locker.pid}`;
            const key = `lock-${locker.pid}`;
            return (
              <li id={base} key={locker.pid} className="flex items-center gap-3 px-3 py-2">
                <span id={`${base}-text`} className="flex min-w-0 flex-1 flex-col">
                  <span id={`${base}-name`} className="truncate text-sm text-zinc-100">
                    {locker.appName || locker.name}{" "}
                    <span id={`${base}-pid`} className="font-mono text-xs text-zinc-500">
                      {locker.name} · {locker.pid}
                    </span>
                  </span>
                  <span id={`${base}-path`} className="truncate text-xs text-zinc-500" title={locker.path}>
                    {locker.path}
                  </span>
                </span>
                <Button
                  id={`${base}-kill`}
                  variant={armed === key ? "destructive" : "outline"}
                  size="xs"
                  onClick={() => confirm(key, () => void kill(locker))}
                >
                  {armed === key ? "Confirm" : "Kill"}
                </Button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
