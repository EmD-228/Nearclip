<script lang="ts">
  import { onMount } from "svelte";
  import {
    ClipboardPaste,
    Eraser,
    File as FileIcon,
    Image as ImageIcon,
    Paperclip,
    Send,
    X,
  } from "@lucide/svelte";
  import { api } from "../api";
  import { MAX_FILE_BYTES, sendFile } from "../files";
  import { appendDraft, formatBytes, plural } from "../format";
  import { devices } from "../stores/devices.svelte";
  import { settings } from "../stores/settings.svelte";
  import { errorMessage, toasts } from "../stores/toasts.svelte";
  import type { SendResult, SendTarget } from "../types";
  import { btn, btnFill, card, input, pageSubtitle, pageTitle } from "../ui";

  interface Props {
    target?: SendTarget;
    text?: string;
    attachment?: File | null;
  }

  let {
    target = $bindable("all"),
    text = $bindable(""),
    attachment = $bindable(null),
  }: Props = $props();

  let textarea = $state<HTMLTextAreaElement | null>(null);
  let fileInput = $state<HTMLInputElement | null>(null);
  let sending = $state(false);
  /** Fraction of the attachment sent, while a file transfer runs. */
  let progress = $state<number | null>(null);
  let dragging = $state(false);

  let hasPaired = $derived(devices.paired.length > 0);
  let canSend = $derived(
    hasPaired && !sending && (text.trim().length > 0 || attachment !== null),
  );

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

  /** One toast for the successes, one per failed device. */
  function report(results: SendResult[], what: string) {
    const okIds = results.filter((r) => r.ok).map((r) => r.deviceId);
    if (results.length === 0) {
      toasts.info(`No device received ${what}.`);
    } else if (okIds.length === 1 && okIds.length === results.length) {
      toasts.success(`Sent ${what} to ${nameOf(okIds[0])}`);
    } else if (okIds.length > 0) {
      toasts.success(`Sent ${what} to ${plural(okIds.length, "device")}`);
    }
    for (const f of results.filter((r) => !r.ok)) {
      toasts.error(`${nameOf(f.deviceId)}: ${f.error ?? "failed to send"}`);
    }
  }

  async function send() {
    if (!canSend) return;
    sending = true;
    try {
      if (attachment) {
        const sent = attachment;
        progress = 0;
        const results = await sendFile(target, sent, (fraction) => (progress = fraction));
        report(results, sent.name);
        if (results.some((r) => r.ok)) attachment = null;
      }
      if (text.trim().length > 0) {
        report(await api.sendText(target, text), "the text");
      }
    } catch (err) {
      toasts.error(`Send failed: ${errorMessage(err)}`);
    } finally {
      sending = false;
      progress = null;
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

  const maxSize = formatBytes(MAX_FILE_BYTES);

  function attach(file: File | undefined) {
    if (!file) return;
    if (file.size === 0) {
      toasts.error(`${file.name} is empty`);
    } else if (file.size > MAX_FILE_BYTES) {
      toasts.error(`${file.name} is too large (${maxSize} max)`);
    } else {
      attachment = file;
    }
  }

  function onFilePicked(e: Event) {
    const picker = e.currentTarget as HTMLInputElement;
    attach(picker.files?.[0]);
    // Picking the same file again must fire `change` again.
    picker.value = "";
  }

  function hasFiles(e: DragEvent): boolean {
    return e.dataTransfer?.types.includes("Files") ?? false;
  }

  function onDragOver(e: DragEvent) {
    if (!hasFiles(e) || sending) return;
    e.preventDefault();
    dragging = true;
  }

  function onDragLeave(e: DragEvent) {
    // Ignore moves between children of the drop zone.
    const next = e.relatedTarget as Node | null;
    if (next && (e.currentTarget as HTMLElement).contains(next)) return;
    dragging = false;
  }

  // Only fires when onDragOver accepted the drag (files, not sending).
  function onDrop(e: DragEvent) {
    e.preventDefault();
    dragging = false;
    attach(e.dataTransfer?.files[0]);
  }
</script>

<!--
  Desktop (sm+): the textarea fills the remaining height, and the whole view
  accepts a dropped file.
  Mobile: a fixed-height textarea with the controls directly under it, so the
  Send button stays reachable when the on-screen keyboard is open.
-->
<div
  class="relative flex flex-col p-4 sm:h-full sm:p-8"
  role="region"
  aria-label="Send"
  ondragover={onDragOver}
  ondragleave={onDragLeave}
  ondrop={onDrop}
>
  <header class="mb-4 sm:mb-5">
    <h1 class={pageTitle}>Send</h1>
    <p class={pageSubtitle}>
      Type or paste text, or attach a file, then send it to a device on your local network.
    </p>
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

  {#if attachment}
    {@const Icon = attachment.type.startsWith("image/") ? ImageIcon : FileIcon}
    <div class="{card} mt-3 flex items-center gap-3 px-3 py-2.5">
      <Icon class="size-5 shrink-0 text-neutral-500 dark:text-neutral-400" aria-hidden="true" />
      <div class="min-w-0 flex-1">
        <p class="truncate text-sm font-medium" title={attachment.name}>{attachment.name}</p>
        {#if progress === null}
          <p class="text-xs text-neutral-500 dark:text-neutral-400">
            {formatBytes(attachment.size)}
          </p>
        {:else}
          <div
            class="mt-1.5 h-1.5 overflow-hidden rounded-full bg-neutral-200 dark:bg-neutral-800"
            role="progressbar"
            aria-label="Sending {attachment.name}"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(progress * 100)}
          >
            <div
              class="h-full rounded-full bg-blue-600 transition-[width] duration-200"
              style:width="{progress * 100}%"
            ></div>
          </div>
        {/if}
      </div>
      <button
        type="button"
        class={btn.icon}
        onclick={() => (attachment = null)}
        disabled={sending}
        aria-label="Remove {attachment.name}"
        title="Remove"
      >
        <X class="size-4" aria-hidden="true" />
      </button>
    </div>
  {/if}

  <div class="mt-3 flex flex-wrap items-center gap-2 sm:mt-4 sm:gap-3">
    <label for="send-target" class="sr-only">Send to</label>
    <select
      id="send-target"
      bind:value={target}
      disabled={!hasPaired || sending}
      class="{input} min-w-0 flex-1 basis-full pr-8 sm:w-auto sm:min-w-48 sm:flex-none sm:basis-auto"
    >
      <option value="all">All paired devices</option>
      {#each devices.paired as device (device.deviceId)}
        <option value={device.deviceId}>{device.name}</option>
      {/each}
    </select>

    <div class="hidden flex-1 sm:block"></div>

    <!-- Phones: icon-only Clear and Attach, then Paste and Send share the row equally. -->
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
    <input bind:this={fileInput} type="file" class="hidden" onchange={onFilePicked} />
    <button
      type="button"
      class={btn.secondary}
      onclick={() => fileInput?.click()}
      disabled={sending}
      aria-label="Attach a file"
    >
      <Paperclip class="size-4" aria-hidden="true" />
      <span class="hidden sm:inline">Attach</span>
    </button>
    <button type="button" class="{btn.secondary} {btnFill}" onclick={paste}>
      <ClipboardPaste class="size-4" aria-hidden="true" />
      Paste
    </button>
    <button type="button" class="{btn.primary} {btnFill}" onclick={send} disabled={!canSend}>
      <Send class="size-4" aria-hidden="true" />
      {#if !sending}
        Send
      {:else if progress !== null}
        Sending {Math.round(progress * 100)}%
      {:else}
        Sending…
      {/if}
    </button>
  </div>

  <p class="mt-2 text-xs text-neutral-500 dark:text-neutral-400">
    {#if !hasPaired}
      Pair a device first from the Devices tab.
    {:else if settings.isDesktop}
      <span class="hidden sm:inline">
        Drop a file here to attach it. Press <kbd class="rounded border border-neutral-300 px-1 font-mono text-[11px] dark:border-neutral-700">⌘</kbd>
        / <kbd class="rounded border border-neutral-300 px-1 font-mono text-[11px] dark:border-neutral-700">Ctrl</kbd>
        + <kbd class="rounded border border-neutral-300 px-1 font-mono text-[11px] dark:border-neutral-700">Enter</kbd> to send.
      </span>
    {:else}
      Files up to {maxSize}. You can also share a photo or file to NearClip from any app.
    {/if}
  </p>

  {#if dragging}
    <div
      class="pointer-events-none absolute inset-2 flex items-center justify-center rounded-xl border-2 border-dashed border-blue-500 bg-blue-50/90 sm:inset-4 dark:bg-blue-950/80"
      aria-hidden="true"
    >
      <p class="flex items-center gap-2 text-sm font-medium text-blue-700 dark:text-blue-200">
        <Paperclip class="size-4" />
        Drop to attach
      </p>
    </div>
  {/if}
</div>
