import type { HTMLAttributes } from "react";
import { cn } from "../lib/cn";

type KbdProps = HTMLAttributes<HTMLElement> & {
  id: string;
};

export function Kbd({ id, className, ...props }: KbdProps) {
  return (
    <kbd
      id={id}
      className={cn(
        "inline-flex h-6 min-w-6 items-center justify-center rounded-md border border-zinc-600/80 bg-zinc-800 px-1.5 font-sans text-[11px] font-medium text-zinc-200 shadow-[inset_0_1px_0_rgba(255,255,255,0.08),0_1px_1px_rgba(0,0,0,0.45)]",
        className,
      )}
      {...props}
    />
  );
}
