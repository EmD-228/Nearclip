<script lang="ts">
  import { Link2, Send, Unlink } from "@lucide/svelte";
  import { relativeTime } from "../format";
  import { clock } from "../stores/clock.svelte";
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

  /**
   * The address is a poor status line: it stays put while the device leaves the
   * network. Say whether the device answers instead, and keep the address in the
   * tooltip for the rare case someone needs it. Devices in the Available list
   * are on this network by definition, since discovery just saw them.
   */
  const ONLINE = "bg-green-500";

  let status = $derived.by(() => {
    if (!device.paired) return { label: "Same network", detail: null, dot: ONLINE };
    if (device.online === null) {
      return {
        label: "Checking…",
        detail: null,
        dot: "bg-neutral-300 animate-pulse dark:bg-neutral-600",
      };
    }
    if (device.online) return { label: "Connected", detail: "Same network", dot: ONLINE };
    return {
      label: "Disconnected",
      detail: device.lastSeenMs
        ? `last seen ${relativeTime(device.lastSeenMs, clock.now)}`
        : "not on this network",
      dot: "bg-neutral-400 dark:bg-neutral-500",
    };
  });
</script>

<div class="{card} flex items-center gap-3 px-4 py-3">
  <div class="min-w-0 flex-1">
    <p class="truncate text-sm font-medium">{device.name}</p>
    <div
      class="mt-1 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-xs text-neutral-500 dark:text-neutral-400"
    >
      <span class="flex shrink-0 items-center gap-1.5" title={device.addr ?? undefined}>
        <span class="size-1.5 rounded-full {status.dot}" aria-hidden="true"></span>
        {status.label}
      </span>
      {#if status.detail}
        <span class="min-w-0 truncate">{status.detail}</span>
      {/if}
      {#if pairedLabel}
        <span
          class="shrink-0 rounded-full bg-neutral-100 px-2 py-px text-[11px] font-medium text-neutral-600 dark:bg-neutral-800 dark:text-neutral-300"
        >
          {pairedLabel}
        </span>
      {/if}
    </div>
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
