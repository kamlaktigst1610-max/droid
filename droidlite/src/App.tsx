import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, inTauri } from "./api";
import type {
  AccelInfo,
  AvdInfo,
  DeviceProfile,
  DeviceStatus,
  LogLine,
  SdkInfo,
  Settings,
  View,
} from "./types";
import Devices from "./views/Devices";
import CreateDevice from "./views/CreateDevice";
import InstallApk from "./views/InstallApk";
import SettingsView from "./views/Settings";
import Logs from "./views/Logs";

const NAV: { id: View; label: string }[] = [
  { id: "devices", label: "Devices" },
  { id: "create", label: "Create Device" },
  { id: "install", label: "Install APK" },
  { id: "settings", label: "Settings" },
  { id: "logs", label: "Logs" },
];

export default function App() {
  const [view, setView] = useState<View>("devices");
  const [sdk, setSdk] = useState<SdkInfo | null>(null);
  const [accel, setAccel] = useState<AccelInfo | null>(null);
  const [profiles, setProfiles] = useState<DeviceProfile[]>([]);
  const [devices, setDevices] = useState<AvdInfo[]>([]);
  const [statuses, setStatuses] = useState<Record<string, DeviceStatus>>({});
  const [settings, setSettings] = useState<Settings | null>(null);
  const [logs, setLogs] = useState<LogLine[]>([]);
  const [busy, setBusy] = useState<Record<string, boolean>>({});
  const [error, setError] = useState<string | null>(null);
  const [installTarget, setInstallTarget] = useState<string>("");

  const refreshSdk = useCallback(async () => {
    const s = await api.detectSdk();
    setSdk(s);
    setDevices(await api.listDevices());
    if (s.emulator) {
      api.checkAcceleration().then(setAccel).catch(() => setAccel(null));
    } else {
      setAccel(null);
    }
  }, []);

  const refreshStatuses = useCallback(async () => {
    const list = await api.deviceStatuses();
    const map: Record<string, DeviceStatus> = {};
    list.forEach((s) => (map[s.name] = s));
    setStatuses(map);
    return list;
  }, []);

  // Initial load.
  useEffect(() => {
    if (!inTauri()) return;
    refreshSdk().catch((e) => setError(errorText(e)));
    api.getProfiles().then(setProfiles).catch(() => {});
    api.getSettings().then(setSettings).catch(() => {});
    api.getLogs().then(setLogs).catch(() => {});
    let unlisten: (() => void) | undefined;
    let disposed = false;
    listen<LogLine>("droidlite-log", (e) =>
      setLogs((l) => [...l.slice(-1999), e.payload]),
    ).then((f) => {
      if (disposed) f();
      else unlisten = f;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refreshSdk]);

  // Poll device status: fast while something is starting or running.
  useEffect(() => {
    if (!sdk?.adb) return;
    let stop = false;
    let timer = 0;
    const tick = async () => {
      let delay = 5000;
      try {
        const list = await refreshStatuses();
        if (list.some((s) => s.state !== "stopped")) delay = 1500;
      } catch {
        /* keep polling */
      }
      if (!stop) timer = window.setTimeout(tick, delay);
    };
    tick();
    return () => {
      stop = true;
      window.clearTimeout(timer);
    };
  }, [sdk?.adb, devices.length, refreshStatuses]);

  const act = useCallback(
    async (name: string, fn: () => Promise<unknown>) => {
      setBusy((b) => ({ ...b, [name]: true }));
      setError(null);
      try {
        await fn();
        await refreshStatuses().catch(() => {});
      } catch (e) {
        setError(errorText(e));
      } finally {
        setBusy((b) => ({ ...b, [name]: false }));
      }
    },
    [refreshStatuses],
  );

  const sdkReady = !!(sdk?.found && sdk.emulator && sdk.adb);
  const runningCount = Object.values(statuses).filter((s) => s.state !== "stopped").length;

  if (!inTauri()) {
    return (
      <div className="standalone">
        <h1>DroidLite</h1>
        <p>
          This page needs the desktop shell to reach the emulator. Start it with{" "}
          <code>npm run tauri dev</code>.
        </p>
      </div>
    );
  }

  return (
    <div className="app">
      <nav className="rail" aria-label="Main">
        <div className="brand">DroidLite</div>
        <ul>
          {NAV.map((n) => (
            <li key={n.id}>
              <button
                className={view === n.id ? "nav active" : "nav"}
                aria-current={view === n.id ? "page" : undefined}
                onClick={() => setView(n.id)}
              >
                {n.label}
                {n.id === "devices" && runningCount > 0 && (
                  <span className="count" aria-label={`${runningCount} active`}>
                    {runningCount}
                  </span>
                )}
              </button>
            </li>
          ))}
        </ul>
        <div className="rail-foot">
          <span className={sdkReady ? "dot ok" : "dot bad"} aria-hidden="true" />
          {sdk === null ? "Checking SDK…" : sdkReady ? "SDK ready" : "SDK needs setup"}
        </div>
      </nav>

      <main className="main">
        {error && (
          <div className="banner error" role="alert">
            <span>{error}</span>
            <button className="link" onClick={() => setError(null)}>
              Dismiss
            </button>
          </div>
        )}

        {view === "devices" && (
          <Devices
            sdk={sdk}
            accel={accel}
            devices={devices}
            statuses={statuses}
            busy={busy}
            onStart={(n) => act(n, () => api.startDevice(n))}
            onStop={(n) => act(n, () => api.stopDevice(n))}
            onCreate={() => setView("create")}
            onSettings={() => setView("settings")}
            onInstall={(n) => {
              setInstallTarget(n);
              setView("install");
            }}
          />
        )}
        {view === "create" && (
          <CreateDevice
            sdk={sdk}
            profiles={profiles}
            existing={devices.map((d) => d.name)}
            onCreated={async () => {
              setDevices(await api.listDevices());
              setView("devices");
            }}
            onRescan={() => refreshSdk().catch((e) => setError(errorText(e)))}
          />
        )}
        {view === "install" && (
          <InstallApk
            devices={devices}
            statuses={statuses}
            initialDevice={installTarget}
          />
        )}
        {view === "settings" && settings && (
          <SettingsView
            settings={settings}
            sdk={sdk}
            accel={accel}
            onSave={async (s) => {
              setSettings(await api.setSettings(s));
              await refreshSdk();
            }}
            onRescan={() => refreshSdk().catch((e) => setError(errorText(e)))}
          />
        )}
        {view === "logs" && (
          <Logs
            logs={logs}
            onClear={async () => {
              await api.clearLogs();
              setLogs([]);
            }}
          />
        )}
      </main>
    </div>
  );
}
