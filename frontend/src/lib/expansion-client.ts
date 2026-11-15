import { invoke } from "@tauri-apps/api/core";
import type { ExpansionState, Snippet } from "../../../types/expansion";

export function expansionState() {
  return invoke<ExpansionState>("expansion_state");
}

export function saveExpansions(snippets: Snippet[]) {
  return invoke<ExpansionState>("save_expansions", { snippets });
}

export function setExpansionEnabled(enabled: boolean) {
  return invoke<boolean>("set_expansion_enabled", { enabled });
}

export function createVaultPin(pin: string) {
  return invoke<ExpansionState>("create_vault_pin", { pin });
}

export function unlockVault(pin: string) {
  return invoke<ExpansionState>("unlock_vault", { pin });
}

export function lockVault() {
  return invoke<ExpansionState>("lock_vault");
}
