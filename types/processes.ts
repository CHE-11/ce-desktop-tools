export type ProcessInfo = {
  pid: number;
  parentPid: number;
  name: string;
  path: string;
  memoryBytes: number;
};

/** A program icon for an executable path, as a PNG data URL. */
export type ProcessIcon = {
  path: string;
  icon: string;
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
