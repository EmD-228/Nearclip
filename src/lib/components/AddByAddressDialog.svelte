<script lang="ts">
  import { addByAddress } from "../stores/dialogs.svelte";
  import { pairing } from "../stores/pairing.svelte";
  import { btn, btnFill, input, modalTitle, muted } from "../ui";
  import Modal from "./Modal.svelte";

  let address = $state("");
  const fieldId = $props.id();

  // The backend validates the address; a bad one comes back as a pairing error
  // shown by PairingDialog, so there is no second parser to keep in sync here.
  function submit(e: SubmitEvent) {
    e.preventDefault();
    const addr = address.trim();
    if (!addr) return;
    addByAddress.close();
    void pairing.startByAddress(addr);
  }
</script>

{#if addByAddress.open}
  <Modal onClose={() => addByAddress.close()} described>
    {#snippet children({ titleId, descId })}
      <form onsubmit={submit} novalidate>
        <h2 id={titleId} class={modalTitle}>Add a device by address</h2>
        <p id={descId} class="mt-1.5 {muted}">
          Use this when the other device is on the same network but does not show up (some Wi-Fi
          networks block discovery). Find the address in that device's Settings under This device.
        </p>

        <label for={fieldId} class="sr-only">Device address</label>
        <!-- No inputmode: numeric keyboards on phones have no ":" for the port. -->
        <input
          id={fieldId}
          bind:value={address}
          data-autofocus
          type="text"
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false"
          placeholder="192.168.1.20:47821"
          class="{input} mt-4 font-mono"
        />

        <div class="mt-5 flex justify-end gap-2">
          <button
            type="button"
            class="{btn.secondary} {btnFill}"
            onclick={() => addByAddress.close()}
          >
            Cancel
          </button>
          <button
            type="submit"
            class="{btn.primary} {btnFill}"
            disabled={address.trim().length === 0}
          >
            Pair
          </button>
        </div>
      </form>
    {/snippet}
  </Modal>
{/if}
