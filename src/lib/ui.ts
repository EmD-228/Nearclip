// Shared Tailwind class strings. Tailwind v4 scans this file, so every
// utility used here is generated.
//
// Heights are 44px (h-11) below the `sm` breakpoint for touch targets and
// shrink to the desktop sizes at `sm` and up.

export const focusRing =
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 focus-visible:ring-offset-2 focus-visible:ring-offset-neutral-50 dark:focus-visible:ring-offset-neutral-950";

const btnBase = `inline-flex shrink-0 items-center justify-center gap-1.5 rounded-md text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${focusRing}`;

export const btn = {
  primary: `${btnBase} h-11 px-4 sm:h-9 sm:px-3.5 bg-blue-600 text-white hover:bg-blue-700 disabled:hover:bg-blue-600`,
  secondary: `${btnBase} h-11 px-4 sm:h-9 sm:px-3.5 border border-neutral-300 bg-white text-neutral-800 hover:bg-neutral-100 dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-100 dark:hover:bg-neutral-800`,
  ghost: `${btnBase} h-11 px-3 sm:h-8 sm:px-2.5 text-neutral-600 hover:bg-neutral-200/70 hover:text-neutral-900 dark:text-neutral-400 dark:hover:bg-neutral-800 dark:hover:text-neutral-100`,
  danger: `${btnBase} h-11 px-4 sm:h-9 sm:px-3.5 border border-red-200 bg-white text-red-600 hover:bg-red-50 dark:border-red-900/60 dark:bg-neutral-900 dark:text-red-400 dark:hover:bg-red-950/40`,
  dangerSolid: `${btnBase} h-11 px-4 sm:h-9 sm:px-3.5 bg-red-600 text-white hover:bg-red-700`,
  icon: `${btnBase} size-11 sm:size-8 text-neutral-500 hover:bg-neutral-200/70 hover:text-neutral-900 dark:text-neutral-400 dark:hover:bg-neutral-800 dark:hover:text-neutral-100`,
};

// text-base below `sm` so mobile browsers do not zoom into the field on focus.
export const input = `h-11 w-full rounded-md border border-neutral-300 bg-white px-3 text-base text-neutral-900 placeholder:text-neutral-400 sm:h-9 sm:text-sm dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-100 dark:placeholder:text-neutral-500 ${focusRing}`;

export const card =
  "rounded-lg border border-neutral-200 bg-white dark:border-neutral-800 dark:bg-neutral-900";

export const sectionTitle =
  "text-xs font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400";

/** Page wrapper: tighter padding on phones, desktop padding at `sm` and up. */
export const page = "mx-auto max-w-2xl p-4 sm:p-8";

export const pageTitle = "text-lg font-semibold tracking-tight sm:text-xl";

export const pageSubtitle = "mt-1 text-sm text-neutral-500 dark:text-neutral-400";

/** Modal backdrop and panel, sized to fit a 360px wide screen. */
export const modalBackdrop =
  "fixed inset-0 z-40 flex items-center justify-center bg-neutral-900/40 p-4 backdrop-blur-[2px] sm:p-6 dark:bg-black/60";

export const modalPanel =
  "w-full max-w-sm rounded-xl border border-neutral-200 bg-white p-5 shadow-2xl sm:p-6 dark:border-neutral-800 dark:bg-neutral-900";
