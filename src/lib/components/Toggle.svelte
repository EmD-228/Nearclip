<script lang="ts">
  import { focusRing } from "../ui";

  interface Props {
    checked: boolean;
    label: string;
    description?: string;
    disabled?: boolean;
    onchange: (checked: boolean) => void;
  }

  let { checked, label, description, disabled = false, onchange }: Props = $props();

  const uid = $props.id();
  const labelId = `${uid}-label`;
  const descId = `${uid}-desc`;
</script>

<div class="flex items-start justify-between gap-6 py-3">
  <div class="min-w-0">
    <span id={labelId} class="text-sm font-medium">{label}</span>
    {#if description}
      <p id={descId} class="mt-0.5 text-xs text-neutral-500 dark:text-neutral-400">
        {description}
      </p>
    {/if}
  </div>
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    aria-labelledby={labelId}
    aria-describedby={description ? descId : undefined}
    {disabled}
    class="relative mt-0.5 inline-flex h-6 w-11 shrink-0 rounded-full transition-colors before:absolute before:-inset-x-2 before:-inset-y-2.5 before:content-[''] disabled:cursor-not-allowed disabled:opacity-50 {focusRing} {checked
      ? 'bg-blue-600'
      : 'bg-neutral-300 dark:bg-neutral-700'}"
    onclick={() => onchange(!checked)}
  >
    <span
      class="absolute top-0.5 left-0.5 size-5 rounded-full bg-white shadow transition-transform {checked
        ? 'translate-x-5'
        : 'translate-x-0'}"
      aria-hidden="true"
    ></span>
  </button>
</div>
