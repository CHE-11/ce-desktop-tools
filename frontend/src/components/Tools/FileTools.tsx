const FILE_TOOLS_URL = "https://ce-file-tools.vercel.app/";

function markDropTarget(frame: HTMLIFrameElement | null) {
  frame?.setAttribute("dropzone", "copy");
}

export function FileTools() {
  return (
    <section id="file-tools" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <iframe
        id="file-tools-frame"
        title="File Tools"
        src={FILE_TOOLS_URL}
        ref={markDropTarget}
        className="min-h-0 w-full flex-1 border-0 bg-zinc-950"
      />
    </section>
  );
}
