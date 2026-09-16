# nearclip

nearclip is a LAN-only, end-to-end encrypted clipboard text sharing app for macOS, Windows and Android, built with Tauri v2, Rust and Svelte 5. There is no server and no account: devices on the same local network discover each other with mDNS (`_nearclip._tcp.local.`), pair once by comparing a 6-digit code shown on both screens, and then exchange text over a direct TCP connection (port 47821, with an ephemeral fallback) encrypted with AES-256-GCM.

## Features (MVP)

- Send text manually to one paired device or to all of them, with a Paste button to grab the current clipboard.
- Optional auto-sync: on desktop every text you copy is sent to your paired devices; on Android, where the clipboard cannot be read in the background, the last copied text is sent each time nearclip opens (the persistent notification has a "Send clipboard" button for that).
- History of sent and received items, with one-click copy.
- Write received text straight to the local clipboard (toggle).
- Pair a phone by scanning a QR code shown on the computer: one scan, no code to compare, and it works even when mDNS discovery fails or addresses change.
- Add a device by IP address when the network blocks mDNS discovery.
- Automatic mDNS discovery is an opt-in setting ("Automatic discovery", experimental): it is unreliable on many Wi-Fi networks, so pairing goes through QR codes or addresses by default. Paired devices show how they were paired rather than an online/offline state.
- Unpairing on one device tells the other to forget the pairing too; if that message cannot be delivered, the stale side drops the pairing the next time it tries to send.
- Android: "Share to nearclip" from any app's share sheet; the text lands in the Send view.
- Desktop: tray icon with close-to-tray behaviour, start at login (autostart).
- System notifications when text is received.

Text only, up to 1 MB per message. Files are out of scope for now. On Android a persistent "Listening for text" notification keeps the app receiving in the background; its Stop button pauses that until the app is next opened.

## Requirements

- Rust stable, installed with [rustup](https://rustup.rs)
- Node 22
- pnpm 10
- macOS: Xcode Command Line Tools (`xcode-select --install`)
- Windows: Visual Studio C++ Build Tools and the WebView2 runtime (preinstalled on Windows 10/11)
- Android: Android Studio with an SDK (platform 36) and an NDK, plus the Rust targets:

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"   # bundled JDK 21
export ANDROID_HOME="$HOME/Library/Android/sdk"
export NDK_HOME="$ANDROID_HOME/ndk/<version>"
```

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

- a second bundle identifier, `com.nearclip.dev2`, so it gets its own app data directory (identity, pairings, settings, history);
- a separate Vite port, 1422;
- a separate Cargo target directory, `src-tauri/target-2`, so the two builds do not lock each other;
- the TCP listener finds port 47821 already taken and falls back to an ephemeral port, which is advertised through mDNS.

The two windows then discover each other on the loopback network and can be paired like two real devices.

### Android

The generated Android project in `src-tauri/gen/android` is committed because it carries hand-made additions that `pnpm tauri android init` would wipe: `NearclipService.kt` (foreground service that keeps the listener alive in the background and holds the multicast lock, without which Android drops mDNS packets), the system-bar and keyboard insets plus the notification permission request in `MainActivity.kt`, and in `AndroidManifest.xml` the network and foreground-service permissions, the `<service>` entry and the share-sheet intent filter. `build.gradle.kts` also has the release signing block and `minSdk = 26`. If you ever regenerate the project, reapply those from git history.

```sh
# debug APK for an arm64 phone (large: unstripped, debuggable)
pnpm tauri android build --debug --target aarch64 --apk --split-per-abi
# -> src-tauri/gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk
```

Install it with `adb install -r <apk>`, or serve it on the LAN (`python3 -m http.server` in the APK folder) and download it from the phone's browser. `pnpm tauri android dev` also works with a phone connected over adb.

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

### Android release APK

Release builds are signed with a keystore that is not in git. Create it once:

```sh
cd src-tauri/gen/android
keytool -genkeypair -v -keystore nearclip-release.jks -alias nearclip \
  -keyalg RSA -keysize 2048 -validity 10000
cat > keystore.properties <<EOF
storeFile=nearclip-release.jks
storePassword=<password>
keyAlias=nearclip
keyPassword=<password>
EOF
```

Back up the `.jks` and its password: every future update must be signed with the same key or Android refuses to install it over the previous version. Then:

```sh
pnpm tauri android build --target aarch64 --apk --split-per-abi
# -> src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release.apk
```

## Security model

- Each device has a long-lived Ed25519 identity key. The fingerprint shown in Settings is derived from its public key.
- Pairing runs an X25519 ECDH exchange with a commitment step, so neither side can pick its ephemeral key after seeing the other's.
- The shared secret is expanded with HKDF bound to the full pairing transcript (both identity keys, both ephemeral keys, both nonces), and each side signs that transcript with its Ed25519 identity key so the pairing is tied to the identity that gets stored.
- A 6-digit short authentication string (SAS) is derived from that transcript and displayed on both screens. Pairing only completes when both users confirm the codes match, which defeats an active man-in-the-middle on the local network.
- QR pairing replaces the code comparison with a visual channel: the QR carries the computer's public key and a one-time token valid for 5 minutes. The phone checks the key it connects to against the QR, the computer checks the token, and both sides then confirm on their own.
- Every connection after pairing derives a fresh per-connection session key from the paired secret. Payloads are encrypted with AES-256-GCM.
- Pairing keys are currently stored in plaintext JSON under the app data directory (macOS: `~/Library/Application Support/com.nearclip/`). Moving them to the OS keychain is a planned follow-up.

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
