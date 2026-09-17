<script lang="ts">
  import { onMount } from "svelte";
  import { ClipboardPaste, Eraser, Send } from "@lucide/svelte";
  import { api } from "../api";
  import { appendDraft, plural } from "../format";
  import { devices } from "../stores/devices.svelte";
  import { settings } from "../stores/settings.svelte";
  import { errorMessage, toasts } from "../stores/toasts.svelte";
  import type { SendTarget } from "../types";
  import { btn, btnFill, input, pageSubtitle, pageTitle } from "../ui";

  interface Props {
    target?: SendTarget;
    text?: string;
  }

  let { target = $bindable("all"), text = $bindable("") }: Props = $props();

  let textarea = $state<HTMLTextAreaElement | null>(null);
  let sending = $state(false);

  let hasPaired = $derived(devices.paired.length > 0);
  let canSend = $derived(text.trim().length > 0 && hasPaired && !sending);

  // If the selected device gets unpaired, fall back to "all".
  $effect(() => {
    if (target !== "all" && !devices.paired.some((d) => d.deviceId === target)) {
      target = "all";
    }
  });

  onMount(() => {
    // Autofocus only on desktop: on phones it would pop the keyboard on every visit.
    if (settings.isDesktop) textarea?.focus();
  });

  function nameOf(deviceId: string): string {
    return devices.byId(deviceId)?.name ?? "Unknown device";
  }

  async function send() {
    if (!canSend) return;
    sending = true;
    try {
      const results = await api.sendText(target, text);
      const okIds = results.filter((r) => r.ok).map((r) => r.deviceId);
      const failed = results.filter((r) => !r.ok);

      if (results.length === 0) {
        toasts.info("No device received the text.");
      } else if (okIds.length === 1 && failed.length === 0) {
        toasts.success(`Sent to ${nameOf(okIds[0])}`);
      } else if (okIds.length > 0) {
        toasts.success(`Sent to ${plural(okIds.length, "device")}`);
      }
      for (const f of failed) {
        toasts.error(`${nameOf(f.deviceId)}: ${f.error ?? "failed to send"}`);
      }
    } catch (err) {
      toasts.error(`Send failed: ${errorMessage(err)}`);
    } finally {
      sending = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      void send();
    }
  }

  function clear() {
    text = "";
    textarea?.focus();
  }

  async function paste() {
    let clip: string;
    try {
      clip = await api.readClipboard();
    } catch (err) {
      toasts.error(`Could not read the clipboard: ${errorMessage(err)}`);
      return;
    }
    if (clip === "") {
      toasts.info("Clipboard is empty");
      return;
    }
    text = appendDraft(text, clip);
    // Same rule as autofocus: focusing on phones would pop the keyboard.
    if (settings.isDesktop) textarea?.focus();
  }
</script>

<!--
  Desktop (sm+): the textarea fills the remaining height.
  Mobile: a fixed-height textarea with the controls directly under it, so the
  Send button stays reachable when the on-screen keyboard is open.
-->
<div class="flex flex-col p-4 sm:h-full sm:p-8">
  <header class="mb-4 sm:mb-5">
    <h1 class={pageTitle}>Send</h1>
    <p class={pageSubtitle}>Paste or type text, choose a device and send it over your local network.</p>
  </header>

  <label for="send-text" class="sr-only">Text to send</label>
  <textarea
    id="send-text"
    bind:this={textarea}
    bind:value={text}
    onkeydown={onKeydown}
    placeholder="Type or paste text to send…"
    spellcheck="false"
    class="{input} h-auto min-h-32 resize-none py-2.5 leading-relaxed sm:min-h-40 sm:flex-1"
  ></textarea>

  <div class="mt-3 flex flex-wrap items-center gap-2 sm:mt-4 sm:gap-3">
    <label for="send-target" class="sr-only">Send to</label>
    <select
      id="send-target"
      bind:value={target}
      disabled={!hasPaired}
      class="{input} min-w-0 flex-1 basis-full pr-8 sm:w-auto sm:min-w-48 sm:flex-none sm:basis-auto"
    >
      <option value="all">All paired devices</option>
      {#each devices.paired as device (device.deviceId)}
        <option value={device.deviceId}>{device.name}</option>
      {/each}
    </select>

    <div class="hidden flex-1 sm:block"></div>

    <!-- Phones: icon-only Clear, then Paste and Send share the row equally. -->
    <button
      type="button"
      class={btn.ghost}
      onclick={clear}
      disabled={text.length === 0}
      aria-label="Clear"
    >
      <Eraser class="size-4" aria-hidden="true" />
      <span class="hidden sm:inline">Clear</span>
    </button>
    <button type="button" class="{btn.secondary} {btnFill}" onclick={paste}>
      <ClipboardPaste class="size-4" aria-hidden="true" />
      Paste
    </button>
    <button type="button" class="{btn.primary} {btnFill}" onclick={send} disabled={!canSend}>
      <Send class="size-4" aria-hidden="true" />
      {sending ? "Sending…" : "Send"}
    </button>
  </div>

  <p class="mt-2 text-xs text-neutral-500 dark:text-neutral-400">
    {#if !hasPaired}
      Pair a device first from the Devices tab.
    {:else if settings.isDesktop}
      <span class="hidden sm:inline">
        Press <kbd class="rounded border border-neutral-300 px-1 font-mono text-[11px] dark:border-neutral-700">⌘</kbd>
        / <kbd class="rounded border border-neutral-300 px-1 font-mono text-[11px] dark:border-neutral-700">Ctrl</kbd>
        + <kbd class="rounded border border-neutral-300 px-1 font-mono text-[11px] dark:border-neutral-700">Enter</kbd> to send.
      </span>
    {/if}
  </p>
</div>
