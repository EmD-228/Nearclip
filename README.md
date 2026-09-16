# nearclip

nearclip is a LAN-only, end-to-end encrypted clipboard text sharing app for desktop, built with Tauri v2, Rust and Svelte 5. There is no server and no account: devices on the same local network discover each other with mDNS (`_nearclip._tcp.local.`), pair once by comparing a 6-digit code shown on both screens, and then exchange text over a direct TCP connection (port 47821, with an ephemeral fallback) encrypted with AES-256-GCM.

## Features (MVP)

- Send text manually to one paired device or to all of them.
- Optional auto-sync: every text you copy is sent to your paired devices.
- History of sent and received items, with one-click copy.
- Write received text straight to the local clipboard (toggle).
- Tray icon with close-to-tray behaviour.
- Start at login (autostart).
- System notifications when text is received.

Text only, up to 1 MB per message. Files and mobile platforms are out of scope for now.

## Requirements

- Rust stable, installed with [rustup](https://rustup.rs)
- Node 22
- pnpm 10
- macOS: Xcode Command Line Tools (`xcode-select --install`)
- Windows: Visual Studio C++ Build Tools and the WebView2 runtime (preinstalled on Windows 10/11)

## Development

```sh
pnpm install
pnpm tauri dev
```

`pnpm tauri dev` starts Vite on port 1420 and launches the Tauri window with hot reload for both the Svelte frontend and the Rust backend.

### Running two instances on one machine

Pairing and sending need two devices. To test without a second computer, run a second instance in another terminal:

```sh
# terminal 1
pnpm tauri dev

# terminal 2
pnpm dev:2
```

`pnpm dev:2` uses `src-tauri/tauri.dev2.conf.json`, which overrides the first instance so both can coexist:

- a second bundle identifier, `com.edomatch.nearclip.dev2`, so it gets its own app data directory (identity, pairings, settings, history);
- a separate Vite port, 1422;
- a separate Cargo target directory, `src-tauri/target-2`, so the two builds do not lock each other;
- the TCP listener finds port 47821 already taken and falls back to an ephemeral port, which is advertised through mDNS.

The two windows then discover each other on the loopback network and can be paired like two real devices.

## Tests and checks

```sh
# Rust
cd src-tauri && cargo test && cargo clippy --all-targets

# Frontend
pnpm check
pnpm build
```

## Building installers

```sh
pnpm tauri build
```

This produces a `.dmg` on macOS and NSIS / MSI installers on Windows. Windows installers must be built on Windows.

Code signing and notarization are not configured yet. On macOS, users need to right-click the app and choose Open the first time. On Windows, SmartScreen shows a warning that must be dismissed with "More info" then "Run anyway".

## Security model

- Each device has a long-lived Ed25519 identity key. The fingerprint shown in Settings is derived from its public key.
- Pairing runs an X25519 ECDH exchange with a commitment step, so neither side can pick its ephemeral key after seeing the other's.
- The shared secret is expanded with HKDF bound to the full pairing transcript (both identity keys, both ephemeral keys, both nonces), and each side signs that transcript with its Ed25519 identity key so the pairing is tied to the identity that gets stored.
- A 6-digit short authentication string (SAS) is derived from that transcript and displayed on both screens. Pairing only completes when both users confirm the codes match, which defeats an active man-in-the-middle on the local network.
- Every connection after pairing derives a fresh per-connection session key from the paired secret. Payloads are encrypted with AES-256-GCM.
- Pairing keys are currently stored in plaintext JSON under the app data directory (macOS: `~/Library/Application Support/com.edomatch.nearclip/`). Moving them to the OS keychain is a planned follow-up.

## Troubleshooting

- **macOS asks for Local Network access.** Allow it. If you refused, enable it under System Settings > Privacy & Security > Local Network.
- **macOS firewall prompt.** When the firewall is on, macOS asks whether nearclip may accept incoming connections. Choose Allow, or the other device cannot reach you.
- **Windows Firewall.** Allow nearclip on Private networks when prompted. If the network is marked Public, either switch it to Private or add a manual rule.
- **Devices do not see each other.** Guest, hotel and many office Wi-Fi networks isolate clients and block mDNS. Both computers must be on the same normal Wi-Fi or wired network.

## Project layout

```
src/                          Svelte 5 frontend
  App.svelte                  shell: sidebar, current view, dialogs, event listeners
  app.css                     Tailwind v4 entry and base styles
  lib/api.ts                  one function per Tauri command and per Rust event
  lib/types.ts                TypeScript mirrors of the Rust payload types
  lib/stores/                 rune-based stores (devices, history, settings, pairing, toasts, confirm)
  lib/components/             Sidebar, DeviceCard, PairingDialog, Toast, Toggle, ConfirmDialog
  lib/views/                  Devices, Send, History, Settings

src-tauri/src/                Rust backend
  lib.rs                      app setup, plugin registration, background tasks
  error.rs                    AppError type shared by all modules and commands
  commands.rs                 Tauri commands exposed to the frontend
  state.rs                    shared app state, settings and persisted data
  identity.rs                 Ed25519 device identity and fingerprint
  crypto.rs                   X25519, HKDF, SAS, AES-256-GCM primitives
  protocol.rs                 wire messages and framing
  discovery.rs                mDNS advertisement and browsing
  transport.rs                TCP listener and outgoing connections
  pairing.rs                  pairing state machine (commitment, SAS, confirmation)
  session.rs                  per-connection encrypted sessions
  clipboard.rs                clipboard read/write and auto-sync watcher
  tray.rs                     tray icon and menu
```
# Nearclip
