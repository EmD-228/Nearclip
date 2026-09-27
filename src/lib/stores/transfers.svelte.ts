/**
 * The files waiting to be sent, and the one going out right now.
 *
 * This lives outside the Send view because a transfer outlives it: the view is
 * destroyed the moment the user opens Devices or History, and a queue of photos
 * takes minutes. Keeping the queue here means progress is still there when they
 * come back, and it can be cancelled from anywhere.
 */
import { acceptable, CANCELLED, sendFile } from "../files";
import { report } from "../report";
import { errorMessage, toasts } from "./toasts.svelte";
import type { SendTarget } from "../types";

export interface QueuedFile {
  file: File;
  /** Fraction sent while this file is going out; 0 otherwise. */
  fraction: number;
  sending: boolean;
  /** Why the last attempt failed, so a file left behind says why. */
  error: string | null;
}

class Transfers {
  files = $state<QueuedFile[]>([]);
  /** True while the queue is being sent. */
  running = $state(false);
  // Plain field: read between chunks, never rendered.
  private cancelled = false;

  /** Queues what can be sent and says why about the rest. */
  add(picked: Iterable<File>) {
    const { kept, rejected } = acceptable(picked);
    for (const message of rejected) toasts.error(message);
    this.files = [
      ...this.files,
      ...kept.map((file) => ({ file, fraction: 0, sending: false, error: null })),
    ];
  }

  remove(entry: QueuedFile) {
    this.files = this.files.filter((f) => f !== entry);
  }

  /** Stops after the current chunk; whatever is left stays queued. */
  cancel() {
    this.cancelled = true;
  }

  /**
   * Sends the queue to `target`, one file at a time: every transfer already
   * fans out to each device, and a phone should not hold several files at once.
   * A file leaves the queue once a device accepted it, so what remains is
   * exactly what still has to go.
   */
  async send(target: SendTarget) {
    this.running = true;
    this.cancelled = false;
    const sent = new Set<QueuedFile>();
    try {
      for (const entry of this.files) {
        if (this.cancelled) break;
        entry.sending = true;
        entry.fraction = 0;
        entry.error = null;
        try {
          const results = await sendFile(
            target,
            entry.file,
            (fraction) => (entry.fraction = fraction),
            () => this.cancelled,
          );
          report(results, entry.file.name);
          if (results.some((r) => r.ok)) sent.add(entry);
          else entry.error = results.find((r) => r.error)?.error ?? "failed to send";
        } catch (err) {
          const message = errorMessage(err);
          entry.error = message;
          if (message !== CANCELLED) toasts.error(`${entry.file.name}: ${message}`);
        } finally {
          entry.sending = false;
        }
      }
    } finally {
      this.files = this.files.filter((f) => !sent.has(f));
      this.running = false;
    }
  }
}

export const transfers = new Transfers();
