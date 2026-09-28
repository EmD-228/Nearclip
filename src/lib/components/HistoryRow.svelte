<script lang="ts">
  // One sent or received item, with the action it offers: copy the text, or
  // open a received file where it was saved. Used by the History view and,
  // under each paired device, by the Devices view.
  import {
    ArrowDownLeft,
    ArrowUpRight,
    Copy,
    ExternalLink,
    File as FileIcon,
    FolderOpen,
    Share2,
  } from "@lucide/svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api } from "../api";
  import { formatBytes, fullTime, relativeTime } from "../format";
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

  /**
   * What the item offers, the first being what tapping the row does: copy text,
   * open a received file, or hand it to another app. A sent file offers nothing,
   * since its copy lives on the other device.
   */
  let actions = $derived.by<ItemAction[]>(() => {
    const file = item.file;
    if (!file) {
      return [{ label: "Copy", icon: Copy, run: () => copy(item.text) }];
    }
    if (settings.isDesktop) {
      const path = file.path;
      return path
        ? [{ label: "Show in folder", icon: FolderOpen, run: () => revealItemInDir(path) }]
        : [];
    }
    // Android hands a saved file back as a URI; a path alone cannot open it.
    const uri = file.uri;
    if (!uri) return [];
    return [
      { label: "Open", icon: ExternalLink, run: () => api.openReceivedFile(uri, file.mime) },
      { label: "Share", icon: Share2, run: () => api.shareReceivedFile(uri, file.mime) },
    ];
  });

  let primary = $derived(actions[0]);

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
  {#if primary}
    <button
      type="button"
      class="{rowClass} transition-colors hover:bg-neutral-50 dark:hover:bg-neutral-800/60 {focusRing} focus-visible:ring-inset"
      onclick={() => run(primary)}
      title={primary.label}
    >
      {@render body()}
    </button>
    <div class="flex shrink-0 items-center gap-1 self-center px-2">
      {#each actions as action}
        {@const ActionIcon = action.icon}
        <button
          type="button"
          class={btn.icon}
          aria-label={action.label}
          title={action.label}
          onclick={() => run(action)}
        >
          <ActionIcon class="size-4" aria-hidden="true" />
        </button>
      {/each}
    </div>
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
      <!-- Phones hide the folder a file went to, so the row says where it is. -->
      {#if !settings.isDesktop && item.file.path}
        <span class="mt-0.5 block truncate text-xs text-neutral-500 dark:text-neutral-400">
          Saved to {item.file.path}
        </span>
      {/if}
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
