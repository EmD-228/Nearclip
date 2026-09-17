/** Open/closed state for a dialog that carries nothing else. */
class DialogState {
  open = $state(false);

  show() {
    this.open = true;
  }

  close() {
    this.open = false;
  }
}

/** Desktop "Show QR code" dialog. App closes it once a pairing succeeds. */
export const qr = new DialogState();

/** "Add a device by address" form. */
export const addByAddress = new DialogState();

/** How many modals are open; the page behind them is inert while any is. */
class ModalCount {
  count = $state(0);

  get any() {
    return this.count > 0;
  }

  opened() {
    this.count += 1;
  }

  closed() {
    this.count -= 1;
  }
}

export const modals = new ModalCount();
