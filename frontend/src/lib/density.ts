export const DENSITY_KEY = "ce-desktop-tools.density";
export const DENSITY_EVENT = "ce-desktop-tools-density";

export const MIN_DENSITY = 50;
export const MAX_DENSITY = 150;
export const DENSITY_STEP = 5;
export const DEFAULT_DENSITY = 100;

const LEGACY_DENSITY: Record<string, number> = {
  cozy: 125,
  default: DEFAULT_DENSITY,
  compact: 75,
};

/** Quick picks for the toolbar styles menu. */
export const DENSITY_OPTIONS: { id: number; label: string }[] = [
  { id: 125, label: "Cozy" },
  { id: DEFAULT_DENSITY, label: "Default" },
  { id: 75, label: "Compact" },
];

export function clampDensity(value: number) {
  const stepped = Math.round(value / DENSITY_STEP) * DENSITY_STEP;
  return Math.min(MAX_DENSITY, Math.max(MIN_DENSITY, stepped));
}

export function normalizeDensity(value: string | number | null | undefined): number {
  if (typeof value === "number" && Number.isFinite(value)) {
    return clampDensity(value);
  }
  if (typeof value === "string") {
    const trimmed = value.trim().toLowerCase();
    if (trimmed in LEGACY_DENSITY) {
      return LEGACY_DENSITY[trimmed];
    }
    const parsed = Number(trimmed);
    if (Number.isFinite(parsed)) {
      return clampDensity(parsed);
    }
  }
  return DEFAULT_DENSITY;
}

function readStored() {
  try {
    return localStorage.getItem(DENSITY_KEY);
  } catch {
    return null;
  }
}

export function readDensity() {
  return normalizeDensity(readStored());
}

export function applyDensity(density: number) {
  const next = clampDensity(density);
  const root = document.documentElement;
  root.dataset.density = String(next);
  root.style.setProperty("--density", String(next / 100));
}

function rememberDensity(density: number) {
  try {
    localStorage.setItem(DENSITY_KEY, String(density));
  } catch {
    // Ignore quota and private-mode failures; the density still applies for this session.
  }
  window.dispatchEvent(new CustomEvent(DENSITY_EVENT, { detail: density }));
}

export function publishDensity(value: string | number) {
  const next = normalizeDensity(value);
  applyDensity(next);
  rememberDensity(next);
  return next;
}
