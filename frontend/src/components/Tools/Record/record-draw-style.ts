/** The stroke size picked on the controls pill. The draw window reads it on each stroke. */
const DRAW_WIDTH = "ce-record-draw-width";

/** Hairline, thin, medium, thick. */
export const DRAW_WIDTH_SCALE: Record<number, number> = { 0: 0.32, 1: 0.55, 2: 1, 3: 2.4 };

export function readDrawWidth() {
  const saved = localStorage.getItem(DRAW_WIDTH);
  const value = saved === null ? 2 : Number(saved);
  return value === 0 || value === 1 || value === 2 || value === 3 ? value : 2;
}

export function rememberDrawWidth(size: number) {
  localStorage.setItem(DRAW_WIDTH, String(size));
}
