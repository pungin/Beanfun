/**
 * Tiny shared toggle for the router's content-fit window resizer.
 *
 * `router/index.ts` installs a `ResizeObserver` that snaps the OS window
 * to its `[data-window-root]` content height on every viewport change.
 * That's exactly what a full-window overlay ({@link AnnouncementModal})
 * fights when it grows the window: growing the window changes `100vh`,
 * which fires the observer, which shrinks the window straight back.
 *
 * The overlay flips this flag on while it wants a fixed larger size, and
 * off (restoring the prior size itself) on close. Kept in its own
 * dependency-free module so a component can import the setter without
 * pulling the whole router (and its page/tauri imports) into a unit test.
 */
let suspended = false

/** `true` while the content-fit resizer should be a no-op. */
export function isWindowFitSuspended(): boolean {
  return suspended
}

/** Suspend / resume the router's content-fit window resizer. */
export function setWindowFitSuspended(value: boolean): void {
  suspended = value
}
