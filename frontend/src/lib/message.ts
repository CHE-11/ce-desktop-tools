export function messageOf(error: unknown) {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "Something went wrong.";
}

export function baseName(path: string) {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export function pickedFolderPath(picked: unknown): string | null {
  const value = Array.isArray(picked) ? picked[0] : picked;
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

export function childPath(folder: string, name: string) {
  const trimmed = folder.trim().replace(/[\\/]+$/, "");
  if (!trimmed) {
    return name;
  }
  return `${trimmed}\\${name}`;
}
