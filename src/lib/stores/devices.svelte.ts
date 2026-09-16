import { api } from "../api";
import type { DeviceView } from "../types";

const byName = (a: DeviceView, b: DeviceView) =>
  a.name.localeCompare(b.name, undefined, { sensitivity: "base" });

class DevicesStore {
  list = $state<DeviceView[]>([]);
  loading = $state(false);
  /** True once the first refresh has settled (success or failure). */
  loaded = $state(false);

  paired = $derived(this.list.filter((d) => d.paired).sort(byName));
  available = $derived(this.list.filter((d) => !d.paired).sort(byName));

  set(devices: DeviceView[]) {
    this.list = devices;
    this.loaded = true;
  }

  byId(deviceId: string): DeviceView | undefined {
    return this.list.find((d) => d.deviceId === deviceId);
  }

  async refresh() {
    this.loading = true;
    try {
      this.set(await api.listDevices());
    } finally {
      this.loading = false;
      this.loaded = true;
    }
  }
}

export const devices = new DevicesStore();
