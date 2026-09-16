export interface ConfirmOptions {
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
}

interface PendingConfirm extends ConfirmOptions {
  resolve: (ok: boolean) => void;
}

class ConfirmStore {
  pending = $state<PendingConfirm | null>(null);

  /** Show the confirm dialog and resolve with the user's choice. */
  ask(options: ConfirmOptions): Promise<boolean> {
    this.pending?.resolve(false);
    return new Promise<boolean>((resolve) => {
      this.pending = { ...options, resolve };
    });
  }

  answer(ok: boolean) {
    const p = this.pending;
    this.pending = null;
    p?.resolve(ok);
  }
}

export const confirm = new ConfirmStore();
