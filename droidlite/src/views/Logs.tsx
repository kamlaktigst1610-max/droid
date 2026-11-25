import { useEffect, useRef } from "react";
import type { LogLine } from "../types";

function time(ts: number): string {
  return new Date(ts).toLocaleTimeString([], { hour12: false });
}

export default function Logs({ logs, onClear }: { logs: LogLine[]; onClear: () => void }) {
  const box = useRef<HTMLDivElement>(null);
  const stick = useRef(true);

  useEffect(() => {
    const el = box.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [logs]);

  return (
    <section className="logs-view">
      <header className="page-head">
        <h1>Logs</h1>
        <div className="head-actions">
          <button onClick={() => navigator.clipboard?.writeText(logs.map((l) => `${time(l.ts)} ${l.msg}`).join("\n"))}>
            Copy
          </button>
          <button onClick={onClear} disabled={logs.length === 0}>
            Clear
          </button>
        </div>
      </header>
      <div
        className="log-box"
        ref={box}
        onScroll={(e) => {
          const el = e.currentTarget;
          stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
        }}
      >
        {logs.length === 0 ? (
          <p className="muted">Nothing yet. Emulator output and adb results appear here.</p>
        ) : (
          logs.map((l, i) => (
            <div key={i} className={`log ${l.level}`}>
              <time>{time(l.ts)}</time>
              <span>{l.msg}</span>
            </div>
          ))
        )}
      </div>
    </section>
  );
}
