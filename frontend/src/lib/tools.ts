export const TOOLS = [
  { id: "expansion", label: "Text Expansion", lockable: true },
  { id: "clipboard", label: "Clipboard", lockable: true },
  { id: "misspell", label: "Common misspells", lockable: false },
  { id: "date", label: "Date expander", lockable: false },
  { id: "ascii", label: "ASCII codes", lockable: false },
  { id: "units", label: "Units", lockable: false },
  { id: "app-shortcuts", label: "Keyboard Shortcuts", lockable: false },
  { id: "rename", label: "Batch rename", lockable: false },
  { id: "pantherbar", label: "Pantherbar", lockable: false },
  { id: "launcher", label: "Launcher", lockable: false },
  { id: "folder-rules", label: "Folder Rules", lockable: false },
  { id: "processes", label: "Processes", lockable: false },
  { id: "file-tools", label: "File Tools", lockable: false },
] as const;

export type ToolId = (typeof TOOLS)[number]["id"];

export function isToolId(value: string): value is ToolId {
  return TOOLS.some((tool) => tool.id === value);
}
