import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { errorText } from "../api";
import type { AccelInfo, SdkInfo, Settings } from "../types";

interface Props {
  settings: Settings;
  sdk: SdkInfo | null;
  accel: AccelInfo | null;
  onSave: (s: Settings) => Promise<void>;
  onRescan: () => void;
}

const GPU_MODES = [
  { id: "auto", label: "Automatic" },
  { id: "host", label: "Host GPU" },
  { id: "angle_indirect", label: "ANGLE (Direct3D)" },
  { id: "swiftshader_indirect", label: "Software (slowest)" },
];

function Row({ label, value }: { label: string; value: string | null | undefined }) {
  return (
    <tr>
      <th scope="row">{label}</th>
      <td>{value ? <code>{value}</code> : <span className="muted">Not found</span>}</td>
    </tr>
  );
}

export default function SettingsView({ settings, sdk, accel, onSave, onRescan }: Props) {
  const [draft, setDraft] = useState<Settings>(settings);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const dirty = JSON.stringify(draft) !== JSON.stringify(settings);

  async function browse() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") setDraft({ ...draft, sdk_path: dir });
  }

  async function save() {
    setSaving(true);
    setMessage(null);
    try {
      await onSave(draft);
      setMessage({ ok: true, text: "Settings saved." });
    } catch (e) {
      setMessage({ ok: false, text: errorText(e) });
    } finally {
      setSaving(false);
    }
  }

  return (
    <section>
      <header className="page-head">
        <h1>Settings</h1>
      </header>

      <div className="form">
        <label className="field">
          <span>Android SDK folder</span>
          <div className="inline">
            <input
              value={draft.sdk_path ?? ""}
              placeholder="Detected automatically"
              onChange={(e) => setDraft({ ...draft, sdk_path: e.target.value || null })}
              spellCheck={false}
            />
            <button onClick={browse}>Browse…</button>
          </div>
          <small className="muted">Leave empty to use ANDROID_SDK_ROOT or the default install location.</small>
        </label>

        <label className="field">
          <span>Graphics</span>
          <select value={draft.gpu_mode} onChange={(e) => setDraft({ ...draft, gpu_mode: e.target.value })}>
            {GPU_MODES.map((m) => (
              <option key={m.id} value={m.id}>
                {m.label}
              </option>
            ))}
          </select>
        </label>

        <label className="check">
          <input
            type="checkbox"
            checked={draft.quick_boot}
            onChange={(e) => setDraft({ ...draft, quick_boot: e.target.checked })}
          />
          <span>
            Quick Boot
            <small className="muted"> Save the device state on shutdown and resume it next time.</small>
          </span>
        </label>

        {message && (
          <div className={message.ok ? "banner ok" : "banner error"} role="status">
            <span>{message.text}</span>
          </div>
        )}
        <div className="form-actions">
          <button className="primary" disabled={!dirty || saving} onClick={save}>
            {saving ? "Saving…" : "Save settings"}
          </button>
        </div>
      </div>

      <header className="page-head sub">
        <h2>Detected SDK</h2>
        <button onClick={onRescan}>Scan again</button>
      </header>
      <table className="kv">
        <tbody>
          <Row label="SDK folder" value={sdk?.root} />
          <Row label="Found via" value={sdk?.found ? sdk.source : null} />
          <Row label="Emulator" value={sdk?.emulator} />
          <Row label="adb" value={sdk?.adb} />
          <Row label="avdmanager" value={sdk?.avdmanager} />
          <tr>
            <th scope="row">Licenses</th>
            <td>{sdk?.licenses_accepted ? "Accepted" : <span className="muted">Not accepted</span>}</td>
          </tr>
          <tr>
            <th scope="row">Acceleration</th>
            <td>{accel ? `${accel.available ? "Available" : "Unavailable"}. ${accel.detail}` : "–"}</td>
          </tr>
          <tr>
            <th scope="row">System images</th>
            <td>
              {sdk && sdk.system_images.length > 0 ? (
                <ul className="plain">
                  {sdk.system_images.map((i) => (
                    <li key={i.id}>
                      <code>{i.id}</code>
                    </li>
                  ))}
                </ul>
              ) : (
                <span className="muted">None installed</span>
              )}
            </td>
          </tr>
        </tbody>
      </table>
    </section>
  );
}
