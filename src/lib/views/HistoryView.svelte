<script lang="ts">
  import { onMount } from "svelte";
  import {
    ArrowDownLeft,
    ArrowUpRight,
    Copy,
    File as FileIcon,
    FolderOpen,
    History,
    Share2,
    Trash2,
  } from "@lucide/svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api } from "../api";
  import { formatBytes, fullTime, relativeTime } from "../format";
  import { shareReceivedFile } from "../share";
  import { confirm } from "../stores/confirm.svelte";
  import { history } from "../stores/history.svelte";
  import { settings } from "../stores/settings.svelte";
  import { errorMessage, toasts } from "../stores/toasts.svelte";
  import type { HistoryItem } from "../types";
  import { btn, card, focusRing, muted, page, pageSubtitle, pageTitle } from "../ui";

  // Ticks so relative times ("3m ago") stay fresh while the view is open.
  let now = $state(Date.now());
  onMount(() => {
    const id = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(id);
  });

  const rowClass = "flex min-h-11 min-w-0 flex-1 items-start gap-3 px-4 py-3 text-left";

  interface ItemAction {
    label: string;
    icon: typeof Copy;
    run: () => Promise<void>;
  }

  /** What tapping an item does: copy text, or open a received file. Sent files have no action. */
  function actionFor(item: HistoryItem): ItemAction | null {
    const file = item.file;
    if (!file) {
      return { label: "Copy", icon: Copy, run: () => copy(item.text) };
    }
    const path = file.path;
    if (!path) return null;
    return settings.isDesktop
      ? { label: "Show in folder", icon: FolderOpen, run: () => revealItemInDir(path) }
      : { label: "Share", icon: Share2, run: () => shareReceivedFile(path, file.name, file.mime) };
  }

  async function run(action: ItemAction) {
    try {
      await action.run();
    } catch (err) {
      toasts.error(`${action.label} failed: ${errorMessage(err)}`);
    }
  }

  async function copy(text: string) {
    await api.copyToClipboard(text);
    toasts.success("Copied");
  }

  async function clearAll() {
    const ok = await confirm.ask({
      title: "Clear history?",
      message: "All sent and received items will be removed from this device. Received files stay where they were saved.",
      confirmLabel: "Clear",
      danger: true,
    });
    if (!ok) return;
    try {
      await history.clear();
      toasts.info("History cleared");
    } catch (err) {
      toasts.error(`Could not clear history: ${errorMessage(err)}`);
    }
  }
</script>

<div class={page}>
  <header class="mb-5 flex items-start justify-between gap-4 sm:mb-6">
    <div>
      <h1 class={pageTitle}>History</h1>
      <p class={pageSubtitle}>
        Everything sent and received on this device. Tap text to copy it, or a received file to
        {settings.isDesktop ? "show it in its folder" : "share it"}.
      </p>
    </div>
    {#if history.items.length > 0}
      <button type="button" class={btn.danger} onclick={clearAll} aria-label="Clear history">
        <Trash2 class="size-4" aria-hidden="true" />
        <span class="hidden sm:inline">Clear history</span>
      </button>
    {/if}
  </header>

  {#if history.items.length === 0}
    <div
      class="flex flex-col items-center rounded-lg border border-dashed border-neutral-300 px-5 py-10 text-center sm:px-6 sm:py-14 dark:border-neutral-700"
    >
      <History class="size-8 text-neutral-400" aria-hidden="true" />
      <h2 class="mt-4 text-sm font-medium">Nothing here yet</h2>
      <p class="mt-1 max-w-xs {muted}">
        Text and files you send or receive will show up here so you can find them again later.
      </p>
    </div>
  {:else}
    <ul class="flex flex-col gap-2">
      {#each history.items as item (item.id)}
        {@const action = actionFor(item)}
        <li class="{card} flex items-stretch overflow-hidden {item.ok ? '' : 'border-red-200 dark:border-red-900/60'}">
          {#if action}
            {@const ActionIcon = action.icon}
            <button
              type="button"
              class="{rowClass} transition-colors hover:bg-neutral-50 dark:hover:bg-neutral-800/60 {focusRing} focus-visible:ring-inset"
              onclick={() => run(action)}
              title={action.label}
            >
              {@render body(item)}
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
            <div class={rowClass}>{@render body(item)}</div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

{#snippet body(item: HistoryItem)}
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
        {sent ? "To" : "From"} {item.peerName}{item.ok ? "" : " · failed"}
      </span>
      <span class="shrink-0 text-neutral-400" title={fullTime(item.tsMs)}>
        {relativeTime(item.tsMs, now)}
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
