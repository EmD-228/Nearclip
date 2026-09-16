<script lang="ts">
  import { CircleAlert, CircleCheck, Info, X } from "@lucide/svelte";
  import { toasts, type ToastKind } from "../stores/toasts.svelte";
  import { btn } from "../ui";

  const icons: Record<ToastKind, typeof Info> = {
    info: Info,
    success: CircleCheck,
    error: CircleAlert,
  };

  const iconColor: Record<ToastKind, string> = {
    info: "text-blue-600",
    success: "text-green-600",
    error: "text-red-600",
  };
</script>

<div
  class="pointer-events-none fixed right-4 bottom-4 z-50 flex w-80 max-w-[calc(100vw-2rem)] flex-col gap-2"
  role="status"
  aria-live="polite"
>
  {#each toasts.items as toast (toast.id)}
    {@const Icon = icons[toast.kind]}
    <div
      class="pointer-events-auto flex items-start gap-2.5 rounded-lg border border-neutral-200 bg-white p-3 shadow-lg dark:border-neutral-700 dark:bg-neutral-800"
    >
      <Icon class="mt-px size-4 shrink-0 {iconColor[toast.kind]}" aria-hidden="true" />
      <p class="min-w-0 flex-1 text-sm break-words">{toast.text}</p>
      <button
        type="button"
        class="{btn.icon} -m-1.5 size-7"
        aria-label="Dismiss"
        onclick={() => toasts.dismiss(toast.id)}
      >
        <X class="size-3.5" aria-hidden="true" />
      </button>
    </div>
  {/each}
</div>
