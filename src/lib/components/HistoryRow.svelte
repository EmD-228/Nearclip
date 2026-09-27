<script lang="ts">
  // One sent or received item, with the action it offers: copy the text, or
  // open a received file where it was saved. Used by the History view and,
  // under each paired device, by the Devices view.
  import {
    ArrowDownLeft,
    ArrowUpRight,
    Copy,
    File as FileIcon,
    FolderOpen,
    Share2,
  } from "@lucide/svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api } from "../api";
  import { formatBytes, fullTime, relativeTime } from "../format";
  import { shareReceivedFile } from "../share";
  import { clock } from "../stores/clock.svelte";
  import { settings } from "../stores/settings.svelte";
  import { errorMessage, toasts } from "../stores/toasts.svelte";
  import type { HistoryItem } from "../types";
  import { btn, card, focusRing } from "../ui";

  interface Props {
    item: HistoryItem;
    /** Off where the peer is already obvious, such as under its own card. */
    showPeer?: boolean;
  }

  let { item, showPeer = true }: Props = $props();

  interface ItemAction {
    label: string;
    icon: typeof Copy;
    run: () => Promise<void>;
  }

  const rowClass = "flex min-h-11 min-w-0 flex-1 items-start gap-3 px-4 py-3 text-left";

  /** What tapping the item does: copy text, or open a received file. Sent files have no action. */
  let action = $derived.by<ItemAction | null>(() => {
    const file = item.file;
    if (!file) {
      return { label: "Copy", icon: Copy, run: () => copy(item.text) };
    }
    const path = file.path;
    if (!path) return null;
    return settings.isDesktop
      ? { label: "Show in folder", icon: FolderOpen, run: () => revealItemInDir(path) }
      : { label: "Share", icon: Share2, run: () => shareReceivedFile(path, file.name, file.mime) };
  });

  async function run(a: ItemAction) {
    try {
      await a.run();
    } catch (err) {
      toasts.error(`${a.label} failed: ${errorMessage(err)}`);
    }
  }

  async function copy(text: string) {
    await api.copyToClipboard(text);
    toasts.success("Copied");
  }
</script>

<div
  class="{card} flex items-stretch overflow-hidden {item.ok
    ? ''
    : 'border-red-200 dark:border-red-900/60'}"
>
  {#if action}
    {@const ActionIcon = action.icon}
    <button
      type="button"
      class="{rowClass} transition-colors hover:bg-neutral-50 dark:hover:bg-neutral-800/60 {focusRing} focus-visible:ring-inset"
      onclick={() => run(action)}
      title={action.label}
    >
      {@render body()}
    </button>
    <button
      type="button"
      class="{btn.icon} m-2 self-center"
      aria-label={action.label}
      title={action.label}
      onclick={() => run(action)}
    >
      <ActionIcon class="size-4" aria-hidden="true" />
    </button>
  {:else}
    <div class={rowClass}>{@render body()}</div>
  {/if}
</div>

{#snippet body()}
  {@const sent = item.direction === "sent"}
  {@const Direction = sent ? ArrowUpRight : ArrowDownLeft}
  <span
    class="mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-full {item.ok
      ? sent
        ? 'bg-blue-50 text-blue-600 dark:bg-blue-950/60 dark:text-blue-300'
        : 'bg-green-50 text-green-600 dark:bg-green-950/60 dark:text-green-300'
      : 'bg-red-50 text-red-600 dark:bg-red-950/60 dark:text-red-400'}"
    aria-hidden="true"
  >
    <Direction class="size-3.5" />
  </span>
  <span class="min-w-0 flex-1">
    <span class="flex items-baseline gap-2 text-xs">
      <span
        class="truncate font-medium {item.ok
          ? 'text-neutral-700 dark:text-neutral-200'
          : 'text-red-600 dark:text-red-400'}"
      >
        {#if showPeer}
          {sent ? "To" : "From"} {item.peerName}{item.ok ? "" : " · failed"}
        {:else}
          {sent ? "Sent" : "Received"}{item.ok ? "" : " · failed"}
        {/if}
      </span>
      <span class="shrink-0 text-neutral-400" title={fullTime(item.tsMs)}>
        {relativeTime(item.tsMs, clock.now)}
      </span>
    </span>
    {#if item.file}
      <span
        class="mt-1 flex min-w-0 items-center gap-1.5 text-sm {item.ok
          ? 'text-neutral-800 dark:text-neutral-100'
          : 'text-red-600 dark:text-red-400'}"
      >
        <FileIcon class="size-4 shrink-0 text-neutral-400" aria-hidden="true" />
        <span class="truncate">{item.file.name}</span>
        <span class="shrink-0 text-xs text-neutral-400">{formatBytes(item.file.size)}</span>
      </span>
    {:else}
      <span
        class="mt-1 line-clamp-2 block text-sm wrap-break-word whitespace-pre-wrap {item.ok
          ? 'text-neutral-800 dark:text-neutral-100'
          : 'text-red-600 dark:text-red-400'}"
      >
        {item.text}
      </span>
    {/if}
  </span>
{/snippet}
