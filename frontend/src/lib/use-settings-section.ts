import { useCallback, useEffect, useRef, useState } from "react";
import { messageOf } from "./message";

type Options<State, Settings> = {
  load: () => Promise<State>;
  save: (settings: Settings) => Promise<State>;
  settingsOf: (state: State) => Settings;
};

/**
 * Loads one settings section and saves edits to it. A save that fails puts
 * the last saved values back. Pass a delay for sliders so dragging does not
 * write the file on every step.
 */
export function useSettingsSection<State, Settings extends object>({ load, save, settingsOf }: Options<State, Settings>) {
  const [state, setState] = useState<State | null>(null);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const draftRef = useRef<Settings | null>(null);
  const savedRef = useRef<Settings | null>(null);
  const timerRef = useRef<number | undefined>(undefined);
  const settingsOfRef = useRef(settingsOf);
  settingsOfRef.current = settingsOf;

  const accept = useCallback((next: State) => {
    const settings = settingsOfRef.current(next);
    setState(next);
    savedRef.current = settings;
    draftRef.current = settings;
    setDraft(settings);
  }, []);

  useEffect(() => {
    let cancelled = false;
    void load()
      .then((next) => {
        if (!cancelled) {
          accept(next);
        }
      })
      .catch((caught) => {
        if (!cancelled) {
          setError(messageOf(caught));
        }
      });
    return () => {
      cancelled = true;
      window.clearTimeout(timerRef.current);
    };
  }, [accept, load]);

  const commit = useCallback(
    async (next: Settings) => {
      try {
        accept(await save(next));
        setError(null);
      } catch (caught) {
        setError(messageOf(caught));
        if (savedRef.current) {
          draftRef.current = savedRef.current;
          setDraft(savedRef.current);
        }
      }
    },
    [accept, save],
  );

  const update = useCallback(
    (patch: Partial<Settings>, delay = 0) => {
      const current = draftRef.current;
      if (!current) {
        return;
      }
      const next = { ...current, ...patch };
      draftRef.current = next;
      setDraft(next);
      window.clearTimeout(timerRef.current);
      if (delay > 0) {
        timerRef.current = window.setTimeout(() => void commit(next), delay);
      } else {
        void commit(next);
      }
    },
    [commit],
  );

  return { state, settings: draft, error, setError, update, accept };
}
