import { api } from "../api";
import type { HistoryItem } from "../types";

const MAX_ITEMS = 500;
const newestFirst = (a: HistoryItem, b: HistoryItem) => b.tsMs - a.tsMs;

class HistoryStore {
  items = $state<HistoryItem[]>([]);
  loaded = $state(false);

  set(items: HistoryItem[]) {
    this.items = [...items].sort(newestFirst);
    this.loaded = true;
  }

  /** Insert or replace an item (by id), keeping newest first. */
  upsert(item: HistoryItem) {
    const rest = this.items.filter((h) => h.id !== item.id);
    this.items = [item, ...rest].sort(newestFirst).slice(0, MAX_ITEMS);
  }

  async refresh() {
    try {
      this.set(await api.getHistory());
    } finally {
      this.loaded = true;
    }
  }

  async clear() {
    await api.clearHistory();
    this.items = [];
  }
}

export const history = new HistoryStore();
