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
  import { MAX_SIZE_LABEL } from "../files";
  import { appendDraft, formatBytes } from "../format";
  import { report } from "../report";
  import { devices } from "../stores/devices.svelte";
  import { settings } from "../stores/settings.svelte";
  import { transfers } from "../stores/transfers.svelte";
  import { errorMessage, toasts } from "../stores/toasts.svelte";
  import type { SendTarget } from "../types";
  import { btn, btnFill, card, input, pageSubtitle, pageTitle } from "../ui";

  interface Props {
    target?: SendTarget;
    text?: string;
  }

  let { target = $bindable("all"), text = $bindable("") }: Props = $props();

  let textarea = $state<HTMLTextAreaElement | null>(null);
  let fileInput = $state<HTMLInputElement | null>(null);
  let sendingText = $state(false);
  let dragging = $state(false);

  // The queue lives in a store: it keeps going when this view is closed.
  let sending = $derived(transfers.running || sendingText);
  let hasPaired = $derived(devices.paired.length > 0);
  let canSend = $derived(
    hasPaired && !sending && (text.trim().length > 0 || transfers.files.length > 0),
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

  async function send() {
    if (!canSend) return;
    await transfers.send(target);
    if (text.trim().length === 0) return;
    sendingText = true;
    try {
      report(await api.sendText(target, text), "the text");
    } catch (err) {
      toasts.error(`Send failed: ${errorMessage(err)}`);
    } finally {
      sendingText = false;
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

  function onFilePicked(e: Event) {
    const picker = e.currentTarget as HTMLInputElement;
    transfers.add(picker.files ?? []);
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
    transfers.add(e.dataTransfer?.files ?? []);
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
      Type or paste text, or attach files, then send them to a device on your local network.
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

  {#if transfers.files.length > 0}
    <ul class="mt-3 flex flex-col gap-2">
      {#each transfers.files as queued (queued.file)}
        {@const file = queued.file}
        {@const Icon = file.type.startsWith("image/") ? ImageIcon : FileIcon}
        <li class="{card} flex items-center gap-3 px-3 py-2.5">
          <Icon class="size-5 shrink-0 text-neutral-500 dark:text-neutral-400" aria-hidden="true" />
          <div class="min-w-0 flex-1">
            <p class="truncate text-sm font-medium" title={file.name}>{file.name}</p>
            {#if queued.sending}
              <!-- Scaled rather than resized: the bar moves on every chunk, and
                   transform keeps that off the layout and paint path. -->
              <div
                class="mt-1.5 h-1.5 overflow-hidden rounded-full bg-neutral-200 dark:bg-neutral-800"
                role="progressbar"
                aria-label="Sending {file.name}"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={Math.round(queued.fraction * 100)}
              >
                <div
                  class="h-full w-full origin-left rounded-full bg-blue-600 transition-transform duration-200"
                  style:transform="scaleX({queued.fraction})"
                ></div>
              </div>
            {:else}
              <p
                class="truncate text-xs {queued.error
                  ? 'text-red-600 dark:text-red-400'
                  : 'text-neutral-500 dark:text-neutral-400'}"
              >
                {formatBytes(file.size)}{queued.error
                  ? ` · ${queued.error}`
                  : sending
                    ? " · waiting"
                    : ""}
              </p>
            {/if}
          </div>
          <button
            type="button"
            class={btn.icon}
            onclick={() => transfers.remove(queued)}
            disabled={sending}
            aria-label="Remove {file.name}"
            title="Remove"
          >
            <X class="size-4" aria-hidden="true" />
          </button>
        </li>
      {/each}
    </ul>
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
    <input bind:this={fileInput} type="file" multiple class="hidden" onchange={onFilePicked} />
    <button
      type="button"
      class={btn.secondary}
      onclick={() => fileInput?.click()}
      disabled={sending}
      aria-label="Attach files"
    >
      <Paperclip class="size-4" aria-hidden="true" />
      <span class="hidden sm:inline">Attach</span>
    </button>
    <button type="button" class="{btn.secondary} {btnFill}" onclick={paste}>
      <ClipboardPaste class="size-4" aria-hidden="true" />
      Paste
    </button>
    {#if transfers.running}
      {@const current = transfers.files.find((f) => f.sending)}
      <!-- A queue of files takes minutes: leaving has to be possible. -->
      <button type="button" class="{btn.secondary} {btnFill}" onclick={() => transfers.cancel()}>
        <X class="size-4" aria-hidden="true" />
        Cancel {current ? `· ${Math.round(current.fraction * 100)}%` : ""}
      </button>
    {:else}
      <button type="button" class="{btn.primary} {btnFill}" onclick={send} disabled={!canSend}>
        <Send class="size-4" aria-hidden="true" />
        {sending ? "Sending…" : "Send"}
      </button>
    {/if}
  </div>

  <p class="mt-2 text-xs text-neutral-500 dark:text-neutral-400">
    {#if !hasPaired}
      Pair a device first from the Devices tab.
    {:else if settings.isDesktop}
      <span class="hidden sm:inline">
        Drop files here to attach them. Press <kbd class="rounded border border-neutral-300 px-1 font-mono text-[11px] dark:border-neutral-700">⌘</kbd>
        / <kbd class="rounded border border-neutral-300 px-1 font-mono text-[11px] dark:border-neutral-700">Ctrl</kbd>
        + <kbd class="rounded border border-neutral-300 px-1 font-mono text-[11px] dark:border-neutral-700">Enter</kbd> to send.
      </span>
    {:else}
      Files up to {MAX_SIZE_LABEL} each. You can also share photos or files to NearClip from any
      app.
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
