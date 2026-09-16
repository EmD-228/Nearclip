<script lang="ts">
  import { Link2, Send, Unlink } from "@lucide/svelte";
  import type { DeviceView } from "../types";
  import { btn, card } from "../ui";

  interface Props {
    device: DeviceView;
    onPair: (device: DeviceView) => void;
    onUnpair: (device: DeviceView) => void;
    onSend: (device: DeviceView) => void;
  }

  let { device, onPair, onUnpair, onSend }: Props = $props();

  let statusText = $derived(
    device.online ? (device.addr ?? "Online") : "Offline",
  );
</script>

<div class="{card} flex items-center gap-3 px-4 py-3">
  <span
    class="size-2.5 shrink-0 rounded-full {device.online
      ? 'bg-green-500'
      : 'bg-neutral-300 dark:bg-neutral-600'}"
    role="img"
    aria-label={device.online ? "Online" : "Offline"}
  ></span>

  <div class="min-w-0 flex-1">
    <div class="flex items-center gap-2">
      <span class="truncate text-sm font-medium">{device.name}</span>
      {#if device.paired}
        <span
          class="shrink-0 rounded-full bg-blue-50 px-2 py-px text-[11px] font-medium text-blue-700 dark:bg-blue-950/60 dark:text-blue-300"
        >
          Paired
        </span>
      {/if}
    </div>
    <p class="truncate text-xs text-neutral-500 dark:text-neutral-400" class:font-mono={device.online && device.addr}>
      {statusText}
    </p>
  </div>

  <div class="flex shrink-0 items-center gap-1 sm:gap-1.5">
    {#if device.paired}
      {#if device.online}
        <button type="button" class={btn.primary} onclick={() => onSend(device)}>
          <Send class="size-4" aria-hidden="true" />
          Send
        </button>
      {/if}
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
    {:else if device.online}
      <button type="button" class={btn.secondary} onclick={() => onPair(device)}>
        <Link2 class="size-4" aria-hidden="true" />
        Pair
      </button>
    {/if}
  </div>
</div>
