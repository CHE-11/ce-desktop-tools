export const DENSITY_KEY = "ce-desktop-tools.density";
export const DENSITY_EVENT = "ce-desktop-tools-density";

export const DENSITIES = ["cozy", "default", "compact"] as const;
export type Density = (typeof DENSITIES)[number];

export const DEFAULT_DENSITY: Density = "default";

export const DENSITY_SCALE: Record<Density, number> = {
  cozy: 1.25,
  default: 1,
  compact: 0.75,
};

export const DENSITY_OPTIONS: { id: Density; label: string }[] = [
  { id: "cozy", label: "Cozy" },
  { id: "default", label: "Default" },
  { id: "compact", label: "Compact" },
];

export function normalizeDensity(value: string | null | undefined): Density {
  return DENSITIES.find((density) => density === value) ?? DEFAULT_DENSITY;
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

export function applyDensity(density: Density) {
  const root = document.documentElement;
  root.dataset.density = density;
  root.style.setProperty("--density", String(DENSITY_SCALE[density]));
}

function rememberDensity(density: Density) {
  try {
    localStorage.setItem(DENSITY_KEY, density);
  } catch {
    // Ignore quota and private-mode failures; the density still applies for this session.
  }
  window.dispatchEvent(new CustomEvent(DENSITY_EVENT, { detail: density }));
}

export function publishDensity(value: string) {
  const next = normalizeDensity(value);
  applyDensity(next);
  rememberDensity(next);
  return next;
}
