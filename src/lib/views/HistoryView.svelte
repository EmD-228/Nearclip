<script lang="ts">
  import { onMount } from "svelte";
  import { ArrowDownLeft, ArrowUpRight, Copy, History, Trash2 } from "@lucide/svelte";
  import { api } from "../api";
  import { fullTime, relativeTime } from "../format";
  import { confirm } from "../stores/confirm.svelte";
  import { history } from "../stores/history.svelte";
  import { errorMessage, toasts } from "../stores/toasts.svelte";
  import type { HistoryItem } from "../types";
  import { btn, card, focusRing, page, pageSubtitle, pageTitle } from "../ui";

  // Ticks so relative times ("3m ago") stay fresh while the view is open.
  let now = $state(Date.now());
  onMount(() => {
    const id = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(id);
  });

  async function copy(item: HistoryItem) {
    try {
      await api.copyToClipboard(item.text);
      toasts.success("Copied");
    } catch (err) {
      toasts.error(`Could not copy: ${errorMessage(err)}`);
    }
  }

  async function clearAll() {
    const ok = await confirm.ask({
      title: "Clear history?",
      message: "All sent and received items will be removed from this device.",
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
      <p class={pageSubtitle}>Everything sent and received on this device. Tap an item to copy it.</p>
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
      <p class="mt-1 max-w-xs text-sm text-neutral-500 dark:text-neutral-400">
        Text you send or receive will show up here so you can copy it again later.
      </p>
    </div>
  {:else}
    <ul class="flex flex-col gap-2">
      {#each history.items as item (item.id)}
        {@const sent = item.direction === "sent"}
        {@const Icon = sent ? ArrowUpRight : ArrowDownLeft}
        <li class="{card} flex items-stretch overflow-hidden {item.ok ? '' : 'border-red-200 dark:border-red-900/60'}">
          <button
            type="button"
            class="flex min-h-11 min-w-0 flex-1 items-start gap-3 px-4 py-3 text-left transition-colors hover:bg-neutral-50 dark:hover:bg-neutral-800/60 {focusRing} focus-visible:ring-inset"
            onclick={() => copy(item)}
            title="Copy to clipboard"
          >
            <span
              class="mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-full {item.ok
                ? sent
                  ? 'bg-blue-50 text-blue-600 dark:bg-blue-950/60 dark:text-blue-300'
                  : 'bg-green-50 text-green-600 dark:bg-green-950/60 dark:text-green-300'
                : 'bg-red-50 text-red-600 dark:bg-red-950/60 dark:text-red-400'}"
              aria-hidden="true"
            >
              <Icon class="size-3.5" />
            </span>
            <span class="min-w-0 flex-1">
              <span class="flex items-baseline gap-2 text-xs">
                <span class="truncate font-medium {item.ok ? 'text-neutral-700 dark:text-neutral-200' : 'text-red-600 dark:text-red-400'}">
                  {sent ? "To" : "From"} {item.peerName}{item.ok ? "" : " · failed"}
                </span>
                <span class="shrink-0 text-neutral-400" title={fullTime(item.tsMs)}>
                  {relativeTime(item.tsMs, now)}
                </span>
              </span>
              <span
                class="mt-1 line-clamp-2 block text-sm break-words whitespace-pre-wrap {item.ok
                  ? 'text-neutral-800 dark:text-neutral-100'
                  : 'text-red-600 dark:text-red-400'}"
              >
                {item.text}
              </span>
            </span>
          </button>
          <button
            type="button"
            class="{btn.icon} m-2 self-center"
            aria-label="Copy"
            title="Copy"
            onclick={() => copy(item)}
          >
            <Copy class="size-4" aria-hidden="true" />
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
