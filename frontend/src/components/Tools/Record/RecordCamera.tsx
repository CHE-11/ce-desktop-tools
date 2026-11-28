import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { onRecordStatus, recordAppendCamera, recordStatus } from "../../../lib/record-client";

let generation = 0;

export function RecordCamera() {
  const videoRef = useRef<HTMLVideoElement>(null);
  const recorder = useRef<MediaRecorder | null>(null);
  const stream = useRef<MediaStream | null>(null);
  const [message, setMessage] = useState("");

  useEffect(() => {
    document.title = "Recording camera";
    const mine = ++generation;
    let closed = false;
    let writes = Promise.resolve();

    const stopRecorder = () => {
      const current = recorder.current;
      if (!current) return;
      if (current.state !== "inactive") current.stop();
      recorder.current = null;
    };

    const stopAll = () => {
      stopRecorder();
      stream.current?.getTracks().forEach((track) => track.stop());
      stream.current = null;
      if (videoRef.current) videoRef.current.srcObject = null;
    };

    const preview = async () => {
      if (closed || stream.current) return;
      try {
        const media = await navigator.mediaDevices.getUserMedia({
          video: { width: { ideal: 640 }, height: { ideal: 640 }, facingMode: "user" },
          audio: false,
        });
        if (closed || mine !== generation) {
          media.getTracks().forEach((track) => track.stop());
          return;
        }
        stream.current = media;
        const video = videoRef.current;
        if (video) {
          video.srcObject = media;
          await video.play().catch(() => undefined);
        }
        setMessage("");
      } catch (caught) {
        const detail = caught instanceof Error ? caught.message : "";
        setMessage(detail ? `Camera: ${detail}` : "The camera could not start.");
      }
    };

    const record = () => {
      const media = stream.current;
      if (!media || recorder.current || closed) return;
      const mime = MediaRecorder.isTypeSupported("video/webm;codecs=vp9") ? "video/webm;codecs=vp9" : "video/webm";
      let recording: MediaRecorder;
      try {
        recording = new MediaRecorder(media, { mimeType: mime });
      } catch {
        return;
      }
      recording.ondataavailable = (event) => {
        if (event.data.size === 0) return;
        const chunk = event.data.arrayBuffer();
        writes = writes
          .then(() => chunk)
          .then((buffer) => recordAppendCamera(new Uint8Array(buffer)))
          .catch(() => undefined);
      };
      recording.start(250);
      recorder.current = recording;
    };

    const apply = (phase: string, camera: boolean) => {
      const live = camera && phase !== "idle" && phase !== "error";
      if (!live) {
        stopAll();
        return;
      }
      void preview().then(() => {
        const current = recorder.current;
        if (phase === "recording") {
          if (current?.state === "paused") current.resume();
          else record();
        } else if (phase === "paused") {
          if (current?.state === "recording") current.pause();
        }
      });
    };

    void recordStatus()
      .then((status) => apply(status.phase, status.camera))
      .catch(() => undefined);
    const listening = onRecordStatus((status) => apply(status.phase, status.camera));
    return () => {
      closed = true;
      if (mine === generation) stopAll();
      void listening.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <div id="record-camera" className="h-full w-full bg-transparent" onPointerDown={() => void getCurrentWindow().startDragging()}>
      <video id="record-camera-video" ref={videoRef} className="h-full w-full rounded-full bg-zinc-900 object-cover" autoPlay muted playsInline />
      {message ? (
        <p id="record-camera-message" className="pointer-events-none absolute inset-2 flex items-center justify-center text-center text-[10px] leading-tight text-zinc-100">
          {message}
        </p>
      ) : null}
    </div>
  );
}
