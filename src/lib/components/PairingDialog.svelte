<script lang="ts">
  import { CircleAlert, CircleCheck, LoaderCircle, Link2 } from "@lucide/svelte";
  import { formatPairingCode } from "../format";
  import { pairing } from "../stores/pairing.svelte";
  import { btn, modalBackdrop, modalPanel } from "../ui";

  let current = $derived(pairing.current);
  let busy = $state(false);

  const titleId = $props.id();

  async function run(fn: () => Promise<void>) {
    if (busy) return;
    busy = true;
    try {
      await fn();
    } finally {
      busy = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (!current || e.key !== "Escape") return;
    e.preventDefault();
    if (current.status === "success" || current.status === "error") pairing.close();
    else void run(() => pairing.cancel());
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if current}
  {@const c = current}
  <div class={modalBackdrop} role="presentation">
    <div role="dialog" aria-modal="true" aria-labelledby={titleId} class={modalPanel}>
      {#if c.status === "waiting" || c.status === "request" || c.status === "confirming"}
        <div class="flex flex-col items-center gap-4 text-center">
          <LoaderCircle class="size-8 animate-spin text-blue-600" aria-hidden="true" />
          <div>
            <h2 id={titleId} class="text-base font-semibold">
              {#if c.status === "request"}
                {c.name} wants to pair
              {:else if c.status === "confirming"}
                Waiting for {c.name}…
              {:else}
                Connecting to {c.name}…
              {/if}
            </h2>
            <p class="mt-1 text-sm text-neutral-500 dark:text-neutral-400">
              {#if c.status === "request"}
                Waiting for the pairing code. A 6-digit code will appear on both screens.
              {:else if c.status === "confirming"}
                The other device still has to confirm that the codes match.
              {:else if c.viaQr}
                Keep nearclip open on both devices. This only takes a few seconds.
              {:else}
                A 6-digit code will appear on both screens once connected.
              {/if}
            </p>
          </div>
          {#if c.status !== "confirming"}
            <button
              type="button"
              class={btn.secondary}
              disabled={busy}
              onclick={() => run(() => pairing.cancel())}
            >
              Cancel
            </button>
          {/if}
        </div>
      {:else if c.status === "code"}
        <div class="flex flex-col items-center gap-5 text-center">
          <div class="flex size-10 items-center justify-center rounded-full bg-blue-50 dark:bg-blue-950/60">
            <Link2 class="size-5 text-blue-600" aria-hidden="true" />
          </div>
          <div>
            <h2 id={titleId} class="text-base font-semibold">Compare the code</h2>
            <p class="mt-1 text-sm text-neutral-500 dark:text-neutral-400">
              Make sure the same code is shown on {c.name}.
            </p>
          </div>
          <p
            class="font-mono text-3xl font-semibold tracking-[0.15em] tabular-nums select-text sm:text-4xl sm:tracking-[0.2em]"
            aria-label="Pairing code {c.code}"
          >
            {formatPairingCode(c.code ?? "")}
          </p>
          <div class="flex w-full gap-2">
            <button
              type="button"
              class="{btn.secondary} flex-1"
              disabled={busy}
              onclick={() => run(() => pairing.confirm(false))}
            >
              Cancel
            </button>
            <button
              type="button"
              class="{btn.primary} flex-1"
              disabled={busy}
              onclick={() => run(() => pairing.confirm(true))}
            >
              Codes match
            </button>
          </div>
        </div>
      {:else if c.status === "success"}
        <div class="flex flex-col items-center gap-3 text-center">
          <CircleCheck class="size-10 text-green-600" aria-hidden="true" />
          <h2 id={titleId} class="text-base font-semibold">Paired with {c.name}</h2>
          <p class="text-sm text-neutral-500 dark:text-neutral-400">
            You can now send text to this device.
          </p>
        </div>
      {:else}
        <div class="flex flex-col items-center gap-4 text-center">
          <CircleAlert class="size-10 text-red-600" aria-hidden="true" />
          <div>
            <h2 id={titleId} class="text-base font-semibold">Pairing failed</h2>
            <p class="mt-1 text-sm wrap-break-word text-neutral-500 dark:text-neutral-400">
              {c.error ?? "Something went wrong while pairing with " + c.name + "."}
            </p>
          </div>
          <button type="button" class={btn.secondary} onclick={() => pairing.close()}>
            Close
          </button>
        </div>
      {/if}
    </div>
  </div>
{/if}
