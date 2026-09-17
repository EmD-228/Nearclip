/** "483912" -> "483 912" */
export function formatPairingCode(code: string): string {
  const digits = code.replace(/\D/g, "");
  if (digits.length !== 6) return code;
  return `${digits.slice(0, 3)} ${digits.slice(3)}`;
}

/** Short relative time: "just now", "3m ago", "2h ago", "Yesterday", "12 Mar". */
export function relativeTime(tsMs: number, now: number = Date.now()): string {
  const diff = Math.max(0, now - tsMs);
  const sec = Math.floor(diff / 1000);
  if (sec < 45) return "just now";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const day = Math.floor(hr / 24);
  if (day === 1) return "Yesterday";
  if (day < 7) return `${day}d ago`;
  const d = new Date(tsMs);
  const sameYear = d.getFullYear() === new Date(now).getFullYear();
  return d.toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    ...(sameYear ? {} : { year: "numeric" }),
  });
}

export function fullTime(tsMs: number): string {
  return new Date(tsMs).toLocaleString();
}

/** Joins incoming text onto an existing draft on a new line (replaces an empty draft). */
export function appendDraft(draft: string, incoming: string): string {
  if (draft.length === 0) return incoming;
  return draft.endsWith("\n") ? draft + incoming : `${draft}\n${incoming}`;
}

/** 1536 -> "1.5 KB", 5_300_000 -> "5.1 MB" (binary units, as file managers show). */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

export function plural(n: number, singular: string, pluralForm = `${singular}s`): string {
  return `${n} ${n === 1 ? singular : pluralForm}`;
}
