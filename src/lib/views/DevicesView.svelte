<script lang="ts">
  import { Plus, RefreshCw, WifiOff } from "@lucide/svelte";
  import AddByAddressDialog from "../components/AddByAddressDialog.svelte";
  import DeviceCard from "../components/DeviceCard.svelte";
  import { confirm } from "../stores/confirm.svelte";
  import { devices } from "../stores/devices.svelte";
  import { pairing } from "../stores/pairing.svelte";
  import { settings } from "../stores/settings.svelte";
  import { errorMessage, toasts } from "../stores/toasts.svelte";
  import type { DeviceView } from "../types";
  import { api } from "../api";
  import { btn, page, pageSubtitle, pageTitle, sectionTitle } from "../ui";

  interface Props {
    onSend: (deviceId: string) => void;
  }

  let { onSend }: Props = $props();

  let addOpen = $state(false);

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
  <header class="mb-5 flex items-start justify-between gap-4 sm:mb-6">
    <div>
      <h1 class={pageTitle}>Devices</h1>
      <p class={pageSubtitle}>Devices on your network running nearclip.</p>
    </div>
    <div class="flex shrink-0 gap-2">
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

  {#if devices.list.length === 0}
    <div
      class="flex flex-col items-center rounded-lg border border-dashed border-neutral-300 px-5 py-10 text-center sm:px-6 sm:py-14 dark:border-neutral-700"
    >
      <WifiOff class="size-8 text-neutral-400" aria-hidden="true" />
      <h2 class="mt-4 text-sm font-medium">No devices found yet</h2>
      <p class="mt-1 max-w-xs text-sm text-neutral-500 dark:text-neutral-400">
        Open nearclip on another device connected to the same Wi-Fi network. It should appear
        here within a few seconds.
        {#if !settings.isDesktop}
          Keep the app open on this phone to receive text.
        {/if}
      </p>
    </div>
  {:else}
    <div class="flex flex-col gap-6 sm:gap-7">
      <section aria-labelledby="paired-heading">
        <h2 id="paired-heading" class="{sectionTitle} mb-2">Paired</h2>
        {#if devices.paired.length === 0}
          <p class="text-sm text-neutral-500 dark:text-neutral-400">
            No paired devices. Pair one from the list below.
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
      </section>

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
      {#if !settings.isDesktop}
        <p class="text-xs text-neutral-500 dark:text-neutral-400">
          Keep the app open on this phone to receive text.
        </p>
      {/if}
    </div>
  {/if}
</div>
