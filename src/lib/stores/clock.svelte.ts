/**
 * One ticking clock behind every relative time on screen ("3m ago").
 *
 * It stops while the window is hidden, so a phone in a pocket does not wake up
 * to re-render labels nobody is reading. Components only read `clock.now`:
 * Svelte re-renders whoever reads it, and a tick nobody reads costs one
 * assignment.
 */
const TICK_MS = 30_000;

class Clock {
  now = $state(Date.now());
  // Plain field, not $state: nothing renders it.
  private timer: ReturnType<typeof setInterval> | null = null;

  constructor() {
    document.addEventListener("visibilitychange", () => this.sync());
    this.sync();
  }

  private sync() {
    if (document.hidden) {
      if (this.timer !== null) clearInterval(this.timer);
      this.timer = null;
    } else if (this.timer === null) {
      // Catch up first: the last tick may be far behind after a hidden spell.
      this.now = Date.now();
      this.timer = setInterval(() => (this.now = Date.now()), TICK_MS);
    }
  }
}

export const clock = new Clock();
