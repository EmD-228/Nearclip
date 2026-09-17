// Inbound "Share to NearClip" (Android/iOS share sheet) and outbound sharing of
// received files. Only used on mobile, where the sharehub plugin is registered.
import {
  clearPendingShares,
  getPendingShares,
  onShare,
  readSharedItem,
  shareFile,
} from "@sosweetham/tauri-plugin-sharehub-api";

export interface SharedContent {
  text: string;
  /** The first shared image or file, if any. */
  file: File | null;
}

/** Takes what another app shared out of the queue; empty when nothing is pending. */
export async function consumeShared(): Promise<SharedContent> {
  try {
    const { items } = await getPendingShares();
    const text = items
      .map((item) => (item.kind === "url" ? item.url : item.text) ?? "")
      .filter((t) => t.trim().length > 0)
      .join("\n");
    const shared = items.find((item) => item.kind === "image" || item.kind === "file");
    // Read before clearing: clearing deletes the copied bytes.
    const file = shared
      ? new File([await readSharedItem(shared.id)], shared.name ?? "shared-file", {
          type: shared.mimeType ?? "",
        })
      : null;
    if (items.length > 0) await clearPendingShares();
    return { text, file };
  } catch (err) {
    // A broken share never interrupts the user, but leave a trace for debugging.
    console.warn("share target: could not read pending shares", err);
    return { text: "", file: null };
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
