<script lang="ts">
  import { confirm } from "../stores/confirm.svelte";
  import { btn, btnFill, modalTitle, muted } from "../ui";
  import Modal from "./Modal.svelte";

  let pending = $derived(confirm.pending);
</script>

{#if pending}
  <Modal onClose={() => confirm.answer(false)} role="alertdialog">
    {#snippet children({ titleId })}
      <h2 id={titleId} class={modalTitle}>{pending.title}</h2>
      <p class="mt-1.5 {muted}">{pending.message}</p>
      <div class="mt-5 flex justify-end gap-2">
        <button type="button" class="{btn.secondary} {btnFill}" onclick={() => confirm.answer(false)}>
          {pending.cancelLabel ?? "Cancel"}
        </button>
        <button
          type="button"
          data-autofocus
          class="{pending.danger ? btn.dangerSolid : btn.primary} {btnFill}"
          onclick={() => confirm.answer(true)}
        >
          {pending.confirmLabel ?? "Confirm"}
        </button>
      </div>
    {/snippet}
  </Modal>
{/if}
