export type RenameKind = "keep" | "prefix" | "suffix" | "number" | "replace";

export type RenameName = {
  from: string;
  name: string;
};

export type RenamePlan = {
  paths: string[];
  kind: RenameKind;
  text: string;
  replacement: string;
  start: number;
  pad: number;
  names: RenameName[];
};

export type RenameRow = {
  from: string;
  to: string;
  ok: boolean;
  message: string;
};
