export type PantherbarTransform = {
  op: string;
  prefix: string;
  suffix: string;
  find: string;
  replaceWith: string;
};

export type PantherbarCustomAction = {
  id: string;
  name: string;
  icon: string;
  kind: string;
  urlTemplate: string;
  transform: PantherbarTransform;
  output: string;
};

export type PantherbarCatalogEntry = {
  id: string;
  name: string;
  group: string;
  icon: string;
};

export type PantherbarAction = {
  id: string;
  name: string;
  icon: string;
};

export type PantherbarState = {
  enabled: boolean;
  installed: string[];
  usingDefaults: boolean;
  custom: PantherbarCustomAction[];
  excludedApps: string[];
  catalog: PantherbarCatalogEntry[];
  icons: string[];
  defaults: string[];
  lastApp: string;
  hotkey: string;
};

export type PantherbarSave = {
  enabled: boolean;
  installed: string[];
  custom: PantherbarCustomAction[];
  excludedApps: string[];
  hotkey: string;
};

export type PantherbarShow = {
  actions: PantherbarAction[];
};
