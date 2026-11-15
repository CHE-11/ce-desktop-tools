import { invoke } from "@tauri-apps/api/core";
import type { AsciiCode, AsciiState } from "../../../types/ascii";

export function asciiState() {
  return invoke<AsciiState>("ascii_state");
}

export function asciiSave(entries: AsciiCode[], prefix: string) {
  return invoke<AsciiState>("ascii_save", { entries, prefix });
}

export function copyText(text: string) {
  return invoke<void>("copy_text", { text });
}
