// Inbound "Share to nearclip" (Android/iOS share sheet). Only called on mobile,
// where the sharehub plugin is registered.
import {
  clearPendingShares,
  getPendingShares,
  onShare,
  type SharePayload,
} from "@sosweetham/tauri-plugin-sharehub-api";

function textOf(payload: SharePayload): string {
  return payload.items
    .map((item) => (item.kind === "url" ? item.url : item.text) ?? "")
    .filter((t) => t.trim().length > 0)
    .join("\n");
}

/** Takes the shared text out of the queue, or "" when nothing is pending. */
export async function consumeSharedText(): Promise<string> {
  try {
    const text = textOf(await getPendingShares());
    if (!text) return "";
    await clearPendingShares();
    return text;
  } catch (err) {
    // A broken share never interrupts the user, but leave a trace for debugging.
    console.warn("share target: could not read pending shares", err);
    return "";
  }
}

/** Fires when a share arrives while the app is already running. */
export function onSharedText(cb: () => void): Promise<() => void> {
  return onShare(cb).catch((err) => {
    console.warn("share target: could not subscribe", err);
    return () => {};
  });
}
