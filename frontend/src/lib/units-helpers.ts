export const PAIRS = [
  ["km", "mi"],
  ["m", "ft"],
  ["in", "cm"],
  ["kg", "lb"],
  ["°F", "°C"],
  ["L", "gal"],
  ["cup", "ml"],
  ["mph", "km/h"],
  ["GB", "GiB"],
] as const;

const SEPARATORS = [" to ", " -> ", " => ", " = ", " as "];

export function slugOf(symbol: string) {
  const slug = symbol
    .toLowerCase()
    .replaceAll("²", "2")
    .replaceAll("³", "3")
    .replaceAll("°", "deg")
    .replaceAll("µ", "u")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
  return slug || "unit";
}

export function leadingAmount(text: string) {
  const trimmed = text.trim();
  const mixed = trimmed.match(/^([+-]?\d[\d,]*(?:\.\d+)?)\s+(\d[\d,]*)\s*\/\s*(\d[\d,]*)/);
  if (mixed) {
    return `${mixed[1]} ${mixed[2]}/${mixed[3]}`;
  }
  const fraction = trimmed.match(/^([+-]?\d[\d,]*(?:\.\d+)?)\s*\/\s*(\d[\d,]*)/);
  if (fraction) {
    return `${fraction[1]}/${fraction[2]}`;
  }
  const number = trimmed.match(/^([+-]?(?:\d[\d,]*(?:\.\d+)?|\.\d+))/);
  if (!number || number[1] === "+" || number[1] === "-" || number[1] === ".") {
    return null;
  }
  return number[1];
}

export function leftOf(text: string) {
  const lower = text.toLowerCase();
  for (const separator of SEPARATORS) {
    const position = lower.indexOf(separator);
    if (position >= 0) {
      return text.slice(0, position).trim();
    }
  }
  const position = lower.lastIndexOf(" in ");
  if (position >= 0) {
    return text.slice(0, position).trim();
  }
  return null;
}
