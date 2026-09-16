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
}

const SUCCESS_CLOSE_MS = 1500;

class PairingStore {
  current = $state<PairingState | null>(null);
  private closeTimer: ReturnType<typeof setTimeout> | null = null;

  /** Initiator side: user clicked "Pair" on a device card. */
  async start(deviceId: string, name: string) {
    this.clearTimer();
    this.current = { deviceId, name, code: null, role: "initiator", status: "waiting", error: null };
    try {
      await api.startPairing(deviceId);
    } catch (err) {
      this.fail(deviceId, err);
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
