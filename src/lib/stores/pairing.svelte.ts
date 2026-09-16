import { api } from "../api";
import type { PairingCode, PairingRequest, PairingResult, PairingRole } from "../types";

/**
 * - request: a remote device asked to pair with us (responder side), waiting for the code
 * - waiting: we asked to pair (initiator side), connecting to the peer
 * - code:       both sides show the same 6-digit code, user must confirm
 * - confirming: this user confirmed, waiting for the other device to confirm
 * - success:    paired, dialog auto-closes
 * - error:      pairing failed or was rejected
 */
export type PairingStatus = "request" | "waiting" | "code" | "confirming" | "success" | "error";

export interface PairingState {
  deviceId: string;
  name: string;
  code: string | null;
  role: PairingRole;
  status: PairingStatus;
  error: string | null;
  /** Started from a scanned QR code: both sides confirm on their own, no code is shown. */
  viaQr: boolean;
}

const SUCCESS_CLOSE_MS = 1500;

class PairingStore {
  current = $state<PairingState | null>(null);
  private closeTimer: ReturnType<typeof setTimeout> | null = null;

  /** Initiator side: user clicked "Pair" on a device card. */
  start(deviceId: string, name: string) {
    return this.initiate(deviceId, name, () => api.startPairing(deviceId));
  }

  /**
   * Initiator side, for a device mDNS cannot see. Until the peer answers, the
   * address stands in for the device id and name: onCode() replaces both, and
   * an early failure comes back as a result whose deviceId is this address.
   */
  startByAddress(addr: string) {
    return this.initiate(addr, addr, () => api.pairByAddress(addr));
  }

  /**
   * Mobile, after scanning the QR code a computer shows. No pairing-code event
   * follows: the only result carries the `id` from the payload, so that is the key.
   */
  startByQr(payload: string) {
    let id: string | null = null;
    try {
      id = new URL(payload).searchParams.get("id");
    } catch {
      // Unparseable payload: the backend rejects it and fail() shows the error.
    }
    return this.initiate(id ?? payload, "your computer", () => api.pairByQr(payload), true);
  }

  private async initiate(key: string, name: string, invoke: () => Promise<void>, viaQr = false) {
    this.clearTimer();
    this.current = {
      deviceId: key,
      name,
      code: null,
      role: "initiator",
      status: "waiting",
      error: null,
      viaQr,
    };
    try {
      await invoke();
    } catch (err) {
      this.fail(key, err);
    }
  }

  /** Responder side: the backend tells us a remote device wants to pair. */
  onRequest(req: PairingRequest) {
    this.clearTimer();
    this.current = {
      deviceId: req.deviceId,
      name: req.name,
      code: null,
      role: "responder",
      status: "request",
      error: null,
      viaQr: false,
    };
  }

  onCode(payload: PairingCode) {
    this.clearTimer();
    this.current = {
      deviceId: payload.deviceId,
      name: payload.name || this.current?.name || "Unknown device",
      code: payload.code,
      role: payload.role,
      status: "code",
      error: null,
      viaQr: false,
    };
  }

  onResult(result: PairingResult) {
    if (!this.current || this.current.deviceId !== result.deviceId) return;
    if (result.ok) {
      this.current = { ...this.current, status: "success", error: null };
      this.clearTimer();
      this.closeTimer = setTimeout(() => this.close(), SUCCESS_CLOSE_MS);
    } else {
      this.current = {
        ...this.current,
        status: "error",
        error: result.error ?? "Pairing failed.",
      };
    }
  }

  /** User confirmed (or denied) that the codes match. */
  async confirm(accepted: boolean) {
    const cur = this.current;
    if (!cur) return;
    try {
      await api.confirmPairing(cur.deviceId, accepted);
      if (!accepted) this.close();
      else this.current = { ...cur, status: "confirming" };
    } catch (err) {
      this.fail(cur.deviceId, err);
    }
  }

  /** User cancelled before a code was shown. */
  async cancel() {
    const cur = this.current;
    if (!cur) return;
    if (cur.status === "code") {
      await this.confirm(false);
      return;
    }
    if (cur.status === "waiting" || cur.status === "request") {
      try {
        await api.cancelPairing(cur.deviceId);
      } catch {
        // Backend may already have dropped the session; closing is still right.
      }
    }
    this.close();
  }

  close() {
    this.clearTimer();
    this.current = null;
  }

  private fail(deviceId: string, err: unknown) {
    const message = typeof err === "string" ? err : err instanceof Error ? err.message : String(err);
    this.current = {
      deviceId,
      name: this.current?.name ?? "Unknown device",
      code: null,
      role: this.current?.role ?? "initiator",
      status: "error",
      error: message,
      viaQr: this.current?.viaQr ?? false,
    };
  }

  private clearTimer() {
    if (this.closeTimer) {
      clearTimeout(this.closeTimer);
      this.closeTimer = null;
    }
  }
}

export const pairing = new PairingStore();
