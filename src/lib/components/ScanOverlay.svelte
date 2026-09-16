<script lang="ts">
  import { X } from "@lucide/svelte";
  import { cancelScan, scanner } from "../scan.svelte";
  import { focusRing } from "../ui";
</script>

<!-- Drawn over the live camera, which shows through the transparent webview. -->
{#if scanner.active}
  <div
    class="fixed inset-0 z-50 flex flex-col items-center justify-between px-6 pt-[calc(4rem+env(safe-area-inset-top))] pb-[calc(2.5rem+env(safe-area-inset-bottom))]"
    role="dialog"
    aria-modal="true"
    aria-label="Scan QR code"
  >
    <p class="rounded-full bg-black/60 px-4 py-2 text-center text-sm font-medium text-white">
      Point the camera at the QR code on your computer
    </p>

    <div class="aspect-square w-full max-w-64 rounded-2xl border-2 border-white/90 shadow-[0_0_0_100vmax_rgb(0_0_0/0.35)]" aria-hidden="true"></div>

    <button
      type="button"
      onclick={cancelScan}
      class="inline-flex h-12 items-center justify-center gap-2 rounded-full bg-white px-6 text-sm font-medium text-neutral-900 shadow-lg active:bg-neutral-200 {focusRing}"
    >
      <X class="size-4" aria-hidden="true" />
      Cancel
    </button>
  </div>
{/if}
