export const TOOLS = [{ id: "expansion", label: "Text Expansion" }] as const;

export type ToolId = (typeof TOOLS)[number]["id"];
