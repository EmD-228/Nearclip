// Mobile "Scan QR code": camera permission, scan, then pair with the computer
// that shows the code. Only called on mobile, where the scanner plugin exists.
//
// Why `windowed: true`: with `windowed: false` the plugin puts the camera view
// ABOVE the webview (iOS insertSubview(aboveSubview:), Android addView after the
// webview) and has no native close button, so the user could not cancel. With
// `windowed: true` the camera sits behind a transparent webview and ScanOverlay
// draws the frame and the Cancel button on top.
import {
  Format,
  cancel,
  checkPermissions,
  requestPermissions,
  scan,
} from "@tauri-apps/plugin-barcode-scanner";
import { pairing } from "./stores/pairing.svelte";
import { errorMessage, toasts } from "./stores/toasts.svelte";

class ScannerStore {
  /** True while the camera is up: App hides its layout and shows ScanOverlay. */
  active = $state(false);
}

export const scanner = new ScannerStore();

// Resolves the pending scan when the user cancels. Needed because on Android
// cancel() clears the saved invoke before rejecting it, so scan() never settles.
let stopWaiting: (() => void) | null = null;
let userCancelled = false;

async function cameraAllowed(): Promise<boolean> {
  let state = await checkPermissions();
  if (state === "prompt") state = await requestPermissions();
  return state === "granted";
}

export async function scanPairingQr() {
  if (scanner.active) return;

  try {
    if (!(await cameraAllowed())) {
      toasts.error("Camera access is needed to scan the code");
      return;
    }
  } catch (err) {
    toasts.error(`Could not access the camera: ${errorMessage(err)}`);
    return;
  }

  userCancelled = false;
  scanner.active = true;
  document.documentElement.classList.add("scanning");

  let content: string | null = null;
  try {
    const cancelled = new Promise<null>((resolve) => (stopWaiting = () => resolve(null)));
    const result = await Promise.race([
      scan({ windowed: true, formats: [Format.QRCode] }),
      cancelled,
    ]);
    content = result?.content ?? null;
  } catch (err) {
    if (!userCancelled && !/cancel/i.test(errorMessage(err))) {
      toasts.error(`Could not scan: ${errorMessage(err)}`);
    }
  } finally {
    stopWaiting = null;
    scanner.active = false;
    document.documentElement.classList.remove("scanning");
  }

  if (content === null || userCancelled) return;
  // The backend validates the payload; a foreign code comes back as a pairing error.
  void pairing.startByQr(content);
}

export async function cancelScan() {
  if (!scanner.active) return;
  userCancelled = true;
  try {
    await cancel();
  } catch {
    // The camera may already be gone; the UI still has to come back.
  }
  stopWaiting?.();
}
