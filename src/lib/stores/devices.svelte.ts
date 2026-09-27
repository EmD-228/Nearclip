import { api } from "../api";
import type { DeviceView } from "../types";

const byName = (a: DeviceView, b: DeviceView) =>
  a.name.localeCompare(b.name, undefined, { sensitivity: "base" });

// Reachable devices first, then the ones still being checked, then the rest.
const rank = (d: DeviceView) => (d.online === true ? 0 : d.online === null ? 1 : 2);
const byReachability = (a: DeviceView, b: DeviceView) => rank(a) - rank(b) || byName(a, b);

class DevicesStore {
  list = $state<DeviceView[]>([]);
  loading = $state(false);
  /** True once the first refresh has settled (success or failure). */
  loaded = $state(false);

  paired = $derived(this.list.filter((d) => d.paired).sort(byReachability));
  available = $derived(this.list.filter((d) => !d.paired).sort(byName));

  set(devices: DeviceView[]) {
    this.list = devices;
    this.loaded = true;
  }

  byId(deviceId: string): DeviceView | undefined {
    return this.list.find((d) => d.deviceId === deviceId);
  }

  refresh() {
    return this.load(api.listDevices);
  }

  /** Refresh that first re-checks which paired devices answer right now. */
  check() {
    return this.load(api.checkDevices);
  }

  private async load(fetch: () => Promise<DeviceView[]>) {
    this.loading = true;
    try {
      this.set(await fetch());
    } finally {
      this.loading = false;
      this.loaded = true;
    }
  }
}

export const devices = new DevicesStore();
