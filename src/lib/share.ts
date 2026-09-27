// Inbound "Share to NearClip" (Android/iOS share sheet) and outbound sharing of
// received files. Only used on mobile, where the sharehub plugin is registered.
import {
  clearPendingShares,
  getPendingShares,
  onShare,
  readSharedItem,
  shareFile,
} from "@sosweetham/tauri-plugin-sharehub-api";
import { acceptable } from "./files";

export interface SharedContent {
  text: string;
  /** Every shared image and file NearClip can send, in the order they came. */
  files: File[];
  /** One message per shared file that was turned down. */
  rejected: string[];
}

/** Takes what another app shared out of the queue; empty when nothing is pending. */
export async function consumeShared(): Promise<SharedContent> {
  try {
    const { items } = await getPendingShares();
    const text = items
      .map((item) => (item.kind === "url" ? item.url : item.text) ?? "")
      .filter((t) => t.trim().length > 0)
      .join("\n");

    // Unlike a picked file, which stays on disk until it is sent, a shared one
    // is copied into memory whole. So judge each by its metadata first, then
    // read them one after another rather than all at once: sharing a dozen
    // videos should not have to fit in the WebView's heap in one go.
    const shared = items
      .filter((item) => item.kind === "image" || item.kind === "file")
      .map((item) => ({
        id: item.id,
        name: item.name ?? "shared-file",
        size: item.size ?? 0,
        type: item.mimeType ?? "",
      }));
    const { kept, rejected } = acceptable(shared);
    const files: File[] = [];
    // Read before clearing: clearing deletes the copied bytes.
    for (const item of kept) {
      files.push(new File([await readSharedItem(item.id)], item.name, { type: item.type }));
    }
    if (items.length > 0) await clearPendingShares();
    return { text, files, rejected };
  } catch (err) {
    // A broken share never interrupts the user, but leave a trace for debugging.
    console.warn("share target: could not read pending shares", err);
    return { text: "", files: [], rejected: [] };
  }
}

/** Fires when a share arrives while the app is already running. */
export function onShared(cb: () => void): Promise<() => void> {
  return onShare(cb).catch((err) => {
    console.warn("share target: could not subscribe", err);
    return () => {};
  });
}

/** Opens the system share sheet for a received file, so it can be opened or saved elsewhere. */
export function shareReceivedFile(path: string, name: string, mime: string): Promise<void> {
  return shareFile(`file://${path}`, { mimeType: mime || undefined, title: name });
}
