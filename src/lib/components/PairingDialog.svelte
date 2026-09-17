<script lang="ts">
  import { CircleAlert, CircleCheck, LoaderCircle, Link2 } from "@lucide/svelte";
  import { formatPairingCode } from "../format";
  import { pairing } from "../stores/pairing.svelte";
  import { btn, modalTitle, muted } from "../ui";
  import Modal from "./Modal.svelte";

  let current = $derived(pairing.current);
  let busy = $state(false);

  async function run(fn: () => Promise<void>) {
    if (busy) return;
    busy = true;
    try {
      await fn();
    } finally {
      busy = false;
    }
  }

  // One dismiss action per step, shared by the button and the Escape key:
  // none while the other device is confirming or a request is in flight.
  let dismiss = $derived.by(() => {
    if (!current || busy || current.status === "confirming") return undefined;
    if (current.status === "success" || current.status === "error") return () => pairing.close();
    return () => run(() => pairing.cancel());
  });
</script>

{#if current}
  {@const c = current}
  <Modal onClose={dismiss}>
    {#snippet children({ titleId })}
      {#if c.status === "waiting" || c.status === "request" || c.status === "confirming"}
        <div class="flex flex-col items-center gap-4 text-center">
          <LoaderCircle class="size-8 animate-spin text-blue-600" aria-hidden="true" />
          <div>
            <h2 id={titleId} class={modalTitle}>
              {#if c.status === "request"}
                {c.name} wants to pair
              {:else if c.status === "confirming"}
                Waiting for {c.name}…
              {:else}
                Connecting to {c.name}…
              {/if}
            </h2>
            <p class="mt-1 {muted}">
              {#if c.status === "request"}
                Waiting for the pairing code. A 6-digit code will appear on both screens.
              {:else if c.status === "confirming"}
                The other device still has to confirm that the codes match.
              {:else if c.viaQr}
                Keep NearClip open on both devices. This only takes a few seconds.
              {:else}
                A 6-digit code will appear on both screens once connected.
              {/if}
            </p>
          </div>
          {#if c.status !== "confirming"}
            <button type="button" class={btn.secondary} disabled={!dismiss} onclick={dismiss}>
              Cancel
            </button>
          {/if}
        </div>
      {:else if c.status === "code"}
        <div class="flex flex-col items-center gap-5 text-center">
          <div
            class="flex size-10 items-center justify-center rounded-full bg-blue-50 dark:bg-blue-950/60"
          >
            <Link2 class="size-5 text-blue-600" aria-hidden="true" />
          </div>
          <div>
            <h2 id={titleId} class={modalTitle}>Compare the code</h2>
            <p class="mt-1 {muted}">Make sure the same code is shown on {c.name}.</p>
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
          <h2 id={titleId} class={modalTitle}>Paired with {c.name}</h2>
          <p class={muted}>You can now send text to this device.</p>
        </div>
      {:else}
        <div class="flex flex-col items-center gap-4 text-center">
          <CircleAlert class="size-10 text-red-600" aria-hidden="true" />
          <div>
            <h2 id={titleId} class={modalTitle}>Pairing failed</h2>
            <p class="mt-1 wrap-break-word {muted}">
              {c.error ?? "Something went wrong while pairing with " + c.name + "."}
            </p>
          </div>
          <button type="button" class={btn.secondary} onclick={() => pairing.close()}>
            Close
          </button>
        </div>
      {/if}
    {/snippet}
  </Modal>
{/if}
