export const TOOLS = [
  { id: "expansion", label: "Text Expansion", lockable: true },
  { id: "clipboard", label: "Clipboard", lockable: true },
  { id: "megaclip", label: "MegaClip", lockable: false },
  { id: "misspell", label: "Common misspells", lockable: false },
  { id: "date", label: "Date expander", lockable: false },
  { id: "ascii", label: "ASCII codes", lockable: false },
  { id: "app-shortcuts", label: "Keyboard Shortcuts", lockable: false },
  { id: "rename", label: "Batch rename", lockable: false },
  { id: "pantherbar", label: "Pantherbar", lockable: false },
] as const;

export type ToolId = (typeof TOOLS)[number]["id"];
