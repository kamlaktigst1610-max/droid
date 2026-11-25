import type { AccelInfo, AvdInfo, DeviceStatus, SdkInfo } from "../types";

interface Props {
  sdk: SdkInfo | null;
  accel: AccelInfo | null;
  devices: AvdInfo[];
  statuses: Record<string, DeviceStatus>;
  busy: Record<string, boolean>;
  onStart: (name: string) => void;
  onStop: (name: string) => void;
  onCreate: () => void;
  onSettings: () => void;
  onInstall: (name: string) => void;
}

const STAGES = [
  { label: "Emulator", at: 8 },
  { label: "ADB", at: 25 },
  { label: "Android", at: 55 },
  { label: "Ready", at: 100 },
];

function BootTrack({ progress }: { progress: number }) {
  return (
    <div
      className="track"
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={progress}
    >
      {STAGES.map((s) => (
        <span key={s.label} className={progress >= s.at ? "seg on" : "seg"}>
          <i />
          <b>{s.label}</b>
        </span>
      ))}
    </div>
  );
}

export default function Devices(p: Props) {
  const { sdk, accel } = p;

  return (
    <section>
      <header className="page-head">
        <h1>Devices</h1>
        <button className="primary" onClick={p.onCreate} disabled={!sdk?.found}>
          Create device
        </button>
      </header>

      {sdk && sdk.problems.length > 0 && (
        <div className="banner warn">
          <div>
            <strong>
              {sdk.found ? "Finish setting up the Android SDK" : "Android SDK not found"}
            </strong>
            <ul>
              {sdk.problems.map((t) => (
                <li key={t}>{t}</li>
              ))}
            </ul>
          </div>
          <button className="link" onClick={p.onSettings}>
            Open settings
          </button>
        </div>
      )}

      {accel && (
        <p className={accel.available ? "note ok" : "note bad"}>
          {accel.available
            ? `Hardware acceleration is on. ${accel.detail}`
            : `Hardware acceleration is not available, so x86_64 devices may not start. ${accel.detail}`}
        </p>
      )}

      {p.devices.length === 0 ? (
        <div className="empty">
          <p>No virtual devices yet.</p>
          <p className="muted">
            Create a Lite phone to run APKs on a real Android system.
          </p>
          <button className="primary" onClick={p.onCreate} disabled={!sdk?.found}>
            Create device
          </button>
        </div>
      ) : (
        <table className="grid">
          <thead>
            <tr>
              <th>Name</th>
              <th>Android</th>
              <th className="num">CPU cores</th>
              <th className="num">RAM</th>
              <th className="num">Screen</th>
              <th>Status</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {p.devices.map((d) => {
              const st = p.statuses[d.name];
              const state = st?.state ?? "stopped";
              const busy = !!p.busy[d.name];
              return (
                <tr key={d.name}>
                  <td>
                    <div className="name">{d.name}</div>
                    {st?.serial && <div className="muted small">{st.serial}</div>}
                  </td>
                  <td>
                    {d.android}
                    <div className="muted small">
                      {d.abi}
                      {d.play_store ? ", Google Play" : ""}
                    </div>
                  </td>
                  <td className="num">{d.cpus}</td>
                  <td className="num">{d.ram_mb ? `${d.ram_mb} MB` : "–"}</td>
                  <td className="num">{d.width ? `${d.width}×${d.height}` : "–"}</td>
                  <td className="status-cell">
                    <span className={`pill ${state}`}>{state}</span>
                    {state === "starting" && st && (
                      <>
                        <BootTrack progress={st.progress} />
                        <div className="muted small">{st.stage}</div>
                      </>
                    )}
                    {state === "running" && !st?.managed && (
                      <div className="muted small">Started outside DroidLite</div>
                    )}
                  </td>
                  <td className="actions">
                    {state === "stopped" ? (
                      <button
                        className="primary"
                        disabled={busy || !sdk?.emulator}
                        onClick={() => p.onStart(d.name)}
                      >
                        {busy ? "Starting…" : "Start"}
                      </button>
                    ) : (
                      <>
                        {state === "running" && (
                          <button onClick={() => p.onInstall(d.name)}>Install APK</button>
                        )}
                        <button
                          disabled={busy || state === "stopping"}
                          onClick={() => p.onStop(d.name)}
                        >
                          {state === "stopping" || busy ? "Stopping…" : "Stop"}
                        </button>
                      </>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </section>
  );
}
