export type ToastKind = "info" | "success" | "error";

export interface ToastItem {
  id: number;
  kind: ToastKind;
  text: string;
}

const DISMISS_MS = 4000;
let nextId = 1;

class ToastStore {
  items = $state<ToastItem[]>([]);

  push(kind: ToastKind, text: string) {
    const id = nextId++;
    this.items.push({ id, kind, text });
    setTimeout(() => this.dismiss(id), DISMISS_MS);
    return id;
  }

  dismiss(id: number) {
    this.items = this.items.filter((t) => t.id !== id);
  }

  info(text: string) {
    return this.push("info", text);
  }

  success(text: string) {
    return this.push("success", text);
  }

  error(text: string) {
    return this.push("error", text);
  }
}

export const toasts = new ToastStore();

/** Turn any thrown value (Tauri returns plain strings) into a readable message. */
export function errorMessage(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}
