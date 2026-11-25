export interface SystemImage {
  id: string;
  api: number;
  android: string;
  variant: string;
  abi: string;
  path: string;
  recommended: boolean;
}

export interface SdkInfo {
  found: boolean;
  root: string | null;
  source: string;
  emulator: string | null;
  adb: string | null;
  avdmanager: string | null;
  licenses_accepted: boolean;
  system_images: SystemImage[];
  problems: string[];
}

export interface AccelInfo {
  available: boolean;
  detail: string;
}

export interface DeviceProfile {
  id: string;
  label: string;
  description: string;
  cpus: number;
  ram_mb: number;
  width: number;
  height: number;
  density: number;
}

export interface AvdInfo {
  name: string;
  dir: string;
  image: string;
  abi: string;
  api: number | null;
  android: string;
  play_store: boolean;
  cpus: number;
  ram_mb: number;
  width: number;
  height: number;
  density: number;
}

export type RunState = "stopped" | "starting" | "running" | "stopping";

export interface DeviceStatus {
  name: string;
  state: RunState;
  stage: string;
  progress: number;
  serial: string | null;
  managed: boolean;
}

export interface CreateRequest {
  name: string;
  image_id: string;
  cpus: number;
  ram_mb: number;
  width: number;
  height: number;
  density: number;
}

export interface Settings {
  sdk_path: string | null;
  gpu_mode: string;
  quick_boot: boolean;
}

export interface LogLine {
  ts: number;
  level: string;
  msg: string;
}

export type View = "devices" | "create" | "install" | "settings" | "logs";
