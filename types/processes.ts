export type ProcessInfo = {
  pid: number;
  parentPid: number;
  name: string;
  path: string;
  memoryBytes: number;
};

export type PortOwner = {
  protocol: string;
  localPort: number;
  localAddress: string;
  remoteAddress: string;
  state: string;
  pid: number;
  name: string;
};

export type FileLocker = {
  pid: number;
  name: string;
  appName: string;
  path: string;
};
