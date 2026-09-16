import { api } from "../api";
import type { Identity, Settings } from "../types";

export const DEFAULT_SETTINGS: Settings = {
  deviceName: "",
  autoSync: false,
  writeReceivedToClipboard: true,
  notifyOnReceive: true,
  closeToTray: true,
  autostart: false,
  discovery: false,
};

class SettingsStore {
  settings = $state<Settings>({ ...DEFAULT_SETTINGS });
  identity = $state<Identity | null>(null);
  loaded = $state(false);
  saving = $state(false);

  /** True on desktop platforms. Defaults to true until the identity has loaded. */
  isDesktop = $derived(this.identity?.desktop ?? true);
  platform = $derived(this.identity?.platform ?? "unknown");

  set(settings: Settings) {
    this.settings = settings;
    this.loaded = true;
  }

  async load() {
    try {
      this.set(await api.getSettings());
    } finally {
      this.loaded = true;
    }
  }

  async loadIdentity() {
    this.identity = await api.getIdentity();
  }

  /** Persist a partial change. Rolls back the local copy if the backend rejects it. */
  async save(patch: Partial<Settings>) {
    const previous = { ...this.settings };
    const next = { ...previous, ...patch };
    this.settings = next;
    this.saving = true;
    try {
      this.settings = await api.setSettings(next);
    } catch (err) {
      this.settings = previous;
      throw err;
    } finally {
      this.saving = false;
    }
  }

  async setDeviceName(name: string) {
    const trimmed = name.trim();
    if (!trimmed || trimmed === this.settings.deviceName) return;
    await api.setDeviceName(trimmed);
    this.settings = { ...this.settings, deviceName: trimmed };
    if (this.identity) this.identity = { ...this.identity, name: trimmed };
  }
}

export const settings = new SettingsStore();
