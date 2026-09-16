/** Desktop "Show QR code" dialog. App closes it once a pairing succeeds. */
class QrStore {
  open = $state(false);

  show() {
    this.open = true;
  }

  close() {
    this.open = false;
  }
}

export const qr = new QrStore();
