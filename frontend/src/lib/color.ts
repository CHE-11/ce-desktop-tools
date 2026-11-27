/** Hue 0-360, saturation and value 0-100. */
export type Hsv = { h: number; s: number; v: number };

export function normalizeHex(value: string) {
  const hex = value.trim().replace(/^#/, "").toLowerCase();
  if (/^[0-9a-f]{3}$/.test(hex)) {
    return `#${hex
      .split("")
      .map((digit) => digit + digit)
      .join("")}`;
  }
  return /^[0-9a-f]{6}$/.test(hex) ? `#${hex}` : null;
}

function channels(hex: string): [number, number, number] {
  const value = Number.parseInt(hex.slice(1), 16);
  return [(value >> 16) & 0xff, (value >> 8) & 0xff, value & 0xff];
}

function toHex(r: number, g: number, b: number) {
  return `#${[r, g, b]
    .map((channel) =>
      Math.round(Math.min(255, Math.max(0, channel)))
        .toString(16)
        .padStart(2, "0"),
    )
    .join("")}`;
}

export function hexToHsv(hex: string): Hsv {
  const [r, g, b] = channels(hex).map((channel) => channel / 255);
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const delta = max - min;
  let h = 0;
  if (delta !== 0) {
    if (max === r) {
      h = 60 * (((g - b) / delta) % 6);
    } else if (max === g) {
      h = 60 * ((b - r) / delta + 2);
    } else {
      h = 60 * ((r - g) / delta + 4);
    }
  }
  if (h < 0) {
    h += 360;
  }
  return { h: Math.round(h), s: Math.round(max === 0 ? 0 : (delta / max) * 100), v: Math.round(max * 100) };
}

export function hsvToHex({ h, s, v }: Hsv) {
  const saturation = s / 100;
  const value = v / 100;
  const chroma = value * saturation;
  const x = chroma * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = value - chroma;
  const [r, g, b] =
    h < 60
      ? [chroma, x, 0]
      : h < 120
        ? [x, chroma, 0]
        : h < 180
          ? [0, chroma, x]
          : h < 240
            ? [0, x, chroma]
            : h < 300
              ? [x, 0, chroma]
              : [chroma, 0, x];
  return toHex((r + m) * 255, (g + m) * 255, (b + m) * 255);
}

/** Mixes toward white (positive amount) or black (negative), amount -1..1. */
export function shade(hex: string, amount: number) {
  const target = amount >= 0 ? 255 : 0;
  const weight = Math.abs(amount);
  const [r, g, b] = channels(hex);
  return toHex(r + (target - r) * weight, g + (target - g) * weight, b + (target - b) * weight);
}

/** Black or white, whichever reads better on the color. */
export function readableOn(hex: string) {
  const [r, g, b] = channels(hex);
  return 0.299 * r + 0.587 * g + 0.114 * b > 150 ? "#000000" : "#ffffff";
}
