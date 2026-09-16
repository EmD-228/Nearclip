<script lang="ts">
  import { onMount } from "svelte";
  import { isTauri } from "@tauri-apps/api/core";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { events } from "./lib/api";
  import ConfirmDialog from "./lib/components/ConfirmDialog.svelte";
  import PairingDialog from "./lib/components/PairingDialog.svelte";
  import PairQrDialog from "./lib/components/PairQrDialog.svelte";
  import ScanOverlay from "./lib/components/ScanOverlay.svelte";
  import { scanner } from "./lib/scan.svelte";
  import { qr } from "./lib/stores/qr.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Toast from "./lib/components/Toast.svelte";
  import { devices } from "./lib/stores/devices.svelte";
  import { history } from "./lib/stores/history.svelte";
  import { pairing } from "./lib/stores/pairing.svelte";
  import { settings } from "./lib/stores/settings.svelte";
  import { errorMessage, toasts } from "./lib/stores/toasts.svelte";
  import { consumeSharedText, onSharedText } from "./lib/share";
  import { appendDraft } from "./lib/format";
  import type { SendTarget } from "./lib/types";
  import type { View } from "./lib/view";
  import DevicesView from "./lib/views/DevicesView.svelte";
  import HistoryView from "./lib/views/HistoryView.svelte";
  import SendView from "./lib/views/SendView.svelte";
  import SettingsView from "./lib/views/SettingsView.svelte";

  const VIEWS: View[] = ["devices", "send", "history", "settings"];
  // `#send` etc. picks the initial view; handy for previewing a view in `pnpm dev`.
  const initialView = (location.hash.slice(1) || "devices") as View;
  let view = $state<View>(VIEWS.includes(initialView) ? initialView : "devices");
  // Kept here so the Send view survives navigation.
  let sendTarget = $state<SendTarget>("all");
  let sendDraft = $state("");

  function openSend(deviceId: string) {
    sendTarget = deviceId;
    view = "send";
  }

  const inTauri = isTauri();

  // Text shared from another app lands in the Send view, ready to send.
  async function applySharedText() {
    const text = await consumeSharedText();
    if (!text) return;
    sendDraft = appendDraft(sendDraft, text);
    view = "send";
  }

  async function init() {
    const loads: [string, () => Promise<unknown>][] = [
      ["identity", () => settings.loadIdentity()],
      ["settings", () => settings.load()],
      ["devices", () => devices.refresh()],
      ["history", () => history.refresh()],
    ];
    const results = await Promise.allSettled(loads.map(([, fn]) => fn()));
    if (!inTauri) return; // Plain browser (pnpm dev): invoke is unavailable, show empty states.
    results.forEach((r, i) => {
      if (r.status === "rejected") {
        toasts.error(`Could not load ${loads[i][0]}: ${errorMessage(r.reason)}`);
      }
    });
  }

  onMount(() => {
    const subscriptions: Promise<UnlistenFn>[] = [
      events.devicesChanged((list) => devices.set(list)),
      events.pairingRequest((req) => pairing.onRequest(req)),
      events.pairingCode((code) => pairing.onCode(code)),
      events.pairingResult((result) => {
        pairing.onResult(result);
        if (result.ok) {
          qr.close();
          toasts.success(`Paired with ${devices.byId(result.deviceId)?.name ?? "device"}`);
        }
        devices.refresh().catch(() => {});
      }),
      events.clipboardReceived((item) => {
        history.upsert(item);
        toasts.info(`Received text from ${item.peerName}`);
      }),
      events.clipboardSent(({ item }) => history.upsert(item)),
      events.settingsChanged((s) => settings.set(s)),
      events.appError(({ message }) => toasts.error(message)),
    ].map((p) => p.catch(() => (() => {}) as UnlistenFn));

    void init().then(() => {
      // The share sheet only exists on mobile; the plugin is not even registered on desktop.
      if (inTauri && !settings.isDesktop) {
        subscriptions.push(onSharedText(() => void applySharedText()));
        void applySharedText();
      }
    });

    return () => {
      for (const sub of subscriptions) sub.then((unlisten) => unlisten());
    };
  });
</script>

<!-- While scanning, the camera shows through the transparent webview behind this layout. -->
<div class="flex h-full min-h-0 {scanner.active ? 'invisible' : ''}">
  <Sidebar {view} onNavigate={(v) => (view = v)} />
  <!-- Bottom padding below `sm` keeps content clear of the fixed tab bar (h-14 + safe area). -->
  <main
    class="min-w-0 flex-1 overflow-y-auto pb-[calc(3.5rem+env(safe-area-inset-bottom))] sm:pb-0"
  >
    {#if view === "devices"}
      <DevicesView onSend={openSend} />
    {:else if view === "send"}
      <SendView bind:target={sendTarget} bind:text={sendDraft} />
    {:else if view === "history"}
      <HistoryView />
    {:else}
      <SettingsView />
    {/if}
  </main>
</div>

<PairingDialog />
<PairQrDialog />
<ScanOverlay />
<ConfirmDialog />
<Toast />
