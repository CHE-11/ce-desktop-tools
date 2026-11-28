import { useEffect, useRef, useState } from "react";
import { Download, FolderOpen, Trash2 } from "lucide-react";
import type { CameraLayout, RecordEntry } from "../../../../../types/record";
import { revealPath } from "../../../lib/app-client";
import { cn } from "../../../lib/cn";
import { messageOf } from "../../../lib/message";
import {
  chooseVideoPath,
  recordAppendExport,
  recordBeginExport,
  recordDelete,
  recordExport,
  recordFinishExport,
  recordMediaUrl,
  recordUpdate,
} from "../../../lib/record-client";
import { Button } from "../../Button";
import { clock } from "./time";
import { TrimBar } from "./TrimBar";

type RecordLibraryProps = {
  items: RecordEntry[];
  selected: string;
  onSelect: (id: string) => void;
  onChange: (entry: RecordEntry) => void;
  onReload: () => void;
};

export function RecordLibrary({ items, selected, onSelect, onChange, onReload }: RecordLibraryProps) {
  const [error, setError] = useState("");
  const [exporting, setExporting] = useState("");
  const item = items.find((entry) => entry.id === selected) ?? items[0];

  return (
    <div id="record-library" className="flex min-h-0 flex-1">
      <aside id="record-library-list" className="flex w-64 shrink-0 flex-col border-r border-zinc-800">
        {items.length === 0 ? (
          <p id="record-library-empty" className="px-4 py-3 text-sm text-zinc-500">
            Recordings you make show up here.
          </p>
        ) : (
          <div id="record-library-items" className="min-h-0 flex-1 overflow-auto">
            {items.map((entry) => (
              <button
                id={`record-library-item-${entry.id}`}
                key={entry.id}
                type="button"
                className={cn("flex w-full flex-col items-start px-4 py-2 text-left hover:bg-zinc-900", item?.id === entry.id && "bg-zinc-900")}
                onClick={() => onSelect(entry.id)}
              >
                <span id={`record-library-item-${entry.id}-title`} className="w-full truncate text-sm text-zinc-100">
                  {entry.title || "Recording"}
                </span>
                <span id={`record-library-item-${entry.id}-meta`} className="w-full truncate text-xs text-zinc-500">
                  {clock(entry.durationMs)} · {new Date(entry.createdAt).toLocaleString()}
                </span>
              </button>
            ))}
          </div>
        )}
      </aside>
      <section id="record-library-player" className="flex min-w-0 flex-1 flex-col">
        {item ? (
          <Player
            key={item.id}
            item={item}
            exporting={exporting}
            error={error}
            onError={setError}
            onExporting={setExporting}
            onChange={onChange}
            onDelete={() => {
              setError("");
              void recordDelete(item.id)
                .then(onReload)
                .catch((caught: unknown) => setError(messageOf(caught)));
            }}
          />
        ) : (
          <p id="record-library-none" className="m-auto text-sm text-zinc-500">
            Nothing to play yet.
          </p>
        )}
      </section>
    </div>
  );
}

type PlayerProps = {
  item: RecordEntry;
  exporting: string;
  error: string;
  onError: (message: string) => void;
  onExporting: (message: string) => void;
  onChange: (item: RecordEntry) => void;
  onDelete: () => void;
};

function Player({ item, exporting, error, onError, onExporting, onChange, onDelete }: PlayerProps) {
  const screenRef = useRef<HTMLVideoElement>(null);
  const cameraRef = useRef<HTMLVideoElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const [videoUrl, setVideoUrl] = useState("");
  const [cameraUrl, setCameraUrl] = useState("");
  const [title, setTitle] = useState(item.title);
  const [camera, setCamera] = useState<CameraLayout>(item.camera);
  const [currentMs, setCurrentMs] = useState(0);
  const [confirming, setConfirming] = useState(false);
  const dragging = useRef<{ x: number; y: number; left: number; top: number } | null>(null);

  useEffect(() => {
    setTitle(item.title);
    setCamera(item.camera);
  }, [item.title, item.camera]);

  useEffect(() => {
    let cancel = false;
    void recordMediaUrl(item.id, "video")
      .then((url) => {
        if (!cancel) setVideoUrl(url);
      })
      .catch((caught: unknown) => onError(messageOf(caught)));
    if (item.cameraPath) {
      void recordMediaUrl(item.id, "camera")
        .then((url) => {
          if (!cancel) setCameraUrl(url);
        })
        .catch(() => undefined);
    } else {
      setCameraUrl("");
    }
    return () => {
      cancel = true;
    };
  }, [item.id, item.cameraPath, onError]);

  const trimEnd = item.trimEndMs || item.durationMs;
  const busy = exporting.startsWith("Exporting");

  const seekMedia = (video: HTMLVideoElement, wallMs: number) => {
    const scale = mediaScale(video, item.durationMs);
    video.playbackRate = scale;
    const at = (wallMs / 1000) * scale;
    if (Math.abs(video.currentTime - at) > 0.05) video.currentTime = at;
  };

  const saveVideo = async () => {
    const path = await chooseVideoPath(item.title);
    if (!path) return;
    onError("");
    try {
      await recordExport(item.id, path);
      onExporting(`Saved to ${path}.`);
    } catch (caught) {
      onError(messageOf(caught));
    }
  };

  const saveWithCamera = async () => {
    if (!videoUrl) return;
    const path = await chooseVideoPath(item.title);
    if (!path) return;
    const target = path.toLowerCase().endsWith(".webm") ? path : path.replace(/\.[^.\\/]+$/, "") + ".webm";
    onError("");
    onExporting("Exporting. This plays the video through once.");
    try {
      await burnCamera(item, videoUrl, cameraUrl, camera, target, (note) => onExporting(note));
      onExporting(`Saved to ${target}.`);
    } catch (caught) {
      onError(messageOf(caught));
      onExporting("");
    }
  };

  return (
    <div id="record-player" className="flex min-h-0 flex-1 flex-col">
      <div id="record-player-toolbar" className="flex flex-wrap items-center gap-2 border-b border-zinc-800 px-4 py-3">
        <input
          id="record-player-title"
          className="h-8 min-w-40 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100"
          value={title}
          aria-label="Title"
          onChange={(event) => setTitle(event.target.value)}
          onBlur={() => {
            if (title.trim() && title !== item.title) {
              void recordUpdate({ id: item.id, title: title.trim() })
                .then(onChange)
                .catch((caught: unknown) => onError(messageOf(caught)));
            }
          }}
        />
        <Button id="record-player-show" variant="outline" onClick={() => void revealPath(item.videoPath).catch((caught: unknown) => onError(messageOf(caught)))}>
          <FolderOpen id="record-player-show-icon" className="size-4" aria-hidden />
          Show
        </Button>
        <Button id="record-player-export" variant="outline" disabled={busy} onClick={() => void saveVideo()}>
          <Download id="record-player-export-icon" className="size-4" aria-hidden />
          Save video
        </Button>
        {item.cameraPath ? (
          <Button id="record-player-export-camera" variant="outline" disabled={busy} onClick={() => void saveWithCamera()}>
            Save with camera
          </Button>
        ) : null}
        {confirming ? (
          <>
            <Button id="record-player-delete-confirm" variant="destructive" onClick={onDelete}>
              Delete for good
            </Button>
            <Button id="record-player-delete-cancel" variant="ghost" onClick={() => setConfirming(false)}>
              Keep
            </Button>
          </>
        ) : (
          <Button id="record-player-delete" variant="ghost" onClick={() => setConfirming(true)}>
            <Trash2 id="record-player-delete-icon" className="size-4" aria-hidden />
            Delete
          </Button>
        )}
      </div>
      {error ? (
        <p id="record-player-error" className="px-4 py-2 text-sm text-amber-300">
          {error}
        </p>
      ) : null}
      {exporting ? (
        <p id="record-player-export-note" className="px-4 py-2 text-sm text-zinc-400">
          {exporting}
        </p>
      ) : null}
      <div id="record-player-stage" ref={stageRef} className="relative m-4 min-h-48 flex-1 overflow-hidden rounded-lg bg-black">
        <video
          id="record-player-video"
          ref={screenRef}
          className="h-full w-full object-contain"
          src={videoUrl || undefined}
          controls
          onLoadedMetadata={(event) => seekMedia(event.currentTarget, item.trimStartMs)}
          onPlay={() => {
            const cameraVideo = cameraRef.current;
            const screen = screenRef.current;
            if (!screen) return;
            screen.playbackRate = mediaScale(screen, item.durationMs);
            if (cameraVideo) {
              cameraVideo.currentTime = wallSeconds(screen, item.durationMs);
              void cameraVideo.play().catch(() => undefined);
            }
          }}
          onPause={() => cameraRef.current?.pause()}
          onTimeUpdate={(event) => {
            const video = event.currentTarget;
            const at = wallSeconds(video, item.durationMs) * 1000;
            setCurrentMs(at);
            if (at >= trimEnd) {
              video.pause();
              seekMedia(video, trimEnd);
            }
            const cameraVideo = cameraRef.current;
            const wall = at / 1000;
            if (cameraVideo && Math.abs(cameraVideo.currentTime - wall) > 0.3) {
              cameraVideo.currentTime = wall;
            }
          }}
        />
        {cameraUrl ? (
          <video
            id="record-player-camera"
            ref={cameraRef}
            className="absolute cursor-grab rounded-full object-cover shadow-lg"
            style={{ left: `${camera.x * 100}%`, top: `${camera.y * 100}%`, width: `${camera.size * 100}%`, aspectRatio: "1" }}
            src={cameraUrl}
            muted
            playsInline
            onPointerDown={(event) => {
              dragging.current = { x: event.clientX, y: event.clientY, left: camera.x, top: camera.y };
              event.currentTarget.setPointerCapture(event.pointerId);
            }}
            onPointerMove={(event) => {
              const drag = dragging.current;
              const stage = stageRef.current?.getBoundingClientRect();
              if (!drag || !stage) return;
              setCamera({
                ...camera,
                x: Math.min(0.9, Math.max(-0.1, drag.left + (event.clientX - drag.x) / stage.width)),
                y: Math.min(0.9, Math.max(-0.1, drag.top + (event.clientY - drag.y) / stage.height)),
              });
            }}
            onPointerUp={() => {
              if (!dragging.current) return;
              dragging.current = null;
              void recordUpdate({ id: item.id, camera })
                .then(onChange)
                .catch(() => undefined);
            }}
          />
        ) : null}
        <span id="record-player-clock" className="pointer-events-none absolute top-3 right-3 rounded-full bg-black/70 px-2 py-1 text-xs font-semibold text-zinc-100 tabular-nums">
          {clock(currentMs)} / {clock(trimEnd)}
        </span>
      </div>
      <TrimBar
        durationMs={item.durationMs}
        startMs={item.trimStartMs}
        endMs={trimEnd}
        currentMs={currentMs}
        onChange={(trimStartMs, trimEndMs, handle) => {
          onChange({ ...item, trimStartMs, trimEndMs });
          const video = screenRef.current;
          if (video) seekMedia(video, handle === "start" ? trimStartMs : trimEndMs);
        }}
        onCommit={(trimStartMs, trimEndMs) => {
          void recordUpdate({ id: item.id, trimStartMs, trimEndMs })
            .then(onChange)
            .catch(() => undefined);
        }}
      />
    </div>
  );
}

/** Plays the trimmed take once into a canvas with the camera bubble drawn on, and saves that as WebM. */
async function burnCamera(item: RecordEntry, videoUrl: string, cameraUrl: string, layout: CameraLayout, path: string, onProgress: (note: string) => void) {
  const screen = document.createElement("video");
  screen.src = videoUrl;
  screen.crossOrigin = "anonymous";
  const camera = cameraUrl ? document.createElement("video") : null;
  if (camera) {
    camera.src = cameraUrl;
    camera.muted = true;
    camera.crossOrigin = "anonymous";
  }
  await new Promise<void>((resolve, reject) => {
    screen.onloadeddata = () => resolve();
    screen.onerror = () => reject(new Error("Couldn't export the recording."));
  });
  const width = item.width || screen.videoWidth || 1280;
  const height = item.height || screen.videoHeight || 720;
  const canvas = document.createElement("canvas");
  canvas.id = "record-export-canvas";
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Couldn't export the recording.");
  const audio = new AudioContext();
  const source = audio.createMediaElementSource(screen);
  const dest = audio.createMediaStreamDestination();
  source.connect(dest);
  const stream = canvas.captureStream(30);
  const track = dest.stream.getAudioTracks()[0];
  if (track) stream.addTrack(track);
  const mime = MediaRecorder.isTypeSupported("video/webm;codecs=vp9,opus") ? "video/webm;codecs=vp9,opus" : "video/webm";
  const recorder = new MediaRecorder(stream, { mimeType: mime });
  await recordBeginExport(path);
  let writes = Promise.resolve();
  const done = new Promise<void>((resolve) => {
    recorder.ondataavailable = (event) => {
      if (event.data.size === 0) return;
      const chunk = event.data.arrayBuffer();
      writes = writes.then(() => chunk).then((buffer) => recordAppendExport(new Uint8Array(buffer)));
    };
    recorder.onstop = () => resolve();
  });
  const start = item.trimStartMs / 1000;
  const end = (item.trimEndMs || item.durationMs) / 1000;
  const scale = mediaScale(screen, item.durationMs);
  screen.playbackRate = scale;
  screen.currentTime = start * scale;
  if (camera) camera.currentTime = start;
  recorder.start(200);
  await screen.play();
  if (camera) void camera.play().catch(() => undefined);
  await new Promise<void>((resolve) => {
    const draw = () => {
      context.drawImage(screen, 0, 0, width, height);
      if (camera && camera.readyState >= 2) {
        const size = layout.size * width;
        context.save();
        context.beginPath();
        context.arc(layout.x * width + size / 2, layout.y * height + size / 2, size / 2, 0, Math.PI * 2);
        context.clip();
        context.drawImage(camera, layout.x * width, layout.y * height, size, size);
        context.restore();
      }
      const wall = wallSeconds(screen, item.durationMs);
      if (camera && Math.abs(camera.currentTime - wall) > 0.3) camera.currentTime = wall;
      onProgress(`Exporting ${clock(wall * 1000)} of ${clock(end * 1000)}. This plays the video through once.`);
      if (wall >= end || screen.ended) {
        screen.pause();
        camera?.pause();
        resolve();
        return;
      }
      requestAnimationFrame(draw);
    };
    requestAnimationFrame(draw);
  });
  recorder.stop();
  await done;
  await writes;
  await recordFinishExport();
  await audio.close();
}

/** How many seconds of the file correspond to one second of the take. */
function mediaScale(video: HTMLVideoElement, durationMs: number) {
  const mediaMs = video.duration * 1000;
  if (!Number.isFinite(mediaMs) || mediaMs <= 0 || durationMs <= 0) return 1;
  return mediaMs / durationMs;
}

function wallSeconds(video: HTMLVideoElement, durationMs: number) {
  const scale = mediaScale(video, durationMs);
  return scale > 0 ? video.currentTime / scale : video.currentTime;
}
