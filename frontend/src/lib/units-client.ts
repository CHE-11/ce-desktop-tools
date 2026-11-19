import { invoke } from "@tauri-apps/api/core";
import type { UnitsAnswer, UnitsCatalog } from "../../../types/units";

export function unitsQuery(text: string) {
  return invoke<UnitsAnswer>("units_query", { text });
}

export function unitsCatalog() {
  return invoke<UnitsCatalog>("units_catalog");
}
