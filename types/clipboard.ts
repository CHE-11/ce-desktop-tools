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
