import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { api, errorText } from "../api";
import type { AvdInfo, DeviceStatus } from "../types";

interface Props {
  devices: AvdInfo[];
  statuses: Record<string, DeviceStatus>;
  initialDevice: string;
}

type Result = { ok: boolean; text: string } | null;

export default function InstallApk({ devices, statuses, initialDevice }: Props) {
  const ready = devices.filter((d) => statuses[d.name]?.state === "running");
  const [target, setTarget] = useState(initialDevice);
  const [path, setPath] = useState("");
  const [hover, setHover] = useState(false);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<Result>(null);

  // Keep the selection valid as devices start and stop.
  useEffect(() => {
    if (!ready.some((d) => d.name === target)) setTarget(ready[0]?.name ?? "");
  }, [ready, target]);

  // Drop an .apk onto the window.
  useEffect(() => {
    let un: (() => void) | undefined;
    let disposed = false;
    getCurrentWebview()
      .onDragDropEvent((e) => {
        const t = e.payload.type;
        if (t === "enter" || t === "over") setHover(true);
        else if (t === "leave") setHover(false);
        else if (t === "drop") {
          setHover(false);
          const apk = e.payload.paths.find((p) => p.toLowerCase().endsWith(".apk"));
          if (apk) {
            setPath(apk);
            setResult(null);
          } else {
            setResult({ ok: false, text: "That file isn’t an .apk." });
          }
        }
      })
      .then((f) => {
        if (disposed) f();
        else un = f;
      });
    return () => {
      disposed = true;
      un?.();
    };
  }, []);

  async function browse() {
    const picked = await open({
      multiple: false,
      filters: [{ name: "Android package", extensions: ["apk"] }],
    });
    if (typeof picked === "string") {
      setPath(picked);
      setResult(null);
    }
  }

  async function install() {
    setBusy(true);
    setResult(null);
    try {
      const msg = await api.installApk(target, path);
      setResult({ ok: true, text: msg });
    } catch (e) {
      setResult({ ok: false, text: errorText(e) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <section>
      <header className="page-head">
        <h1>Install APK</h1>
      </header>

      {ready.length === 0 ? (
        <div className="empty">
          <p>No device is ready.</p>
          <p className="muted">Start a device and wait for it to show Running, then come back.</p>
        </div>
      ) : (
        <div className="form">
          <label className="field">
            <span>Install on</span>
            <select value={target} onChange={(e) => setTarget(e.target.value)}>
              {ready.map((d) => (
                <option key={d.name} value={d.name}>
                  {d.name}
                </option>
              ))}
            </select>
          </label>

          <div className={hover ? "drop on" : "drop"}>
            <p>{path ? "Selected file" : "Drop an APK here"}</p>
            {path && <code className="path">{path}</code>}
            <button onClick={browse}>{path ? "Choose another" : "Browse…"}</button>
          </div>

          {result && (
            <div className={result.ok ? "banner ok" : "banner error"} role="status">
              <span className="pre">{result.text}</span>
            </div>
          )}

          <div className="form-actions">
            <button className="primary" disabled={!path || !target || busy} onClick={install}>
              {busy ? "Installing…" : "Install"}
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
