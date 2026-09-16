<script lang="ts">
  import { confirm } from "../stores/confirm.svelte";
  import { btn, modalBackdrop, modalPanel } from "../ui";

  let pending = $derived(confirm.pending);
  const titleId = $props.id();
  let confirmButton = $state<HTMLButtonElement | null>(null);

  $effect(() => {
    if (pending) confirmButton?.focus();
  });

  function onKeydown(e: KeyboardEvent) {
    if (!pending || e.key !== "Escape") return;
    e.preventDefault();
    confirm.answer(false);
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if pending}
  <div class={modalBackdrop} role="presentation">
    <div role="alertdialog" aria-modal="true" aria-labelledby={titleId} class={modalPanel}>
      <h2 id={titleId} class="text-base font-semibold">{pending.title}</h2>
      <p class="mt-1.5 text-sm text-neutral-500 dark:text-neutral-400">{pending.message}</p>
      <div class="mt-5 flex justify-end gap-2">
        <button
          type="button"
          class="{btn.secondary} flex-1 sm:flex-none"
          onclick={() => confirm.answer(false)}
        >
          {pending.cancelLabel ?? "Cancel"}
        </button>
        <button
          type="button"
          bind:this={confirmButton}
          class="{pending.danger ? btn.dangerSolid : btn.primary} flex-1 sm:flex-none"
          onclick={() => confirm.answer(true)}
        >
          {pending.confirmLabel ?? "Confirm"}
        </button>
      </div>
    </div>
  </div>
{/if}
