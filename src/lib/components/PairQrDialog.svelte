<script lang="ts">
  import { untrack } from "svelte";
  import { LoaderCircle, RefreshCw } from "@lucide/svelte";
  import { api } from "../api";
  import { qr } from "../stores/qr.svelte";
  import { errorMessage, toasts } from "../stores/toasts.svelte";
  import type { PairQr } from "../types";
  import { btn, modalBackdrop, modalPanel } from "../ui";

  let code = $state<PairQr | null>(null);
  let loading = $state(false);
  let expired = $state(false);
  let expiryTimer: ReturnType<typeof setTimeout> | null = null;
  let ttlMinutes = $derived(code ? Math.max(1, Math.round(code.ttlMs / 60_000)) : 5);

  const uid = $props.id();
  const titleId = `${uid}-title`;
  const helpId = `${uid}-help`;

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

  function onKeydown(e: KeyboardEvent) {
    if (!qr.open || e.key !== "Escape") return;
    e.preventDefault();
    qr.close();
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if qr.open}
  <div class={modalBackdrop} role="presentation">
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      aria-describedby={helpId}
      class="{modalPanel} flex flex-col items-center text-center"
    >
      <h2 id={titleId} class="text-base font-semibold">Pair with your phone</h2>

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
          <LoaderCircle class="absolute size-8 animate-spin text-blue-600" aria-label="Creating code" />
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

      <p id={helpId} class="mt-4 text-sm text-neutral-500 dark:text-neutral-400">
        On your phone, open NearClip, tap Scan QR code and point it at this code. It expires after
        {ttlMinutes} minutes.
      </p>

      <button type="button" class="{btn.secondary} mt-5 w-full sm:w-auto" onclick={() => qr.close()}>
        Close
      </button>
    </div>
  </div>
{/if}
