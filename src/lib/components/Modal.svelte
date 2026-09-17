<script lang="ts">
  // A div rather than <dialog>: showModal() would put the panel in the top
  // layer above the toasts App shows while a dialog is still open, and its
  // Escape handling cannot express a step that must not be interrupted.
  //
  // Mount it only while the dialog is open ({#if ...}<Modal>). Focus moves to
  // the element marked data-autofocus (or the panel), the page behind turns
  // inert, and focus returns to the opener on close.
  import type { Snippet } from "svelte";
  import { modals } from "../stores/dialogs.svelte";

  interface Props {
    /** Runs on Escape. Leave out while a step must not be interrupted. */
    onClose?: () => void;
    role?: "dialog" | "alertdialog";
    /** Set when the content has a paragraph with id `descId`. */
    described?: boolean;
    children: Snippet<[{ titleId: string; descId: string }]>;
  }

  let { onClose, role = "dialog", described = false, children }: Props = $props();

  const uid = $props.id();
  const ids = { titleId: `${uid}-title`, descId: `${uid}-desc` };
  let panel = $state<HTMLDivElement | null>(null);

  $effect(() => {
    const opener = document.activeElement as HTMLElement | null;
    (panel?.querySelector<HTMLElement>("[data-autofocus]") ?? panel)?.focus();
    modals.opened();
    return () => {
      modals.closed();
      opener?.focus();
    };
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key !== "Escape" || !onClose) return;
    e.preventDefault();
    onClose();
  }
</script>

<div
  class="fixed inset-0 z-40 flex items-center justify-center bg-neutral-900/40 p-4 backdrop-blur-[2px] sm:p-6 dark:bg-black/60"
  role="presentation"
>
  <!-- Panel sized to fit a 360px wide screen. -->
  <div
    bind:this={panel}
    {role}
    aria-modal="true"
    aria-labelledby={ids.titleId}
    aria-describedby={described ? ids.descId : undefined}
    tabindex="-1"
    class="w-full max-w-sm rounded-xl border border-neutral-200 bg-white p-5 shadow-2xl outline-none sm:p-6 dark:border-neutral-800 dark:bg-neutral-900"
    onkeydown={onKeydown}
  >
    {@render children(ids)}
  </div>
</div>
