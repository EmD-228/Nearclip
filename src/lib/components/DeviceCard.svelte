<script lang="ts">
  import { Link2, Send, Unlink } from "@lucide/svelte";
  import type { DeviceView, PairedVia } from "../types";
  import { btn, card } from "../ui";

  interface Props {
    device: DeviceView;
    onPair: (device: DeviceView) => void;
    onUnpair: (device: DeviceView) => void;
    onSend: (device: DeviceView) => void;
  }

  let { device, onPair, onUnpair, onSend }: Props = $props();

  const VIA_LABEL: Record<PairedVia, string> = {
    qr: "Paired with QR code",
    address: "Paired by IP address",
    discovery: "Paired via discovery",
  };

  let pairedLabel = $derived(device.paired ? (device.via ? VIA_LABEL[device.via] : "Paired") : null);
</script>

<div class="{card} flex items-center gap-3 px-4 py-3">
  <div class="min-w-0 flex-1">
    <p class="truncate text-sm font-medium">{device.name}</p>
    {#if pairedLabel || device.addr}
      <div class="mt-1 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
        {#if pairedLabel}
          <span
            class="shrink-0 rounded-full bg-neutral-100 px-2 py-px text-[11px] font-medium text-neutral-600 dark:bg-neutral-800 dark:text-neutral-300"
          >
            {pairedLabel}
          </span>
        {/if}
        {#if device.addr}
          <span class="min-w-0 truncate font-mono text-xs text-neutral-500 select-text dark:text-neutral-400">
            {device.addr}
          </span>
        {/if}
      </div>
    {/if}
  </div>

  <div class="flex shrink-0 items-center gap-1 sm:gap-1.5">
    {#if device.paired}
      <button type="button" class={btn.primary} onclick={() => onSend(device)}>
        <Send class="size-4" aria-hidden="true" />
        Send
      </button>
      <button
        type="button"
        class={btn.ghost}
        onclick={() => onUnpair(device)}
        aria-label="Unpair {device.name}"
        title="Unpair"
      >
        <Unlink class="size-4" aria-hidden="true" />
        <span class="hidden sm:inline">Unpair</span>
      </button>
    {:else}
      <button type="button" class={btn.secondary} onclick={() => onPair(device)}>
        <Link2 class="size-4" aria-hidden="true" />
        Pair
      </button>
    {/if}
  </div>
</div>
