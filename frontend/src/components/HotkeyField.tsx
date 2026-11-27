import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { chordOf } from "../lib/hotkey";
import { setHotkeyRecording } from "../lib/hotkeys-client";
import { cn } from "../lib/cn";

type HotkeyFieldProps = {
  id: string;
  value: string;
  disabled?: boolean;
  className?: string;
  onChange: (hotkey: string) => void;
};

export function HotkeyField({ id, value, disabled, className, onChange }: HotkeyFieldProps) {
  const [recording, setRecording] = useState(false);
  const fieldRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    void setHotkeyRecording(recording).catch(() => undefined);
  }, [recording]);

  useEffect(() => {
    return () => {
      void setHotkeyRecording(false).catch(() => undefined);
    };
  }, []);

  function onKeyDown(event: ReactKeyboardEvent<HTMLButtonElement>) {
    if (!recording) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      setRecording(false);
      return;
    }
    const chord = chordOf(event);
    if (!chord) {
      return;
    }
    setRecording(false);
    onChange(chord);
  }

  return (
    <button
      id={id}
      ref={fieldRef}
      type="button"
      aria-pressed={recording}
      disabled={disabled}
      onClick={() => {
        setRecording(true);
        fieldRef.current?.focus();
      }}
      onKeyDown={onKeyDown}
      onBlur={() => setRecording(false)}
      className={cn(
        "h-8 min-w-36 rounded-md border bg-zinc-950 px-2 text-left text-sm text-zinc-100 disabled:opacity-40",
        recording ? "border-sky-500" : "border-zinc-700",
        className,
      )}
    >
      {recording ? "Press a shortcut" : value}
    </button>
  );
}
