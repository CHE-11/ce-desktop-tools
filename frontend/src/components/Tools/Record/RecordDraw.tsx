import { useEffect, useRef } from "react";
import type { RecordDrawTool, RecordStroke } from "../../../../../types/record";
import { onRecordDrawClear, onRecordDrawUndo, onRecordStatus, recordAddStroke, recordErase, recordSetDraft, recordStatus } from "../../../lib/record-client";
import { DRAW_WIDTH_SCALE, readDrawWidth } from "./record-draw-style";

type Point = { x: number; y: number };

/** Stroke widths as fractions of the frame width, at the medium size. */
const WIDTH: Record<RecordDrawTool, number> = {
  pen: 0.004,
  arrow: 0.004,
  rectangle: 0.004,
  highlighter: 0.02,
  eraser: 0.02,
};

const ERASE = 0.02;

function drawStroke(context: CanvasRenderingContext2D, canvas: HTMLCanvasElement, local: RecordStroke) {
  if (local.points.length === 0 || local.tool === "eraser") return;
  const xOf = (point: Point) => point.x * canvas.width;
  const yOf = (point: Point) => point.y * canvas.height;
  context.strokeStyle = local.color;
  context.fillStyle = local.color;
  context.lineWidth = Math.max(2, local.width * canvas.width);
  context.lineCap = "round";
  context.lineJoin = "round";
  context.globalAlpha = local.tool === "highlighter" ? 0.45 : 1;
  const first = local.points[0];
  const last = local.points[local.points.length - 1];
  if (local.tool === "rectangle") {
    context.strokeRect(xOf(first), yOf(first), xOf(last) - xOf(first), yOf(last) - yOf(first));
  } else if (local.tool === "arrow") {
    const x0 = xOf(first);
    const y0 = yOf(first);
    const x1 = xOf(last);
    const y1 = yOf(last);
    context.beginPath();
    context.moveTo(x0, y0);
    context.lineTo(x1, y1);
    context.stroke();
    const angle = Math.atan2(y1 - y0, x1 - x0);
    const length = Math.max(14, context.lineWidth * 4);
    context.beginPath();
    context.moveTo(x1, y1);
    context.lineTo(x1 - length * Math.cos(angle - 0.45), y1 - length * Math.sin(angle - 0.45));
    context.moveTo(x1, y1);
    context.lineTo(x1 - length * Math.cos(angle + 0.45), y1 - length * Math.sin(angle + 0.45));
    context.stroke();
  } else {
    context.beginPath();
    local.points.forEach((point, index) => {
      const x = xOf(point);
      const y = yOf(point);
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    });
    context.stroke();
  }
  context.globalAlpha = 1;
}

function nearSegment(point: Point, start: Point, end: Point, radius: number) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const len2 = dx * dx + dy * dy;
  const t = len2 <= 1e-8 ? 0 : Math.min(1, Math.max(0, ((point.x - start.x) * dx + (point.y - start.y) * dy) / len2));
  const px = start.x + dx * t - point.x;
  const py = start.y + dy * t - point.y;
  return px * px + py * py <= radius * radius;
}

function hits(stroke: RecordStroke, point: Point) {
  const radius = Math.max(ERASE, (stroke.width || 0.004) * 0.5);
  const edge = (start: Point, end: Point) => nearSegment(point, start, end, radius);
  const first = stroke.points[0];
  const last = stroke.points[stroke.points.length - 1];
  if (!first || !last) return false;
  if (stroke.tool === "rectangle") {
    return edge(first, { x: last.x, y: first.y }) || edge({ x: last.x, y: first.y }, last) || edge(last, { x: first.x, y: last.y }) || edge({ x: first.x, y: last.y }, first);
  }
  if (stroke.tool === "arrow") return edge(first, last);
  return stroke.points.some((sample, index) => edge(stroke.points[index - 1] ?? sample, sample));
}

export function RecordDraw() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const style = useRef<{ tool: RecordDrawTool; color: string }>({ tool: "pen", color: "#ef4444" });
  const points = useRef<Point[]>([]);
  const finished = useRef<RecordStroke[]>([]);
  const drawing = useRef(false);

  const stroke = (draft: boolean): RecordStroke => {
    const tool = style.current.tool;
    const raw = points.current;
    const shape = tool === "rectangle" || tool === "arrow";
    const used = shape && raw.length > 1 ? [raw[0], raw[raw.length - 1]] : raw;
    const scale = DRAW_WIDTH_SCALE[readDrawWidth()] ?? 1;
    return {
      tool,
      color: style.current.color,
      width: (WIDTH[tool] ?? 0.004) * scale,
      points: used.map((point) => ({ x: point.x, y: point.y })),
      draft,
    };
  };

  const redraw = () => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context) return;
    context.clearRect(0, 0, canvas.width, canvas.height);
    for (const done of finished.current) drawStroke(context, canvas, done);
    if (style.current.tool === "eraser") return;
    const local = stroke(false);
    if (local.points.length === 0) return;
    drawStroke(context, canvas, local);
  };

  useEffect(() => {
    document.title = "Recording draw";
    const canvas = canvasRef.current;
    const apply = (status: { tool: RecordDrawTool; color: string }) => {
      style.current = { tool: status.tool, color: status.color };
      if (canvas) canvas.style.cursor = status.tool === "eraser" ? "cell" : "crosshair";
    };
    void recordStatus().then(apply).catch(() => undefined);
    const listening = [
      onRecordStatus(apply),
      onRecordDrawUndo(() => {
        finished.current.pop();
        points.current = [];
        redraw();
      }),
      onRecordDrawClear(() => {
        finished.current = [];
        points.current = [];
        redraw();
      }),
    ];
    const resize = () => {
      if (!canvas) return;
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
      redraw();
    };
    resize();
    window.addEventListener("resize", resize);
    return () => {
      window.removeEventListener("resize", resize);
      for (const pending of listening) void pending.then((stop) => stop());
    };
    // The canvas and handlers live for the whole window.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const pointOf = (event: { nativeEvent: { offsetX: number; offsetY: number } }, canvas: HTMLCanvasElement): Point => ({
    x: event.nativeEvent.offsetX / canvas.width,
    y: event.nativeEvent.offsetY / canvas.height,
  });

  const erase = (point: Point) => {
    const before = finished.current.length;
    finished.current = finished.current.filter((done) => !hits(done, point));
    if (finished.current.length !== before) redraw();
    void recordErase([point]).catch(() => undefined);
  };

  return (
    <canvas
      id="record-draw-canvas"
      ref={canvasRef}
      className="h-full w-full touch-none"
      onPointerDown={(event) => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        drawing.current = true;
        canvas.setPointerCapture(event.pointerId);
        const point = pointOf(event, canvas);
        if (style.current.tool === "eraser") {
          erase(point);
          return;
        }
        points.current = [point];
        redraw();
      }}
      onPointerMove={(event) => {
        const canvas = canvasRef.current;
        if (!drawing.current || !canvas) return;
        const point = pointOf(event, canvas);
        if (style.current.tool === "eraser") {
          erase(point);
          return;
        }
        points.current.push(point);
        redraw();
        void recordSetDraft(stroke(true)).catch(() => undefined);
      }}
      onPointerUp={() => {
        if (!drawing.current) return;
        drawing.current = false;
        if (style.current.tool === "eraser") {
          points.current = [];
          return;
        }
        const done = stroke(false);
        points.current = [];
        if (done.points.length > 0) {
          finished.current.push(done);
          void recordAddStroke(done).catch(() => undefined);
        }
        redraw();
      }}
    />
  );
}
