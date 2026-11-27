import { invoke } from "@tauri-apps/api/core";

export function stayAwakeState() {
  return invoke<boolean>("stay_awake_state");
}

export function setStayAwake(on: boolean) {
  return invoke<boolean>("set_stay_awake", { on });
}
