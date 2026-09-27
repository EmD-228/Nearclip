import { plural } from "./format";
import { devices } from "./stores/devices.svelte";
import { toasts } from "./stores/toasts.svelte";
import type { SendResult } from "./types";

/**
 * One toast for the devices that took `what`, one per device that did not.
 * Shared by the text and file paths so a send reads the same either way.
 */
export function report(results: SendResult[], what: string) {
  const okIds = results.filter((r) => r.ok).map((r) => r.deviceId);
  if (results.length === 0) {
    toasts.info(`No device received ${what}.`);
  } else if (okIds.length === 1 && okIds.length === results.length) {
    toasts.success(`Sent ${what} to ${nameOf(okIds[0])}`);
  } else if (okIds.length > 0) {
    toasts.success(`Sent ${what} to ${plural(okIds.length, "device")}`);
  }
  for (const f of results.filter((r) => !r.ok)) {
    toasts.error(`${nameOf(f.deviceId)}: ${f.error ?? "failed to send"}`);
  }
}

function nameOf(deviceId: string): string {
  return devices.byId(deviceId)?.name ?? "Unknown device";
}
