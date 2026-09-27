<script lang="ts">
  import { History, Trash2 } from "@lucide/svelte";
  import HistoryRow from "../components/HistoryRow.svelte";
  import { confirm } from "../stores/confirm.svelte";
  import { history } from "../stores/history.svelte";
  import { settings } from "../stores/settings.svelte";
  import { errorMessage, toasts } from "../stores/toasts.svelte";
  import { btn, muted, page, pageSubtitle, pageTitle } from "../ui";

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
        <li><HistoryRow {item} /></li>
      {/each}
    </ul>
  {/if}
</div>
