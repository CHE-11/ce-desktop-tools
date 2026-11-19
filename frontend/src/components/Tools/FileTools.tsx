const FILE_TOOLS_URL = "https://ce-file-tools.vercel.app/";

export function FileTools() {
  return (
    <section id="file-tools" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <iframe
        id="file-tools-frame"
        title="File Tools"
        src={FILE_TOOLS_URL}
        className="min-h-0 w-full flex-1 border-0 bg-zinc-950"
      />
    </section>
  );
}
