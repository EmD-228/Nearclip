// Fake Tauri runtime with demo data, loaded before the app bundle so the
// frontend renders in a plain browser. A screenshot fixture, not a contract:
// if a command is renamed in src-tauri/src/commands.rs, update the keys below.
// `?platform=android` switches to the phone variant.
(() => {
  const phone = new URLSearchParams(location.search).get("platform") === "android";
  const now = Date.now();
  const min = 60_000;

  const mac = { deviceId: "1c064ee1c52159d4b447e4678db056de", name: "MacBook Air" };
  const pixel = { deviceId: "9d818416e44f87c985e1593754068dfe", name: "Pixel 6 Pro" };
  const self = phone ? pixel : mac;
  const other = phone ? mac : pixel;

  const devices = phone
    ? [{ ...mac, paired: true, addr: "192.168.1.148:47821", via: "qr" }]
    : [
        { ...pixel, paired: true, addr: "192.168.1.67:47821", via: "qr" },
        { deviceId: "5b2e0c7a91d34f6e8a0b1c2d3e4f5a6b", name: "Office PC", paired: true, addr: "192.168.1.31:47821", via: "address" },
      ];

  const history = [
    { id: "h1", direction: "received", peerId: other.deviceId, peerName: other.name, text: "https://github.com/EmD-228/Nearclip/releases/latest", tsMs: now - 2 * min, ok: true },
    { id: "h2", direction: "sent", peerId: "all", peerName: "All paired devices", text: "Wi-Fi guest password: blue-harbor-2291", tsMs: now - 14 * min, ok: true },
    { id: "h3", direction: "received", peerId: other.deviceId, peerName: other.name, text: "Meeting moved to 3:30 pm, room B2. Bring the printed contract.", tsMs: now - 52 * min, ok: true },
    { id: "h4", direction: "sent", peerId: "office", peerName: "Office PC", text: "ssh deploy@10.0.4.12 -p 2222", tsMs: now - 180 * min, ok: true },
  ];

  const responses = {
    get_identity: {
      ...self,
      fingerprint: self.deviceId.match(/.{4}/g).join(" "),
      publicKey: "",
      platform: phone ? "android" : "macos",
      desktop: !phone,
    },
    list_devices: devices,
    get_history: history,
    get_settings: {
      deviceName: self.name,
      autoSync: true,
      writeReceivedToClipboard: true,
      notifyOnReceive: true,
      closeToTray: true,
      autostart: false,
      discovery: false,
    },
    get_listen_info: { port: 47821, addrs: [phone ? "192.168.1.67" : "192.168.1.148"] },
  };

  let nextId = 1;
  window.isTauri = true;
  window.__TAURI_INTERNALS__ = {
    transformCallback: () => nextId++,
    unregisterCallback: () => {},
    convertFileSrc: (path) => path,
    // Event subscriptions ("plugin:event|listen") just need an id back.
    invoke: async (cmd) => (cmd.startsWith("plugin:") ? nextId++ : (responses[cmd] ?? null)),
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
})();
