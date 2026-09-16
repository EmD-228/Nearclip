<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../api";
  import Toggle from "../components/Toggle.svelte";
  import { settings } from "../stores/settings.svelte";
  import { errorMessage, toasts } from "../stores/toasts.svelte";
  import type { ListenInfo, Settings } from "../types";
  import { card, input, page, pageSubtitle, pageTitle, sectionTitle } from "../ui";
  import { APP_VERSION } from "../view";

  let nameDraft = $state("");
  let listenInfo = $state<ListenInfo | null>(null);

  // Keep the draft in sync with the stored name unless the user is editing.
  let editing = $state(false);
  $effect(() => {
    if (!editing) nameDraft = settings.settings.deviceName;
  });

  onMount(async () => {
    try {
      listenInfo = await api.getListenInfo();
    } catch {
      listenInfo = null;
    }
  });

  async function commitName() {
    editing = false;
    const trimmed = nameDraft.trim();
    if (!trimmed) {
      nameDraft = settings.settings.deviceName;
      return;
    }
    if (trimmed === settings.settings.deviceName) return;
    try {
      await settings.setDeviceName(trimmed);
      toasts.success("Device name updated");
    } catch (err) {
      nameDraft = settings.settings.deviceName;
      toasts.error(`Could not rename device: ${errorMessage(err)}`);
    }
  }

  function onNameKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      (e.currentTarget as HTMLInputElement).blur();
    } else if (e.key === "Escape") {
      nameDraft = settings.settings.deviceName;
      (e.currentTarget as HTMLInputElement).blur();
    }
  }

  async function toggle<K extends keyof Settings>(key: K, value: Settings[K]) {
    try {
      await settings.save({ [key]: value } as Partial<Settings>);
    } catch (err) {
      toasts.error(`Could not save settings: ${errorMessage(err)}`);
    }
  }

  interface ToggleDef {
    key: keyof Settings;
    label: string;
    description: string;
    /** Only meaningful on desktop (background clipboard watch, tray, login items). */
    desktopOnly?: boolean;
  }

  const allToggles: ToggleDef[] = [
    {
      key: "autoSync",
      label: "Auto-sync clipboard",
      description: "Send every copied text to paired devices automatically.",
      desktopOnly: true,
    },
    {
      key: "writeReceivedToClipboard",
      label: "Write received text to clipboard",
      description: "Text received from paired devices replaces your clipboard.",
    },
    {
      key: "notifyOnReceive",
      label: "Notify when text is received",
      description: "Show a system notification for incoming text.",
    },
    {
      key: "closeToTray",
      label: "Keep running in the tray",
      description: "Closing the window hides it instead of quitting the app.",
      desktopOnly: true,
    },
    {
      key: "autostart",
      label: "Start at login",
      description: "Launch nearclip automatically when you sign in.",
      desktopOnly: true,
    },
  ];

  let toggles = $derived(allToggles.filter((t) => settings.isDesktop || !t.desktopOnly));
</script>

<div class={page}>
  <header class="mb-5 sm:mb-6">
    <h1 class={pageTitle}>Settings</h1>
    <p class={pageSubtitle}>
      How this device shows up on the network and how it handles clipboard text.
    </p>
  </header>

  <div class="flex flex-col gap-6 sm:gap-8">
    <section aria-labelledby="general-heading">
      <h2 id="general-heading" class="{sectionTitle} mb-2">General</h2>
      <div class="{card} divide-y divide-neutral-200 px-4 dark:divide-neutral-800">
        <div class="flex flex-col gap-2 py-3 sm:flex-row sm:items-center sm:justify-between sm:gap-6">
          <div class="min-w-0">
            <label for="device-name" class="text-sm font-medium">Device name</label>
            <p class="mt-0.5 text-xs text-neutral-500 dark:text-neutral-400">
              Shown to other devices when they discover you.
            </p>
          </div>
          <input
            id="device-name"
            type="text"
            class="{input} sm:w-56"
            bind:value={nameDraft}
            maxlength="48"
            autocomplete="off"
            onfocus={() => (editing = true)}
            onblur={commitName}
            onkeydown={onNameKeydown}
          />
        </div>

        {#each toggles as t (t.key)}
          <Toggle
            label={t.label}
            description={t.description}
            checked={settings.settings[t.key] as boolean}
            disabled={settings.saving}
            onchange={(v) => toggle(t.key, v as Settings[typeof t.key])}
          />
        {/each}
      </div>
    </section>

    <section aria-labelledby="device-heading">
      <h2 id="device-heading" class="{sectionTitle} mb-2">This device</h2>
      <dl class="{card} grid grid-cols-[6.5rem_1fr] gap-x-3 gap-y-3 px-4 py-3 text-sm sm:grid-cols-[9rem_1fr] sm:gap-x-4">
        <dt class="text-neutral-500 dark:text-neutral-400">Fingerprint</dt>
        <dd class="font-mono text-xs leading-5 break-all select-text">
          {settings.identity?.fingerprint ?? "—"}
        </dd>

        <dt class="text-neutral-500 dark:text-neutral-400">Listening on</dt>
        <dd class="font-mono text-xs leading-5 select-text">
          {#if listenInfo}
            {#if listenInfo.addrs.length === 0}
              port {listenInfo.port}
            {:else}
              {#each listenInfo.addrs as addr (addr)}
                <div>{addr}:{listenInfo.port}</div>
              {/each}
            {/if}
          {:else}
            —
          {/if}
        </dd>

        <dt class="text-neutral-500 dark:text-neutral-400">Version</dt>
        <dd class="tabular-nums">{APP_VERSION}</dd>
      </dl>
    </section>

    <section aria-labelledby="troubleshooting-heading">
      <h2 id="troubleshooting-heading" class="{sectionTitle} mb-2">Troubleshooting</h2>
      <div class="{card} px-4 py-3 text-sm text-neutral-600 dark:text-neutral-300">
        <p>
          Devices only find each other on the same local network. If a device does not show up,
          check that both computers are on the same Wi-Fi or wired network, and that your firewall
          allows nearclip to accept incoming connections on the port listed above. Guest
          networks and some office networks block device-to-device traffic.
        </p>
      </div>
    </section>
  </div>
</div>
