// Mirrors of the Rust serde types (all fields camelCase on the wire).
// Keep in sync with src-tauri/src/state.rs and src-tauri/src/commands.rs.

export interface Identity {
  deviceId: string;
  name: string;
  /** Grouped hex fingerprint, e.g. "ab12 cd34 ef56 ..." */
  fingerprint: string;
  /** Base64 Ed25519 public key */
  publicKey: string;
  /** Rust std::env::consts::OS: "macos" | "windows" | "linux" | "android" | "ios" */
  platform: string;
  /** True on desktop (tray, autostart, close-to-tray and clipboard auto-sync exist only there) */
  desktop: boolean;
}

/** How a pairing was established. Shown instead of an online/offline state. */
export type PairedVia = "discovery" | "address" | "qr";

export interface DeviceView {
  deviceId: string;
  name: string;
  paired: boolean;
  /** "192.168.1.20:47821" when known (last address used), otherwise null */
  addr: string | null;
  /** Set for paired devices only */
  via: PairedVia | null;
}

export type Direction = "sent" | "received";

export interface HistoryItem {
  id: string;
  direction: Direction;
  peerId: string;
  peerName: string;
  /** Empty for a file transfer */
  text: string;
  /** Unix epoch milliseconds */
  tsMs: number;
  ok: boolean;
  file?: FileMeta;
}

export interface FileMeta {
  name: string;
  /** Bytes */
  size: number;
  /** MIME type as reported by the sender, may be empty */
  mime: string;
  /** Where a received file was saved; absent for sent files */
  path?: string;
}

export interface Settings {
  deviceName: string;
  autoSync: boolean;
  writeReceivedToClipboard: boolean;
  notifyOnReceive: boolean;
  closeToTray: boolean;
  autostart: boolean;
  /** mDNS automatic discovery. Experimental, off by default. */
  discovery: boolean;
}

export interface SendResult {
  deviceId: string;
  ok: boolean;
  error: string | null;
}

export type PairingRole = "initiator" | "responder";

export interface PairingRequest {
  deviceId: string;
  name: string;
}

export interface PairingCode {
  deviceId: string;
  name: string;
  /** Six digits, e.g. "483912" */
  code: string;
  role: PairingRole;
}

export interface PairingResult {
  deviceId: string;
  ok: boolean;
  error: string | null;
}

export interface ListenInfo {
  port: number;
  addrs: string[];
}

/** QR code shown on a desktop so a phone can pair with one scan. */
export interface PairQr {
  /** Inline <svg> markup of the QR code, dark modules on a light background */
  svg: string;
  /** How long the code stays valid, in milliseconds */
  ttlMs: number;
}

export interface ClipboardSent {
  item: HistoryItem;
  results: SendResult[];
}

export interface AppErrorEvent {
  message: string;
}

/** Target for send_text: a device id, or "all" for every paired device. */
export type SendTarget = "all" | (string & {});
