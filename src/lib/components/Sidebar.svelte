<script lang="ts">
  import { ClipboardCopy, History, Monitor, Send, Settings } from "@lucide/svelte";
  import { devices } from "../stores/devices.svelte";
  import { focusRing } from "../ui";
  import type { View } from "../view";

  interface Props {
    view: View;
    onNavigate: (view: View) => void;
  }

  let { view, onNavigate }: Props = $props();

  const items: { id: View; label: string; icon: typeof Monitor }[] = [
    { id: "devices", label: "Devices", icon: Monitor },
    { id: "send", label: "Send", icon: Send },
    { id: "history", label: "History", icon: History },
    { id: "settings", label: "Settings", icon: Settings },
  ];
</script>

<!-- Desktop: left sidebar (sm and up). -->
<aside
  class="hidden w-52 shrink-0 flex-col border-r border-neutral-200 bg-neutral-100/70 sm:flex dark:border-neutral-800 dark:bg-neutral-900/60"
>
  <div class="flex items-center gap-2 px-4 pt-5 pb-4">
    <ClipboardCopy class="size-5 text-blue-600" aria-hidden="true" />
    <span class="text-sm font-semibold tracking-tight">copynapaste</span>
  </div>

  <nav class="flex flex-col gap-0.5 px-2" aria-label="Main">
    {#each items as item (item.id)}
      {@const active = view === item.id}
      <button
        type="button"
        class="flex h-9 items-center gap-2.5 rounded-md px-2.5 text-sm transition-colors {focusRing} {active
          ? 'bg-white font-medium text-neutral-900 shadow-sm dark:bg-neutral-800 dark:text-neutral-50'
          : 'text-neutral-600 hover:bg-neutral-200/60 hover:text-neutral-900 dark:text-neutral-400 dark:hover:bg-neutral-800/60 dark:hover:text-neutral-100'}"
        aria-current={active ? "page" : undefined}
        onclick={() => onNavigate(item.id)}
      >
        <item.icon class="size-4 {active ? 'text-blue-600' : ''}" aria-hidden="true" />
        <span class="flex-1 text-left">{item.label}</span>
        {#if item.id === "devices" && devices.onlineCount > 0}
          <span
            class="rounded-full bg-green-100 px-1.5 py-px text-[11px] font-medium tabular-nums text-green-700 dark:bg-green-900/50 dark:text-green-300"
            title="{devices.onlineCount} online"
          >
            {devices.onlineCount}
          </span>
        {/if}
      </button>
    {/each}
  </nav>
</aside>

<!-- Mobile: fixed bottom tab bar (below sm). -->
<nav
  class="fixed inset-x-0 bottom-0 z-30 flex border-t border-neutral-200 bg-neutral-100/95 pb-[env(safe-area-inset-bottom)] backdrop-blur sm:hidden dark:border-neutral-800 dark:bg-neutral-900/95"
  aria-label="Main"
>
  {#each items as item (item.id)}
    {@const active = view === item.id}
    <button
      type="button"
      class="relative flex h-14 flex-1 flex-col items-center justify-center gap-1 text-[11px] font-medium transition-colors {focusRing} focus-visible:ring-inset {active
        ? 'text-blue-600'
        : 'text-neutral-500 dark:text-neutral-400'}"
      aria-current={active ? "page" : undefined}
      onclick={() => onNavigate(item.id)}
    >
      <span class="relative">
        <item.icon class="size-5" aria-hidden="true" />
        {#if item.id === "devices" && devices.onlineCount > 0}
          <span
            class="absolute -top-1.5 -right-2.5 min-w-4 rounded-full bg-green-600 px-1 text-center text-[10px] leading-4 font-semibold text-white"
            title="{devices.onlineCount} online"
          >
            {devices.onlineCount}
          </span>
        {/if}
      </span>
      {item.label}
    </button>
  {/each}
</nav>
