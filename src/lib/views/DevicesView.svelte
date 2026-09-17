<script lang="ts">
  import { ClipboardCopy, MonitorSmartphone, Plus, QrCode, RefreshCw, ScanLine } from "@lucide/svelte";
  import AddByAddressDialog from "../components/AddByAddressDialog.svelte";
  import DeviceCard from "../components/DeviceCard.svelte";
  import { confirm } from "../stores/confirm.svelte";
  import { devices } from "../stores/devices.svelte";
  import { scanPairingQr } from "../scan.svelte";
  import { pairing } from "../stores/pairing.svelte";
  import { qr } from "../stores/qr.svelte";
  import { settings } from "../stores/settings.svelte";
  import { errorMessage, toasts } from "../stores/toasts.svelte";
  import type { DeviceView } from "../types";
  import { api } from "../api";
  import { btn, card, page, pageSubtitle, pageTitle, sectionTitle } from "../ui";

  interface Props {
    onSend: (deviceId: string) => void;
  }

  let { onSend }: Props = $props();

  let addOpen = $state(false);

  // Without discovery only paired devices exist in the UI; unpaired ones are never listed.
  let showAvailable = $derived(settings.settings.discovery);
  let hasDevices = $derived(
    devices.paired.length > 0 || (showAvailable && devices.available.length > 0),
  );

  // Dismissed for this session only.
  let autoSyncTipDismissed = $state(false);
  let showAutoSyncTip = $derived(
    devices.paired.length > 0 &&
      !settings.settings.autoSync &&
      !autoSyncTipDismissed,
  );

  async function enableAutoSync() {
    try {
      await settings.save({ autoSync: true });
      toasts.success("Auto-sync clipboard is on");
    } catch (err) {
      toasts.error(`Could not turn on auto-sync: ${errorMessage(err)}`);
    }
  }

  async function refresh() {
    try {
      await devices.refresh();
    } catch (err) {
      toasts.error(`Could not refresh devices: ${errorMessage(err)}`);
    }
  }

  function pair(device: DeviceView) {
    void pairing.start(device.deviceId, device.name);
  }

  async function unpair(device: DeviceView) {
    const ok = await confirm.ask({
      title: `Unpair ${device.name}?`,
      message:
        "This device will no longer be able to send text to you, and you will need to pair again to send to it.",
      confirmLabel: "Unpair",
      danger: true,
    });
    if (!ok) return;
    try {
      await api.unpair(device.deviceId);
      toasts.info(`Unpaired ${device.name}`);
      await devices.refresh();
    } catch (err) {
      toasts.error(`Could not unpair: ${errorMessage(err)}`);
    }
  }
</script>

<div class={page}>
  <!-- Below `sm` the actions wrap under the title, with Scan QR code stretching to fill the row. -->
  <header class="mb-5 flex flex-wrap items-start justify-between gap-x-4 gap-y-3 sm:mb-6">
    <div>
      <h1 class={pageTitle}>Devices</h1>
      <p class={pageSubtitle}>Devices paired with this one on your local network.</p>
    </div>
    <div class="flex w-full gap-2 sm:w-auto sm:shrink-0">
      {#if settings.isDesktop}
        <button type="button" class={btn.primary} onclick={() => qr.show()}>
          <QrCode class="size-4" aria-hidden="true" />
          Show QR code
        </button>
      {:else}
        <button type="button" class="{btn.primary} flex-1 sm:flex-none" onclick={scanPairingQr}>
          <ScanLine class="size-4" aria-hidden="true" />
          Scan QR code
        </button>
      {/if}
      <button type="button" class={btn.secondary} onclick={() => (addOpen = true)}>
        <Plus class="size-4" aria-hidden="true" />
        Add by IP
      </button>
      <button
        type="button"
        class={btn.secondary}
        onclick={refresh}
        disabled={devices.loading}
        aria-label="Refresh devices"
      >
        <RefreshCw class="size-4 {devices.loading ? 'animate-spin' : ''}" aria-hidden="true" />
        <span class="hidden sm:inline">Refresh</span>
      </button>
    </div>
  </header>

  <AddByAddressDialog bind:open={addOpen} />

  {#if !hasDevices}
    <div
      class="flex flex-col items-center rounded-lg border border-dashed border-neutral-300 px-5 py-10 text-center sm:px-6 sm:py-14 dark:border-neutral-700"
    >
      <MonitorSmartphone class="size-8 text-neutral-400" aria-hidden="true" />
      <h2 class="mt-4 text-sm font-medium">No devices yet</h2>
      <p class="mt-1 max-w-xs text-sm text-neutral-500 dark:text-neutral-400">
        {#if settings.isDesktop}
          Click Show QR code and scan it with NearClip on your phone, or use Add by IP to pair
          with another computer.
        {:else}
          Open NearClip on your computer, click Show QR code there, then tap Scan QR code here.
        {/if}
        {#if showAvailable}
          Devices on the same Wi-Fi also show up here on their own.
        {/if}
      </p>
    </div>
  {:else}
    <div class="flex flex-col gap-6 sm:gap-7">
      <section aria-labelledby="paired-heading">
        <h2 id="paired-heading" class="{sectionTitle} mb-2">Paired</h2>
        {#if devices.paired.length === 0}
          <p class="text-sm text-neutral-500 dark:text-neutral-400">
            No paired devices yet. Pair one from the list below.
          </p>
        {:else}
          <ul class="flex flex-col gap-2">
            {#each devices.paired as device (device.deviceId)}
              <li>
                <DeviceCard
                  {device}
                  onPair={pair}
                  onUnpair={unpair}
                  onSend={(d) => onSend(d.deviceId)}
                />
              </li>
            {/each}
          </ul>
        {/if}

        {#if showAutoSyncTip}
          <div class="{card} mt-3 flex flex-col gap-3 p-4 sm:flex-row sm:items-start sm:gap-4">
            <div
              class="flex size-9 shrink-0 items-center justify-center rounded-full bg-blue-50 dark:bg-blue-950/60"
            >
              <ClipboardCopy class="size-4 text-blue-600" aria-hidden="true" />
            </div>
            <div class="min-w-0 flex-1">
              <h3 class="text-sm font-medium">Send what you copy, automatically</h3>
              <p class="mt-1 text-sm text-neutral-500 dark:text-neutral-400">
                {#if settings.isDesktop}
                  Turn on Auto-sync clipboard and every text you copy on this computer is sent to
                  your paired devices. Nothing to paste, nothing to click.
                {:else}
                  Turn on Auto-sync clipboard and every time you open NearClip, what you last copied
                  is sent to your paired devices.
                {/if}
              </p>
              <div class="mt-3 flex gap-2">
                <button
                  type="button"
                  class={btn.primary}
                  onclick={enableAutoSync}
                  disabled={settings.saving}
                >
                  Turn on
                </button>
                <button
                  type="button"
                  class={btn.secondary}
                  onclick={() => (autoSyncTipDismissed = true)}
                >
                  Not now
                </button>
              </div>
            </div>
          </div>
        {/if}
      </section>

      {#if showAvailable}
        <section aria-labelledby="available-heading">
          <h2 id="available-heading" class="{sectionTitle} mb-2">Available</h2>
          {#if devices.available.length === 0}
            <p class="text-sm text-neutral-500 dark:text-neutral-400">
              No unpaired devices on the network right now.
            </p>
          {:else}
            <ul class="flex flex-col gap-2">
              {#each devices.available as device (device.deviceId)}
                <li>
                  <DeviceCard
                    {device}
                    onPair={pair}
                    onUnpair={unpair}
                    onSend={(d) => onSend(d.deviceId)}
                  />
                </li>
              {/each}
            </ul>
          {/if}
        </section>
      {/if}
      {#if !settings.isDesktop}
        <p class="text-xs text-neutral-500 dark:text-neutral-400">
          Keep the app open on this phone to receive text.
        </p>
      {/if}
    </div>
  {/if}
</div>
