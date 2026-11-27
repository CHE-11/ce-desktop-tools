import type { ReactNode } from "react";
import { cn } from "../lib/cn";

type SettingRowProps = {
  id: string;
  title: string;
  description?: string;
  nested?: boolean;
  children?: ReactNode;
};

/** One settings line: a title and description on the left, a control on the right. */
export function SettingRow({ id, title, description, nested, children }: SettingRowProps) {
  return (
    <div id={id} className={cn("flex flex-wrap items-center justify-between gap-x-6 gap-y-2 px-3 py-2.5", nested && "pl-8")}>
      <div id={`${id}-text`} className="min-w-0 flex-1">
        <p id={`${id}-title`} className="text-sm text-zinc-100">
          {title}
        </p>
        {description ? (
          <p id={`${id}-description`} className="mt-0.5 text-xs text-zinc-400">
            {description}
          </p>
        ) : null}
      </div>
      {children ? (
        <div id={`${id}-control`} className="flex shrink-0 items-center gap-2">
          {children}
        </div>
      ) : null}
    </div>
  );
}

type SettingGroupProps = {
  id: string;
  title: string;
  children: ReactNode;
};

/** A heading over a bordered list of rows. */
export function SettingGroup({ id, title, children }: SettingGroupProps) {
  return (
    <section id={id} className="flex flex-col gap-2">
      <h2 id={`${id}-title`} className="text-sm font-medium text-zinc-100">
        {title}
      </h2>
      <div id={`${id}-rows`} className="flex flex-col divide-y divide-zinc-800 rounded-md border border-zinc-800 bg-zinc-900/40">
        {children}
      </div>
    </section>
  );
}

export const SETTING_SELECT = "h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 disabled:opacity-40";
export const SETTING_INPUT = "h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 disabled:opacity-40";
export const SETTING_RANGE = "w-44 accent-sky-500 disabled:opacity-40";
