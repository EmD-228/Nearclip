import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppErrorEvent,
  ClipboardSent,
  DeviceView,
  HistoryItem,
  Identity,
  ListenInfo,
  PairingCode,
  PairingRequest,
  PairingResult,
  SendResult,
  SendTarget,
  Settings,
} from "./types";

// One function per Tauri command. Argument names are camelCase here and
// converted to snake_case Rust parameters by Tauri automatically.
export const api = {
  getIdentity: () => invoke<Identity>("get_identity"),
  setDeviceName: (name: string) => invoke<void>("set_device_name", { name }),

  listDevices: () => invoke<DeviceView[]>("list_devices"),

  startPairing: (deviceId: string) => invoke<void>("start_pairing", { deviceId }),
  confirmPairing: (deviceId: string, accepted: boolean) =>
    invoke<void>("confirm_pairing", { deviceId, accepted }),
  cancelPairing: (deviceId: string) => invoke<void>("cancel_pairing", { deviceId }),
  unpair: (deviceId: string) => invoke<void>("unpair", { deviceId }),

  sendText: (target: SendTarget, text: string) =>
    invoke<SendResult[]>("send_text", { target, text }),
  copyToClipboard: (text: string) => invoke<void>("copy_to_clipboard", { text }),

  getHistory: () => invoke<HistoryItem[]>("get_history"),
  clearHistory: () => invoke<void>("clear_history"),

  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<Settings>("set_settings", { settings }),

  getListenInfo: () => invoke<ListenInfo>("get_listen_info"),
};

// One function per Rust-emitted event. Each returns the unlisten promise.
type Handler<T> = (payload: T) => void;
const on = <T>(name: string) => (cb: Handler<T>): Promise<UnlistenFn> =>
  listen<T>(name, (e) => cb(e.payload));

export const events = {
  devicesChanged: on<DeviceView[]>("devices-changed"),
  pairingRequest: on<PairingRequest>("pairing-request"),
  pairingCode: on<PairingCode>("pairing-code"),
  pairingResult: on<PairingResult>("pairing-result"),
  clipboardReceived: on<HistoryItem>("clipboard-received"),
  clipboardSent: on<ClipboardSent>("clipboard-sent"),
  settingsChanged: on<Settings>("settings-changed"),
  appError: on<AppErrorEvent>("app-error"),
};
