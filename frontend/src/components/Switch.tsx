import { cn } from "../lib/cn";

type SwitchProps = {
  id: string;
  checked: boolean;
  label: string;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
};

/** An on/off switch with the state written beside it. */
export function Switch({ id, checked, label, disabled, onChange }: SwitchProps) {
  return (
    <span id={`${id}-wrap`} className="inline-flex shrink-0 items-center gap-2">
      <span id={`${id}-state`} className="w-6 text-right text-xs text-zinc-400">
        {checked ? "On" : "Off"}
      </span>
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={cn(
          "inline-flex h-5 w-9 shrink-0 items-center rounded-full border p-0.5 transition-colors disabled:opacity-40",
          checked ? "justify-end border-sky-600 bg-sky-600" : "justify-start border-zinc-600 bg-zinc-800",
        )}
      >
        <span id={`${id}-thumb`} className="size-3.5 rounded-full bg-zinc-100 shadow-sm" />
      </button>
    </span>
  );
}
