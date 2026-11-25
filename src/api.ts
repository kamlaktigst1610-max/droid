import { invoke } from "@tauri-apps/api/core";
import type {
  AccelInfo,
  AvdInfo,
  CreateRequest,
  DeviceProfile,
  DeviceStatus,
  LogLine,
  SdkInfo,
  Settings,
} from "./types";

export const inTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const api = {
  detectSdk: () => invoke<SdkInfo>("detect_sdk"),
  checkAcceleration: () => invoke<AccelInfo>("check_acceleration"),
  getProfiles: () => invoke<DeviceProfile[]>("get_profiles"),
  listDevices: () => invoke<AvdInfo[]>("list_devices"),
  deviceStatuses: () => invoke<DeviceStatus[]>("device_statuses"),
  createDevice: (req: CreateRequest) => invoke<string>("create_device", { req }),
  startDevice: (name: string) => invoke<string>("start_device", { name }),
  stopDevice: (name: string) => invoke<void>("stop_device", { name }),
  installApk: (name: string, path: string) =>
    invoke<string>("install_apk", { name, path }),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<Settings>("set_settings", { settings }),
  getLogs: () => invoke<LogLine[]>("get_logs"),
  clearLogs: () => invoke<void>("clear_logs"),
};

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}
