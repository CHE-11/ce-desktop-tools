import { invoke } from "@tauri-apps/api/core";
import type { CalcRef, CalcResult } from "../../../types/calculator";

export function calculatorEval(expression: string, previous: CalcRef[]) {
  return invoke<CalcResult>("calculator_eval", { expression, previous });
}
