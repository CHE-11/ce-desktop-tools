export type AppCallPlacement = "foreground" | "background";

export type AppCallConnection = {
  protocol: string;
  localAddress: string;
  remoteAddress: string;
  remotePort: number;
  state: string;
  local: boolean;
};

export type AppCall = {
  placement: AppCallPlacement;
  name: string;
  path: string;
  pid: number;
  windowTitle: string;
  memoryBytes: number;
  cpuPercent: number | null;
  connections: AppCallConnection[];
};

export type AppCallsSnapshot = {
  foregroundPid: number;
  apps: AppCall[];
};
