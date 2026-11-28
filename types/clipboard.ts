export type Clip = {
  id: string;
  text: string;
  pinned: boolean;
  copiedAt: number;
};

export type ClipboardState = {
  locked: boolean;
  clips: Clip[];
  error: string | null;
};

export type ClipRule = {
  id: string;
  enabled: boolean;
  find: string;
  replace: string;
  caseInsensitive: boolean;
};

export type ClipCleaner = {
  enabled: boolean;
  rules: ClipRule[];
};

export type ClipCleanerPreview = {
  text: string;
  error: string | null;
};
