# DroidLite

A small Windows desktop manager for **real** Android virtual devices. It does not simulate Android: it drives the official Android Emulator, `avdmanager` and `adb` from the Android SDK, so APKs run on an actual Android system image.

Stack: Tauri 2, React, TypeScript. Runtime dependencies are `react`, `react-dom`, `@tauri-apps/api` and the Tauri dialog plugin.

## Prerequisites (Windows)

1. Node.js 18+ and Rust (stable, MSVC toolchain) with Visual Studio Build Tools. WebView2 ships with Windows 11.
2. Android SDK with these packages, and licenses accepted:
   ```
   sdkmanager "emulator" "platform-tools" "cmdline-tools;latest"
   sdkmanager "system-images;android-30;default;x86_64"
   sdkmanager --licenses
   ```
3. Windows Hypervisor Platform enabled (Windows Features), for hardware acceleration.
4. Java on the PATH or `JAVA_HOME` set, which `avdmanager` needs.

DroidLite never downloads system images itself. It finds the SDK through the folder set in Settings, `ANDROID_SDK_ROOT`, `ANDROID_HOME`, or `%LOCALAPPDATA%\Android\Sdk`.

## Run

```
npm install
npm run tauri dev      # development
npm run tauri build    # NSIS installer
```

## What 0.1 does

- Detects the SDK, emulator, adb, avdmanager, licenses and installed system images, and reports what is missing.
- Creates an AVD (default profile: 2 CPU cores, 1536 MB, 480×854, Android 11/12 x86_64 without Google Play) by running `avdmanager`, then writing the profile values into the AVD’s `config.ini`.
- Starts the emulator with Quick Boot, on a free port, and tracks boot progress through `adb` (emulator, ADB, Android, Ready).
- Stops it through `adb emu kill` so the Quick Boot snapshot is saved, with a forced kill as a fallback.
- Installs APKs with `adb install -r -t` (browse or drag and drop).
- Also shows emulators started outside DroidLite and can stop them.
- Streams emulator output and adb results into the Logs view.

## Layout

```
src/                  React UI (views/: Devices, CreateDevice, InstallApk, Settings, Logs)
src-tauri/src/
  proc.rs             process execution: timeouts, captured output, no console windows
  sdk.rs              SDK and system image detection
  adb.rs              adb wrapper
  avd.rs              list and create AVDs
  emulator.rs         start, stop, boot status
  profiles.rs         device profiles
  settings.rs         settings.json in the app config folder
  lib.rs              Tauri commands
```

## Notes

- `INSTALL_FAILED_NO_MATCHING_ABIS` means the APK has only ARM code. Use an x86_64 or universal build.
- If the emulator exits right after Start, read Logs. A missing hypervisor is the usual cause.
