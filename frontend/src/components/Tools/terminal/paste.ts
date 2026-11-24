import type { ShellKind } from "../../../../../types/terminal";

export type PasteMode = "lines" | "block";

const SMART_QUOTES: [RegExp, string][] = [
  [/[\u2018\u2019\u201A\u201B]/g, "'"],
  [/[\u201C\u201D\u201E\u201F]/g, '"'],
  [/[\u2013\u2014]/g, "-"],
  [/\u00A0/g, " "],
];

export function normalizePaste(text: string, stripSmartQuotes: boolean) {
  let next = text.replace(/\r\n?/g, "\n");
  if (stripSmartQuotes) {
    for (const [pattern, replacement] of SMART_QUOTES) {
      next = next.replace(pattern, replacement);
    }
  }
  return next;
}

export function hasSmartQuotes(text: string) {
  return SMART_QUOTES.some(([pattern]) => text.search(pattern) >= 0);
}

/** Lines to run, without the trailing blank lines a copied block usually ends with. */
export function pasteLines(text: string) {
  const lines = text.split("\n");
  while (lines.length > 0 && lines[lines.length - 1].trim() === "") {
    lines.pop();
  }
  return lines;
}

export function isMultiline(text: string) {
  return pasteLines(normalizePaste(text, false)).length > 1;
}

export function quotePath(path: string, shell: ShellKind) {
  if (shell === "cmd") {
    return /[\s&()^,;=!'+]/.test(path) ? `"${path}"` : path;
  }
  return /[\s'`$&(){}@;,]/.test(path) ? `'${path.replace(/'/g, "''")}'` : path;
}

/** Clears the line being typed (Escape) and changes to `folder`. */
export function changeDirectory(folder: string, shell: ShellKind) {
  if (shell === "cmd") {
    return `\x1bcd /d "${folder}"\r`;
  }
  return `\x1bSet-Location -LiteralPath '${folder.replace(/'/g, "''")}'\r`;
}

/**
 * The chunks to send for a paste. Line mode sends one command per chunk.
 * Block mode sends one chunk that the shell runs together.
 */
export function pasteChunks(text: string, mode: PasteMode, shell: ShellKind): string[] {
  const lines = pasteLines(text);
  if (mode === "lines") {
    return lines.map((line) => `${line}\r`);
  }
  if (shell === "cmd") {
    const commands = lines.map((line) => line.trim()).filter(Boolean);
    return [`${commands.join(" & ")}\r`];
  }
  // PSReadLine keeps reading while braces are open, so the block runs once when it closes.
  return [`& {\r${lines.join("\r")}\r}\r`];
}
