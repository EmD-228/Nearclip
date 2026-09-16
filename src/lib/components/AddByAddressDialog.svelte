<script lang="ts">
  import { pairing } from "../stores/pairing.svelte";
  import { btn, input, modalBackdrop, modalPanel } from "../ui";

  interface Props {
    open: boolean;
  }

  let { open = $bindable(false) }: Props = $props();

  let address = $state("");
  let field = $state<HTMLInputElement | null>(null);

  const uid = $props.id();
  const titleId = `${uid}-title`;
  const helpId = `${uid}-help`;
  const fieldId = `${uid}-address`;

  $effect(() => {
    if (open) {
      address = "";
      field?.focus();
    }
  });

  function close() {
    open = false;
  }

  // The backend validates the address; a bad one comes back as a pairing error
  // shown by PairingDialog, so there is no second parser to keep in sync here.
  function submit(e: SubmitEvent) {
    e.preventDefault();
    const addr = address.trim();
    if (!addr) return;
    close();
    void pairing.startByAddress(addr);
  }

  function onKeydown(e: KeyboardEvent) {
    if (!open || e.key !== "Escape") return;
    e.preventDefault();
    close();
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <div class={modalBackdrop} role="presentation">
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      aria-describedby={helpId}
      class={modalPanel}
    >
      <form onsubmit={submit} novalidate>
        <h2 id={titleId} class="text-base font-semibold">Add a device by address</h2>
        <p id={helpId} class="mt-1.5 text-sm text-neutral-500 dark:text-neutral-400">
          Use this when the other device is on the same network but does not show up (some Wi-Fi
          networks block discovery). Find the address in that device's Settings under This device.
        </p>

        <label for={fieldId} class="sr-only">Device address</label>
        <!-- No inputmode: numeric keyboards on phones have no ":" for the port. -->
        <input
          id={fieldId}
          bind:this={field}
          bind:value={address}
          type="text"
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false"
          placeholder="192.168.1.20:47821"
          class="{input} mt-4 font-mono"
        />

        <div class="mt-5 flex justify-end gap-2">
          <button type="button" class="{btn.secondary} flex-1 sm:flex-none" onclick={close}>
            Cancel
          </button>
          <button
            type="submit"
            class="{btn.primary} flex-1 sm:flex-none"
            disabled={address.trim().length === 0}
          >
            Pair
          </button>
        </div>
      </form>
    </div>
  </div>
{/if}
