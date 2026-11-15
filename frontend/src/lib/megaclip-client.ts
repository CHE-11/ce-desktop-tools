import { invoke } from "@tauri-apps/api/core";
import type { MegaClipState } from "../../../types/megaclip";

export function megaclipState() {
  return invoke<MegaClipState>("megaclip_state");
}

export function megaclipClear() {
  return invoke<MegaClipState>("megaclip_clear");
}

export function megaclipCopy(index: number) {
  return invoke<void>("megaclip_copy", { index });
}

export function megaclipSetIndex(index: number) {
  return invoke<MegaClipState>("megaclip_set_index", { index });
}
