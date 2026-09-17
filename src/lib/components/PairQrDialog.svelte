<script lang="ts">
  import { untrack } from "svelte";
  import { LoaderCircle, RefreshCw } from "@lucide/svelte";
  import { api } from "../api";
  import { qr } from "../stores/dialogs.svelte";
  import { errorMessage, toasts } from "../stores/toasts.svelte";
  import type { PairQr } from "../types";
  import { btn, modalTitle, muted } from "../ui";
  import Modal from "./Modal.svelte";

  let code = $state<PairQr | null>(null);
  let loading = $state(false);
  let expired = $state(false);
  let expiryTimer: ReturnType<typeof setTimeout> | null = null;
  let ttlMinutes = $derived(code ? Math.max(1, Math.round(code.ttlMs / 60_000)) : 5);

  function clearExpiry() {
    if (expiryTimer) {
      clearTimeout(expiryTimer);
      expiryTimer = null;
    }
  }

  async function load() {
    clearExpiry();
    loading = true;
    expired = false;
    code = null;
    try {
      const next = await api.createPairQr();
      if (!qr.open) return;
      code = next;
      expiryTimer = setTimeout(() => (expired = true), next.ttlMs);
    } catch (err) {
      toasts.error(`Could not create the QR code: ${errorMessage(err)}`);
      qr.close();
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (!qr.open) return;
    untrack(() => void load());
    return () => {
      clearExpiry();
      code = null;
    };
  });
</script>

{#if qr.open}
  <Modal onClose={() => qr.close()} described>
    {#snippet children({ titleId, descId })}
      <div class="flex flex-col items-center text-center">
        <h2 id={titleId} class={modalTitle}>Pair with your phone</h2>

        <!-- Always white, so the code stays scannable in dark mode. -->
        <div
          class="relative mt-4 flex size-60 items-center justify-center overflow-hidden rounded-xl bg-white p-3 ring-1 ring-neutral-200 dark:ring-0 [&_svg]:size-full"
        >
          {#if code}
            <div class="size-full {expired ? 'opacity-10 blur-[2px]' : ''}" aria-hidden={expired}>
              {@html code.svg}
            </div>
          {/if}
          {#if loading}
            <LoaderCircle
              class="absolute size-8 animate-spin text-blue-600"
              aria-label="Creating code"
            />
          {:else if expired}
            <div class="absolute flex flex-col items-center gap-3 px-4">
              <p class="text-sm font-medium text-neutral-900">This code has expired</p>
              <button type="button" class={btn.primary} onclick={load}>
                <RefreshCw class="size-4" aria-hidden="true" />
                New code
              </button>
            </div>
          {/if}
        </div>

        <p id={descId} class="mt-4 {muted}">
          On your phone, open NearClip, tap Scan QR code and point it at this code. It expires
          after {ttlMinutes} minutes.
        </p>

        <button
          type="button"
          class="{btn.secondary} mt-5 w-full sm:w-auto"
          onclick={() => qr.close()}
        >
          Close
        </button>
      </div>
    {/snippet}
  </Modal>
{/if}
